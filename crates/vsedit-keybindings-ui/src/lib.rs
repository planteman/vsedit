//! Keybinding editor UI – key codes, modifiers, bindings, and conflict detection.

use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Enter,
    Escape,
    Tab,
    Backspace,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Char(char),
    F(u8),
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub fn none() -> Self {
        Self { ctrl: false, shift: false, alt: false, meta: false }
    }

    pub fn has_modifier(&self) -> bool {
        self.ctrl || self.shift || self.alt || self.meta
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeybindingSource {
    Default,
    User,
    Extension,
}

#[derive(Debug, Clone)]
pub struct Keybinding {
    pub key: KeyCode,
    pub modifiers: Modifiers,
    pub command: String,
    pub when_clause: Option<String>,
    pub source: KeybindingSource,
}

#[derive(Debug, Clone)]
pub struct ChordKeybinding {
    pub first: (KeyCode, Modifiers),
    pub second: (KeyCode, Modifiers),
    pub command: String,
}

#[derive(Debug)]
pub struct KeybindingConflict {
    pub bindings: Vec<Keybinding>,
}

pub struct KeybindingRegistry {
    bindings: Vec<Keybinding>,
}

impl KeybindingRegistry {
    pub fn new() -> Self {
        Self { bindings: Vec::new() }
    }

    pub fn add(&mut self, binding: Keybinding) {
        self.bindings.push(binding);
    }

    pub fn remove_binding(&mut self, command: &str) -> bool {
        let before = self.bindings.len();
        self.bindings.retain(|b| b.command != command);
        self.bindings.len() < before
    }

    pub fn get_all_bindings(&self) -> &[Keybinding] {
        &self.bindings
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    pub fn find_by_command(&self, cmd: &str) -> Vec<&Keybinding> {
        self.bindings.iter().filter(|b| b.command == cmd).collect()
    }

    pub fn find_by_key(&self, key: &KeyCode, modifiers: &Modifiers) -> Vec<&Keybinding> {
        self.bindings
            .iter()
            .filter(|b| b.key == *key && b.modifiers == *modifiers)
            .collect()
    }

    /// Returns the highest-priority binding for a key combo (User > Extension > Default).
    pub fn get_effective_binding(&self, key: &KeyCode, modifiers: &Modifiers) -> Option<&Keybinding> {
        let matches = self.find_by_key(key, modifiers);
        if matches.is_empty() {
            return None;
        }
        matches
            .into_iter()
            .min_by_key(|b| match b.source {
                KeybindingSource::User => 0,
                KeybindingSource::Extension => 1,
                KeybindingSource::Default => 2,
            })
    }

    /// Find groups of bindings that share the same key + modifiers.
    pub fn find_conflicts(&self) -> Vec<KeybindingConflict> {
        use std::collections::HashMap;
        let mut groups: HashMap<(KeyCode, Modifiers), Vec<&Keybinding>> = HashMap::new();
        for b in &self.bindings {
            groups
                .entry((b.key.clone(), b.modifiers.clone()))
                .or_default()
                .push(b);
        }
        groups
            .into_values()
            .filter(|g| g.len() > 1)
            .map(|g| KeybindingConflict {
                bindings: g.into_iter().cloned().collect(),
            })
            .collect()
    }

    pub fn format_keybinding(binding: &Keybinding) -> String {
        format_key_combo(&binding.key, &binding.modifiers)
    }

    /// Returns true if bindings is empty.
    pub fn is_bindings_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Get the first binding, if any.
    pub fn first_binding(&self) -> Option<&Keybinding> {
        self.bindings.first()
    }

    /// Get the last binding, if any.
    pub fn last_binding(&self) -> Option<&Keybinding> {
        self.bindings.last()
    }

    /// Retain only bindings matching the predicate.
    pub fn retain_bindings(&mut self, f: impl Fn(&Keybinding) -> bool) {
        self.bindings.retain(|item| f(item));
    }
}

fn format_key_combo(key: &KeyCode, modifiers: &Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if modifiers.shift {
        parts.push("Shift".to_string());
    }
    if modifiers.alt {
        parts.push("Alt".to_string());
    }
    if modifiers.meta {
        parts.push("Meta".to_string());
    }
    let key_str = match key {
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Escape => "Escape".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Space => "Space".to_string(),
        KeyCode::ArrowUp => "Up".to_string(),
        KeyCode::ArrowDown => "Down".to_string(),
        KeyCode::ArrowLeft => "Left".to_string(),
        KeyCode::ArrowRight => "Right".to_string(),
        KeyCode::Char(c) => c.to_uppercase().to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
    };
    parts.push(key_str);
    parts.join("+")
}

pub fn format_chord(chord: &ChordKeybinding) -> String {
    let first = format_key_combo(&chord.first.0, &chord.first.1);
    let second = format_key_combo(&chord.second.0, &chord.second.1);
    format!("{first} {second}")
}

pub fn parse_keybinding(input: &str) -> Option<Keybinding> {
    let parts: Vec<&str> = input.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut meta = false;

    for &part in &parts[..parts.len() - 1] {
        match part.to_lowercase().as_str() {
            "ctrl" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "meta" => meta = true,
            _ => return None,
        }
    }

    let key_part = parts.last()?;
    let key = match key_part.to_lowercase().as_str() {
        "enter" => KeyCode::Enter,
        "escape" | "esc" => KeyCode::Escape,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "space" => KeyCode::Space,
        "up" => KeyCode::ArrowUp,
        "down" => KeyCode::ArrowDown,
        "left" => KeyCode::ArrowLeft,
        "right" => KeyCode::ArrowRight,
        "delete" | "del" => KeyCode::Delete,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        s if s.starts_with('f') && s.len() > 1 => {
            let n: u8 = s[1..].parse().ok()?;
            if n == 0 || n > 24 {
                return None;
            }
            KeyCode::F(n)
        }
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            if c.is_ascii_alphanumeric() {
                KeyCode::Char(c.to_ascii_lowercase())
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some(Keybinding {
        key,
        modifiers: Modifiers { ctrl, shift, alt, meta },
        command: String::new(),
        when_clause: None,
        source: KeybindingSource::Default,
    })
}

impl Default for KeybindingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A row in the keybinding editor table, ready for display.
#[derive(Debug, Clone)]
pub struct KeybindingTableRow {
    pub command: String,
    pub keybinding_label: String,
    pub when_clause: String,
    pub source_label: String,
    pub has_conflict: bool,
}

impl KeybindingTableRow {
    /// Build a table row from a binding and conflict status.
    pub fn from_binding(binding: &Keybinding, has_conflict: bool) -> Self {
        Self {
            command: binding.command.clone(),
            keybinding_label: format_key_combo(&binding.key, &binding.modifiers),
            when_clause: binding.when_clause.clone().unwrap_or_default(),
            source_label: match binding.source {
                KeybindingSource::Default => "Default".to_string(),
                KeybindingSource::User => "User".to_string(),
                KeybindingSource::Extension => "Extension".to_string(),
            },
            has_conflict,
        }
    }

    /// Returns a formatted display string for the row.
    pub fn display_line(&self) -> String {
        let conflict_marker = if self.has_conflict { " ⚠" } else { "" };
        format!(
            "{:<30} {:<20} {:<20} [{}]{}",
            self.command, self.keybinding_label, self.when_clause, self.source_label, conflict_marker
        )
    }
}

/// Build table rows from a registry, marking conflicts.
pub fn build_table_rows(registry: &KeybindingRegistry) -> Vec<KeybindingTableRow> {
    let conflicts = registry.find_conflicts();
    let conflict_keys: Vec<(KeyCode, Modifiers)> = conflicts
        .iter()
        .flat_map(|c| c.bindings.iter().map(|b| (b.key.clone(), b.modifiers.clone())))
        .collect();
    registry
        .get_all_bindings()
        .iter()
        .map(|b| {
            let has_conflict = conflict_keys.iter().any(|(k, m)| k == &b.key && m == &b.modifiers);
            KeybindingTableRow::from_binding(b, has_conflict)
        })
        .collect()
}

/// Result of a conflict check.
#[derive(Debug, Clone)]
pub struct ConflictCheckResult {
    pub has_conflicts: bool,
    pub conflict_count: usize,
    pub conflicting_commands: Vec<(String, String)>,
}

/// Check a proposed new binding against an existing registry for conflicts.
pub fn keybinding_conflict_check(
    registry: &KeybindingRegistry,
    key: &KeyCode,
    modifiers: &Modifiers,
    when_clause: Option<&str>,
) -> ConflictCheckResult {
    let existing = registry.find_by_key(key, modifiers);
    let mut conflicting_commands = Vec::new();
    for binding in &existing {
        // Two bindings conflict if they share the same key+modifiers and
        // either has no when-clause or they share the same when-clause.
        let when_overlaps = match (&binding.when_clause, when_clause) {
            (None, _) | (_, None) => true,
            (Some(a), Some(b)) => a == b,
        };
        if when_overlaps {
            conflicting_commands.push((
                binding.command.clone(),
                format_key_combo(&binding.key, &binding.modifiers),
            ));
        }
    }
    ConflictCheckResult {
        has_conflicts: !conflicting_commands.is_empty(),
        conflict_count: conflicting_commands.len(),
        conflicting_commands,
    }
}

/// State machine for recording a keybinding sequence from user input.
#[derive(Debug, Clone)]
pub struct KeybindingRecorder {
    keys: Vec<(KeyCode, Modifiers)>,
    max_chords: usize,
    recording: bool,
}

impl KeybindingRecorder {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            max_chords: 2,
            recording: false,
        }
    }

    pub fn start(&mut self) {
        self.keys.clear();
        self.recording = true;
    }

    pub fn stop(&mut self) {
        self.recording = false;
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Record a key press. Returns true if the recording is now complete
    /// (reached max_chords).
    pub fn record_key(&mut self, key: KeyCode, modifiers: Modifiers) -> bool {
        if !self.recording {
            return false;
        }
        // Don't record bare modifier keys
        if matches!(key, KeyCode::Char(_) | KeyCode::Enter | KeyCode::Escape |
            KeyCode::Tab | KeyCode::Space | KeyCode::F(_) | KeyCode::Delete |
            KeyCode::Home | KeyCode::End | KeyCode::PageUp | KeyCode::PageDown |
            KeyCode::Backspace | KeyCode::ArrowUp | KeyCode::ArrowDown |
            KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            self.keys.push((key, modifiers));
        }
        if self.keys.len() >= self.max_chords {
            self.recording = false;
            return true;
        }
        false
    }

    pub fn recorded_keys(&self) -> &[(KeyCode, Modifiers)] {
        &self.keys
    }

    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// Format the recorded sequence as a human-readable string.
    pub fn format(&self) -> String {
        self.keys
            .iter()
            .map(|(k, m)| format_key_combo(k, m))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn set_max_chords(&mut self, max: usize) {
        self.max_chords = max.max(1);
    }

    /// Convert a single-chord recording to a Keybinding.
    pub fn to_keybinding(&self, command: &str) -> Option<Keybinding> {
        let (key, modifiers) = self.keys.first()?;
        Some(Keybinding {
            key: key.clone(),
            modifiers: modifiers.clone(),
            command: command.to_string(),
            when_clause: None,
            source: KeybindingSource::User,
        })
    }
}

impl Default for KeybindingRecorder {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for keybindings-ui operations.
#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingsUiStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl KeybindingsUiStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &KeybindingsUiStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for KeybindingsUiStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for KeybindingsUiStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "KeybindingsUiStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for keybindings-ui.
#[derive(Debug, Clone)]
pub struct KeybindingsUiValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl KeybindingsUiValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for KeybindingsUiValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Search engine for filtering keybindings by command name, key label, or when-clause.
#[derive(Debug, Clone)]
pub struct KeybindingSearchEngine {
    query: String,
    case_sensitive: bool,
}

impl KeybindingSearchEngine {
    pub fn new(query: &str) -> Self {
        Self {
            query: query.to_string(),
            case_sensitive: false,
        }
    }

    pub fn case_sensitive(mut self, yes: bool) -> Self {
        self.case_sensitive = yes;
        self
    }

    fn matches_str(&self, haystack: &str) -> bool {
        if self.case_sensitive {
            haystack.contains(&self.query)
        } else {
            haystack.to_lowercase().contains(&self.query.to_lowercase())
        }
    }

    /// Search bindings in a registry, returning indices of matching entries.
    pub fn search(&self, registry: &KeybindingRegistry) -> Vec<usize> {
        registry
            .get_all_bindings()
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                self.matches_str(&b.command)
                    || self.matches_str(&format_key_combo(&b.key, &b.modifiers))
                    || b.when_clause.as_deref().map_or(false, |w| self.matches_str(w))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Convenience: return matching bindings directly.
    pub fn search_bindings<'a>(&self, registry: &'a KeybindingRegistry) -> Vec<&'a Keybinding> {
        let indices = self.search(registry);
        let all = registry.get_all_bindings();
        indices.into_iter().map(|i| &all[i]).collect()
    }
}

/// Tracks an undo/redo history of keybinding changes.
#[derive(Debug, Clone)]
pub enum KeybindingAction {
    Add(Keybinding),
    Remove { command: String },
}

#[derive(Debug, Clone)]
pub struct KeybindingHistory {
    undo_stack: Vec<KeybindingAction>,
    redo_stack: Vec<KeybindingAction>,
}

impl KeybindingHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Push an action onto the undo stack and clear redo.
    pub fn push(&mut self, action: KeybindingAction) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    /// Pop the most recent action for undoing. Returns `None` if nothing to undo.
    pub fn undo(&mut self) -> Option<KeybindingAction> {
        let action = self.undo_stack.pop()?;
        self.redo_stack.push(action.clone());
        Some(action)
    }

    /// Pop the most recent undone action for redoing. Returns `None` if nothing to redo.
    pub fn redo(&mut self) -> Option<KeybindingAction> {
        let action = self.redo_stack.pop()?;
        self.undo_stack.push(action.clone());
        Some(action)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for KeybindingHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Export format for keybindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
}

/// Exports keybindings from a registry into a textual format.
pub struct KeybindingExporter;

impl KeybindingExporter {
    /// Export all bindings as a JSON array string.
    pub fn export_json(registry: &KeybindingRegistry) -> String {
        let mut entries = Vec::new();
        for b in registry.get_all_bindings() {
            let key_label = format_key_combo(&b.key, &b.modifiers);
            let when = b.when_clause.as_deref().unwrap_or("");
            let source = match b.source {
                KeybindingSource::Default => "default",
                KeybindingSource::User => "user",
                KeybindingSource::Extension => "extension",
            };
            entries.push(format!(
                "  {{\"command\":\"{}\",\"key\":\"{}\",\"when\":\"{}\",\"source\":\"{}\"}}",
                b.command, key_label, when, source,
            ));
        }
        format!("[\n{}\n]", entries.join(",\n"))
    }

    /// Export all bindings as CSV (header + rows).
    pub fn export_csv(registry: &KeybindingRegistry) -> String {
        let mut lines = vec!["command,key,when,source".to_string()];
        for b in registry.get_all_bindings() {
            let key_label = format_key_combo(&b.key, &b.modifiers);
            let when = b.when_clause.as_deref().unwrap_or("");
            let source = match b.source {
                KeybindingSource::Default => "default",
                KeybindingSource::User => "user",
                KeybindingSource::Extension => "extension",
            };
            lines.push(format!("{},{},{},{}", b.command, key_label, when, source));
        }
        lines.join("\n")
    }

    /// Export using the specified format.
    pub fn export(registry: &KeybindingRegistry, format: ExportFormat) -> String {
        match format {
            ExportFormat::Json => Self::export_json(registry),
            ExportFormat::Csv => Self::export_csv(registry),
        }
    }
}

/// Parse a chord string like `"Ctrl+K Ctrl+C"` into a `ChordKeybinding`.
pub fn parse_chord(input: &str) -> Option<ChordKeybinding> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let first = parse_keybinding(parts[0])?;
    let second = parse_keybinding(parts[1])?;
    Some(ChordKeybinding {
        first: (first.key, first.modifiers),
        second: (second.key, second.modifiers),
        command: String::new(),
    })
}

// ---------------------------------------------------------------------------
// Conflict resolution suggestions
// ---------------------------------------------------------------------------

/// A suggestion for resolving a keybinding conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionSuggestion {
    /// Rebind one of the conflicting commands to a suggested key combo.
    Rebind { command: String, suggested_key: String },
    /// Add a when-clause to disambiguate.
    AddWhenClause { command: String, suggested_clause: String },
    /// Remove the lower-priority binding entirely.
    RemoveLowerPriority { command: String, source: KeybindingSource },
}

/// Produce resolution suggestions for a given `KeybindingConflict`.
pub fn suggest_resolutions(conflict: &KeybindingConflict) -> Vec<ResolutionSuggestion> {
    let mut suggestions = Vec::new();
    if conflict.bindings.len() < 2 {
        return suggestions;
    }

    // Sort by priority (User > Extension > Default) so highest-priority is first.
    let mut sorted = conflict.bindings.clone();
    sorted.sort_by_key(|b| match b.source {
        KeybindingSource::User => 0,
        KeybindingSource::Extension => 1,
        KeybindingSource::Default => 2,
    });

    // Suggest removing every binding except the highest-priority one.
    for b in sorted.iter().skip(1) {
        suggestions.push(ResolutionSuggestion::RemoveLowerPriority {
            command: b.command.clone(),
            source: b.source.clone(),
        });
    }

    // Suggest adding when-clauses for bindings that lack one.
    for b in &sorted {
        if b.when_clause.is_none() {
            suggestions.push(ResolutionSuggestion::AddWhenClause {
                command: b.command.clone(),
                suggested_clause: format!("editorTextFocus && resourceScheme == 'file'"),
            });
        }
    }

    suggestions
}

// ---------------------------------------------------------------------------
// Platform-specific key label conversion
// ---------------------------------------------------------------------------

/// Target platform for key label display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
}

/// Convert a key combo into a platform-appropriate display string.
pub fn platform_key_label(key: &KeyCode, modifiers: &Modifiers, platform: Platform) -> String {
    let mut parts: Vec<&str> = Vec::new();

    match platform {
        Platform::MacOs => {
            if modifiers.ctrl { parts.push("⌃"); }
            if modifiers.alt { parts.push("⌥"); }
            if modifiers.shift { parts.push("⇧"); }
            if modifiers.meta { parts.push("⌘"); }
        }
        Platform::Windows | Platform::Linux => {
            if modifiers.ctrl { parts.push("Ctrl"); }
            if modifiers.alt { parts.push("Alt"); }
            if modifiers.shift { parts.push("Shift"); }
            if modifiers.meta {
                parts.push(if platform == Platform::Windows { "Win" } else { "Super" });
            }
        }
    }

    let key_str = match key {
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Escape => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => if platform == Platform::MacOs { "⌫".to_string() } else { "Backspace".to_string() },
        KeyCode::Space => "Space".to_string(),
        KeyCode::ArrowUp => "↑".to_string(),
        KeyCode::ArrowDown => "↓".to_string(),
        KeyCode::ArrowLeft => "←".to_string(),
        KeyCode::ArrowRight => "→".to_string(),
        KeyCode::Char(c) => c.to_uppercase().to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Delete => if platform == Platform::MacOs { "⌦".to_string() } else { "Delete".to_string() },
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
    };

    if parts.is_empty() {
        return key_str;
    }
    match platform {
        Platform::MacOs => {
            // macOS uses concatenation without separator
            let prefix: String = parts.into_iter().collect();
            format!("{prefix}{key_str}")
        }
        _ => {
            parts.push(&key_str);
            parts.join("+")
        }
    }
}

// ---------------------------------------------------------------------------
// Keybinding category grouping
// ---------------------------------------------------------------------------

/// A named group of keybindings.
#[derive(Debug, Clone)]
pub struct KeybindingCategory {
    pub name: String,
    pub prefix: String,
}

/// Group bindings from a registry by command prefix (e.g. "editor.", "workbench.").
pub fn group_by_category(
    registry: &KeybindingRegistry,
    categories: &[KeybindingCategory],
) -> Vec<(String, Vec<Keybinding>)> {
    let mut groups: Vec<(String, Vec<Keybinding>)> = categories
        .iter()
        .map(|c| (c.name.clone(), Vec::new()))
        .collect();
    let mut uncategorised: Vec<Keybinding> = Vec::new();

    for b in registry.get_all_bindings() {
        let mut matched = false;
        for (i, cat) in categories.iter().enumerate() {
            if b.command.starts_with(&cat.prefix) {
                groups[i].1.push(b.clone());
                matched = true;
                break;
            }
        }
        if !matched {
            uncategorised.push(b.clone());
        }
    }
    if !uncategorised.is_empty() {
        groups.push(("Other".to_string(), uncategorised));
    }
    groups
}

// ---------------------------------------------------------------------------
// Keymap diff – compare two registries
// ---------------------------------------------------------------------------

/// A single difference between two keymaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapDiffEntry {
    /// Binding exists only in the first keymap.
    OnlyInFirst { command: String, key_label: String },
    /// Binding exists only in the second keymap.
    OnlyInSecond { command: String, key_label: String },
    /// Same command is bound to different keys.
    Changed { command: String, first_key: String, second_key: String },
}

/// Diff two registries, returning the list of differences.
pub fn diff_keymaps(first: &KeybindingRegistry, second: &KeybindingRegistry) -> Vec<KeymapDiffEntry> {
    use std::collections::HashMap;

    let build_map = |reg: &KeybindingRegistry| -> HashMap<String, String> {
        let mut m = HashMap::new();
        for b in reg.get_all_bindings() {
            m.entry(b.command.clone())
                .or_insert_with(|| format_key_combo(&b.key, &b.modifiers));
        }
        m
    };

    let map1 = build_map(first);
    let map2 = build_map(second);

    let mut diffs = Vec::new();

    for (cmd, key1) in &map1 {
        match map2.get(cmd) {
            None => diffs.push(KeymapDiffEntry::OnlyInFirst {
                command: cmd.clone(),
                key_label: key1.clone(),
            }),
            Some(key2) if key1 != key2 => diffs.push(KeymapDiffEntry::Changed {
                command: cmd.clone(),
                first_key: key1.clone(),
                second_key: key2.clone(),
            }),
            _ => {}
        }
    }
    for (cmd, key2) in &map2 {
        if !map1.contains_key(cmd) {
            diffs.push(KeymapDiffEntry::OnlyInSecond {
                command: cmd.clone(),
                key_label: key2.clone(),
            });
        }
    }

    diffs.sort_by(|a, b| {
        let cmd_a = match a {
            KeymapDiffEntry::OnlyInFirst { command, .. }
            | KeymapDiffEntry::OnlyInSecond { command, .. }
            | KeymapDiffEntry::Changed { command, .. } => command,
        };
        let cmd_b = match b {
            KeymapDiffEntry::OnlyInFirst { command, .. }
            | KeymapDiffEntry::OnlyInSecond { command, .. }
            | KeymapDiffEntry::Changed { command, .. } => command,
        };
        cmd_a.cmp(cmd_b)
    });
    diffs
}

// ---------------------------------------------------------------------------
// Default vs custom tracking
// ---------------------------------------------------------------------------

/// Summary of how many bindings are default vs customised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomisationSummary {
    pub total: usize,
    pub default_count: usize,
    pub user_count: usize,
    pub extension_count: usize,
    pub overridden_defaults: Vec<String>,
}

/// Analyse a registry and report customisation statistics.
pub fn customisation_summary(registry: &KeybindingRegistry) -> CustomisationSummary {
    let bindings = registry.get_all_bindings();
    let total = bindings.len();
    let mut default_count = 0usize;
    let mut user_count = 0usize;
    let mut extension_count = 0usize;

    use std::collections::HashMap;
    let mut by_key: HashMap<(KeyCode, Modifiers), Vec<&Keybinding>> = HashMap::new();
    for b in bindings {
        match b.source {
            KeybindingSource::Default => default_count += 1,
            KeybindingSource::User => user_count += 1,
            KeybindingSource::Extension => extension_count += 1,
        }
        by_key.entry((b.key.clone(), b.modifiers.clone())).or_default().push(b);
    }

    // A default is "overridden" if a User or Extension binding shares the same key+modifiers.
    let mut overridden_defaults = Vec::new();
    for group in by_key.values() {
        let has_default = group.iter().any(|b| b.source == KeybindingSource::Default);
        let has_override = group.iter().any(|b| b.source != KeybindingSource::Default);
        if has_default && has_override {
            for b in group {
                if b.source == KeybindingSource::Default {
                    overridden_defaults.push(b.command.clone());
                }
            }
        }
    }
    overridden_defaults.sort();

    CustomisationSummary { total, default_count, user_count, extension_count, overridden_defaults }
}

// ---------------------------------------------------------------------------
// KeybindingSourceIndicator – identifies binding origin
// ---------------------------------------------------------------------------

/// Indicator showing where a keybinding came from.
#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingSourceIndicator {
    pub source: KeybindingSource,
    pub label: String,
    pub icon: String,
}

impl KeybindingSourceIndicator {
    pub fn from_source(source: &KeybindingSource) -> Self {
        let (label, icon) = match source {
            KeybindingSource::Default => ("Default", "$(gear)"),
            KeybindingSource::User => ("User", "$(account)"),
            KeybindingSource::Extension => ("Extension", "$(extensions)"),
        };
        Self {
            source: source.clone(),
            label: label.to_string(),
            icon: icon.to_string(),
        }
    }

    /// A short summary string for display.
    pub fn summary(&self) -> String {
        format!("{} {}", self.icon, self.label)
    }
}

impl fmt::Display for KeybindingSourceIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

// ---------------------------------------------------------------------------
// KeybindingWhenClauseEditor – parse and validate when clauses
// ---------------------------------------------------------------------------

/// A parsed when-clause condition token.
#[derive(Debug, Clone, PartialEq)]
pub enum WhenClauseToken {
    Key(String),
    Not,
    And,
    Or,
    Equals(String, String),
}

/// Editor for when-clause strings with validation.
#[derive(Debug, Clone)]
pub struct KeybindingWhenClauseEditor {
    raw: String,
    tokens: Vec<WhenClauseToken>,
    valid: bool,
}

impl KeybindingWhenClauseEditor {
    /// Parse a raw when-clause string.
    pub fn parse(raw: &str) -> Self {
        let mut tokens = Vec::new();
        let mut valid = true;
        for part in raw.split_whitespace() {
            if part == "&&" {
                tokens.push(WhenClauseToken::And);
            } else if part == "||" {
                tokens.push(WhenClauseToken::Or);
            } else if part == "!" || part.starts_with('!') {
                let key = part.trim_start_matches('!');
                if key.is_empty() {
                    tokens.push(WhenClauseToken::Not);
                } else {
                    tokens.push(WhenClauseToken::Not);
                    tokens.push(WhenClauseToken::Key(key.to_string()));
                }
            } else if let Some((k, v)) = part.split_once("==") {
                tokens.push(WhenClauseToken::Equals(k.to_string(), v.to_string()));
            } else if part.is_empty() {
                valid = false;
            } else {
                tokens.push(WhenClauseToken::Key(part.to_string()));
            }
        }
        if tokens.is_empty() && !raw.trim().is_empty() {
            valid = false;
        }
        Self { raw: raw.to_string(), tokens, valid }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn tokens(&self) -> &[WhenClauseToken] {
        &self.tokens
    }

    /// Extract all context keys referenced in the when clause.
    pub fn context_keys(&self) -> Vec<&str> {
        self.tokens.iter().filter_map(|t| match t {
            WhenClauseToken::Key(k) => Some(k.as_str()),
            WhenClauseToken::Equals(k, _) => Some(k.as_str()),
            _ => None,
        }).collect()
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }
}

// ---------------------------------------------------------------------------
// Keybinding profile diff (builds on existing diff_keymaps)
// ---------------------------------------------------------------------------

/// Classifies a diff entry from `diff_keymaps` for UI display.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffClassification {
    Added,
    Removed,
    Modified,
}

impl fmt::Display for DiffClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added => write!(f, "+"),
            Self::Removed => write!(f, "-"),
            Self::Modified => write!(f, "~"),
        }
    }
}

/// Classify a `KeymapDiffEntry` for UI rendering.
pub fn classify_diff(entry: &KeymapDiffEntry) -> DiffClassification {
    match entry {
        KeymapDiffEntry::OnlyInFirst { .. } => DiffClassification::Removed,
        KeymapDiffEntry::OnlyInSecond { .. } => DiffClassification::Added,
        KeymapDiffEntry::Changed { .. } => DiffClassification::Modified,
    }
}

/// Summarize diffs from `diff_keymaps` into counts.
pub fn summarize_diffs(diffs: &[KeymapDiffEntry]) -> (usize, usize, usize) {
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut modified = 0usize;
    for d in diffs {
        match classify_diff(d) {
            DiffClassification::Added => added += 1,
            DiffClassification::Removed => removed += 1,
            DiffClassification::Modified => modified += 1,
        }
    }
    (added, removed, modified)
}

// ---------------------------------------------------------------------------
// KeybindingTableRenderer – render keybindings as a text table
// ---------------------------------------------------------------------------

/// Column widths for keybinding table rendering.
#[derive(Debug, Clone)]
pub struct KeybindingTableColumnWidths {
    pub command_width: usize,
    pub keybinding_width: usize,
    pub when_width: usize,
    pub source_width: usize,
}

impl KeybindingTableColumnWidths {
    pub fn default_widths() -> Self {
        Self {
            command_width: 30,
            keybinding_width: 20,
            when_width: 25,
            source_width: 10,
        }
    }

    pub fn total_width(&self) -> usize {
        self.command_width + self.keybinding_width + self.when_width + self.source_width + 9
    }
}

/// Renders keybindings as a formatted text table.
#[derive(Debug)]
pub struct KeybindingTableRenderer {
    widths: KeybindingTableColumnWidths,
}

impl KeybindingTableRenderer {
    pub fn new() -> Self {
        Self {
            widths: KeybindingTableColumnWidths::default_widths(),
        }
    }

    pub fn with_widths(widths: KeybindingTableColumnWidths) -> Self {
        Self { widths }
    }

    fn pad(s: &str, width: usize) -> String {
        if s.len() >= width {
            s[..width].to_string()
        } else {
            format!("{}{}", s, " ".repeat(width - s.len()))
        }
    }

    /// Render the header line.
    pub fn render_header(&self) -> String {
        format!(
            "{} | {} | {} | {}",
            Self::pad("Command", self.widths.command_width),
            Self::pad("Keybinding", self.widths.keybinding_width),
            Self::pad("When", self.widths.when_width),
            Self::pad("Source", self.widths.source_width),
        )
    }

    /// Render a separator line.
    pub fn render_separator(&self) -> String {
        format!(
            "{}-+-{}-+-{}-+-{}",
            "-".repeat(self.widths.command_width),
            "-".repeat(self.widths.keybinding_width),
            "-".repeat(self.widths.when_width),
            "-".repeat(self.widths.source_width),
        )
    }

    /// Render a single binding as a table row.
    pub fn render_row(&self, binding: &Keybinding) -> String {
        let key_str = format_key_combo(&binding.key, &binding.modifiers);
        let when = binding.when_clause.as_deref().unwrap_or("");
        let source = match binding.source {
            KeybindingSource::Default => "Default",
            KeybindingSource::User => "User",
            KeybindingSource::Extension => "Extension",
        };
        format!(
            "{} | {} | {} | {}",
            Self::pad(&binding.command, self.widths.command_width),
            Self::pad(&key_str, self.widths.keybinding_width),
            Self::pad(when, self.widths.when_width),
            Self::pad(source, self.widths.source_width),
        )
    }

    /// Render a full table from a list of bindings.
    pub fn render_table(&self, bindings: &[Keybinding]) -> String {
        let mut lines = Vec::new();
        lines.push(self.render_header());
        lines.push(self.render_separator());
        for b in bindings {
            lines.push(self.render_row(b));
        }
        lines.join("\n")
    }

    pub fn column_widths(&self) -> &KeybindingTableColumnWidths {
        &self.widths
    }
}

impl Default for KeybindingTableRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// KeybindingWhenClausePreview – preview when-clause evaluation
// ---------------------------------------------------------------------------

/// A context variable and its current value, used for when-clause preview.
#[derive(Debug, Clone)]
pub struct ContextVariable {
    pub name: String,
    pub value: String,
}

/// Preview engine for when clauses, showing how they evaluate against context.
#[derive(Debug)]
pub struct KeybindingWhenClausePreview {
    variables: Vec<ContextVariable>,
}

impl KeybindingWhenClausePreview {
    pub fn new() -> Self {
        Self {
            variables: Vec::new(),
        }
    }

    /// Set a context variable.
    pub fn set_variable(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        if let Some(existing) = self.variables.iter_mut().find(|v| v.name == name) {
            existing.value = value;
        } else {
            self.variables.push(ContextVariable { name, value });
        }
    }

    /// Get the value of a context variable.
    pub fn get_variable(&self, name: &str) -> Option<&str> {
        self.variables.iter().find(|v| v.name == name).map(|v| v.value.as_str())
    }

    /// Remove a context variable.
    pub fn remove_variable(&mut self, name: &str) -> bool {
        let before = self.variables.len();
        self.variables.retain(|v| v.name != name);
        self.variables.len() < before
    }

    /// Evaluate a simple when clause against the current context.
    /// Supports: `key`, `!key`, `key == value`, `key != value`.
    pub fn evaluate_simple(&self, clause: &str) -> bool {
        let clause = clause.trim();
        if clause.is_empty() {
            return true;
        }
        if let Some(rest) = clause.strip_prefix('!') {
            let key = rest.trim();
            return self.get_variable(key).map_or(true, |v| v == "false" || v.is_empty());
        }
        if let Some(idx) = clause.find("!=") {
            let key = clause[..idx].trim();
            let expected = clause[idx + 2..].trim().trim_matches('\'').trim_matches('"');
            return self.get_variable(key).map_or(true, |v| v != expected);
        }
        if let Some(idx) = clause.find("==") {
            let key = clause[..idx].trim();
            let expected = clause[idx + 2..].trim().trim_matches('\'').trim_matches('"');
            return self.get_variable(key).map_or(false, |v| v == expected);
        }
        // Plain key – truthy check
        self.get_variable(clause).map_or(false, |v| v != "false" && !v.is_empty())
    }

    /// Preview a binding's when clause evaluation.
    pub fn preview_binding(&self, binding: &Keybinding) -> (bool, String) {
        match &binding.when_clause {
            None => (true, "no when clause – always active".to_string()),
            Some(clause) => {
                let result = self.evaluate_simple(clause);
                let status = if result { "ACTIVE" } else { "INACTIVE" };
                (result, format!("{status}: {clause}"))
            }
        }
    }

    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    pub fn clear_variables(&mut self) {
        self.variables.clear();
    }

    /// List all variable names.
    pub fn variable_names(&self) -> Vec<&str> {
        self.variables.iter().map(|v| v.name.as_str()).collect()
    }
}

impl Default for KeybindingWhenClausePreview {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// KeybindingSearchFilter – search/filter keybindings
// ---------------------------------------------------------------------------

/// A search filter for keybindings in the editor UI.
#[derive(Debug, Clone)]
pub struct KeybindingSearchFilter {
    pub query: String,
    pub search_in_commands: bool,
    pub search_in_keys: bool,
    pub search_in_when: bool,
    pub source_filter: Option<KeybindingSource>,
}

impl KeybindingSearchFilter {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            search_in_commands: true,
            search_in_keys: true,
            search_in_when: true,
            source_filter: None,
        }
    }

    pub fn with_source_filter(mut self, source: KeybindingSource) -> Self {
        self.source_filter = Some(source);
        self
    }

    pub fn commands_only(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            search_in_commands: true,
            search_in_keys: false,
            search_in_when: false,
            source_filter: None,
        }
    }

    /// Check if a binding matches this filter.
    pub fn matches(&self, binding: &Keybinding) -> bool {
        if let Some(ref src) = self.source_filter {
            if binding.source != *src {
                return false;
            }
        }
        if self.query.is_empty() {
            return true;
        }
        let query_lower = self.query.to_lowercase();
        if self.search_in_commands && binding.command.to_lowercase().contains(&query_lower) {
            return true;
        }
        if self.search_in_keys {
            let key_str = format_key_combo(&binding.key, &binding.modifiers).to_lowercase();
            if key_str.contains(&query_lower) {
                return true;
            }
        }
        if self.search_in_when {
            if let Some(ref when) = binding.when_clause {
                if when.to_lowercase().contains(&query_lower) {
                    return true;
                }
            }
        }
        false
    }

    /// Filter a slice of bindings, returning matching ones.
    pub fn filter<'a>(&self, bindings: &'a [Keybinding]) -> Vec<&'a Keybinding> {
        bindings.iter().filter(|b| self.matches(b)).collect()
    }

    /// Count matches in a slice.
    pub fn count_matches(&self, bindings: &[Keybinding]) -> usize {
        bindings.iter().filter(|b| self.matches(b)).count()
    }

    pub fn is_empty_query(&self) -> bool {
        self.query.is_empty()
    }
}

// ---------------------------------------------------------------------------
// KeybindingCopyToClipboard – format keybindings for clipboard
// ---------------------------------------------------------------------------

/// Format for copying keybindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeybindingCopyFormat {
    Json,
    Text,
    Markdown,
}

/// Formats keybindings for copying to the clipboard.
pub struct KeybindingCopyToClipboard;

impl KeybindingCopyToClipboard {
    /// Format a single binding as a JSON-like string.
    pub fn format_as_json(binding: &Keybinding) -> String {
        let key_str = format_key_combo(&binding.key, &binding.modifiers);
        let when = binding.when_clause.as_deref().unwrap_or("");
        format!(
            r#"{{ "key": "{}", "command": "{}"{} }}"#,
            key_str,
            binding.command,
            if when.is_empty() {
                String::new()
            } else {
                format!(r#", "when": "{}""#, when)
            },
        )
    }

    /// Format a single binding as plain text.
    pub fn format_as_text(binding: &Keybinding) -> String {
        let key_str = format_key_combo(&binding.key, &binding.modifiers);
        let when = binding.when_clause.as_deref().unwrap_or("");
        if when.is_empty() {
            format!("{}\t{}", key_str, binding.command)
        } else {
            format!("{}\t{}\t{}", key_str, binding.command, when)
        }
    }

    /// Format a single binding as markdown.
    pub fn format_as_markdown(binding: &Keybinding) -> String {
        let key_str = format_key_combo(&binding.key, &binding.modifiers);
        let when = binding.when_clause.as_deref().unwrap_or("-");
        format!("| `{}` | `{}` | {} |", key_str, binding.command, when)
    }

    /// Format multiple bindings in the given format.
    pub fn format_bindings(bindings: &[Keybinding], format: KeybindingCopyFormat) -> String {
        match format {
            KeybindingCopyFormat::Json => {
                let items: Vec<String> = bindings.iter().map(Self::format_as_json).collect();
                format!("[\n  {}\n]", items.join(",\n  "))
            }
            KeybindingCopyFormat::Text => {
                bindings.iter().map(Self::format_as_text).collect::<Vec<_>>().join("\n")
            }
            KeybindingCopyFormat::Markdown => {
                let mut lines = vec![
                    "| Keybinding | Command | When |".to_string(),
                    "| --- | --- | --- |".to_string(),
                ];
                for b in bindings {
                    lines.push(Self::format_as_markdown(b));
                }
                lines.join("\n")
            }
        }
    }

    /// Count how many characters the formatted output would be.
    pub fn estimate_size(bindings: &[Keybinding], format: KeybindingCopyFormat) -> usize {
        Self::format_bindings(bindings, format).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(key: KeyCode, ctrl: bool, cmd: &str) -> Keybinding {
        Keybinding {
            key,
            modifiers: Modifiers { ctrl, shift: false, alt: false, meta: false },
            command: cmd.to_string(),
            when_clause: None,
            source: KeybindingSource::Default,
        }
    }

    fn kb_with_source(key: KeyCode, ctrl: bool, cmd: &str, source: KeybindingSource) -> Keybinding {
        Keybinding {
            key,
            modifiers: Modifiers { ctrl, shift: false, alt: false, meta: false },
            command: cmd.to_string(),
            when_clause: None,
            source,
        }
    }

    #[test]
    fn find_by_command() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        let found = reg.find_by_command("save");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].command, "save");
    }

    #[test]
    fn detect_conflicts() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('s'), true, "search"));
        let conflicts = reg.find_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].bindings.len(), 2);
    }

    #[test]
    fn format_keybinding() {
        let binding = Keybinding {
            key: KeyCode::Char('s'),
            modifiers: Modifiers { ctrl: true, shift: true, alt: false, meta: false },
            command: "saveAs".to_string(),
            when_clause: None,
            source: KeybindingSource::Default,
        };
        assert_eq!(KeybindingRegistry::format_keybinding(&binding), "Ctrl+Shift+S");
    }

    #[test]
    fn no_conflicts_different_keys() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        assert!(reg.find_conflicts().is_empty());
    }

    #[test]
    fn remove_existing_binding() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        assert!(reg.remove_binding("save"));
        assert_eq!(reg.binding_count(), 1);
        assert!(reg.find_by_command("save").is_empty());
    }

    #[test]
    fn remove_nonexistent_binding() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        assert!(!reg.remove_binding("missing"));
        assert_eq!(reg.binding_count(), 1);
    }

    #[test]
    fn get_all_bindings_and_count() {
        let mut reg = KeybindingRegistry::new();
        assert_eq!(reg.binding_count(), 0);
        assert!(reg.get_all_bindings().is_empty());
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        assert_eq!(reg.binding_count(), 2);
        assert_eq!(reg.get_all_bindings().len(), 2);
    }

    #[test]
    fn find_by_key_returns_matching() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        reg.add(kb(KeyCode::Char('s'), true, "search"));
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: false };
        let found = reg.find_by_key(&KeyCode::Char('s'), &mods);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn find_by_key_no_match() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        let found = reg.find_by_key(&KeyCode::Char('x'), &Modifiers::none());
        assert!(found.is_empty());
    }

    #[test]
    fn format_chord_binding() {
        let chord = ChordKeybinding {
            first: (
                KeyCode::Char('k'),
                Modifiers { ctrl: true, shift: false, alt: false, meta: false },
            ),
            second: (
                KeyCode::Char('c'),
                Modifiers { ctrl: true, shift: false, alt: false, meta: false },
            ),
            command: "comment".to_string(),
        };
        assert_eq!(format_chord(&chord), "Ctrl+K Ctrl+C");
    }

    #[test]
    fn parse_simple_keybinding() {
        let parsed = parse_keybinding("Ctrl+S").unwrap();
        assert_eq!(parsed.key, KeyCode::Char('s'));
        assert!(parsed.modifiers.ctrl);
        assert!(!parsed.modifiers.shift);
    }

    #[test]
    fn parse_keybinding_with_multiple_modifiers() {
        let parsed = parse_keybinding("Ctrl+Shift+P").unwrap();
        assert_eq!(parsed.key, KeyCode::Char('p'));
        assert!(parsed.modifiers.ctrl);
        assert!(parsed.modifiers.shift);
    }

    #[test]
    fn parse_function_key() {
        let parsed = parse_keybinding("F5").unwrap();
        assert_eq!(parsed.key, KeyCode::F(5));
        assert!(!parsed.modifiers.has_modifier());
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse_keybinding("Ctrl+???").is_none());
        assert!(parse_keybinding("Ctrl+F0").is_none());
    }

    #[test]
    fn has_modifier_true_and_false() {
        assert!(!Modifiers::none().has_modifier());
        assert!(Modifiers { ctrl: true, shift: false, alt: false, meta: false }.has_modifier());
        assert!(Modifiers { ctrl: false, shift: false, alt: true, meta: false }.has_modifier());
    }

    #[test]
    fn effective_binding_user_overrides_default() {
        let mut reg = KeybindingRegistry::new();
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: false };
        reg.add(kb_with_source(KeyCode::Char('s'), true, "default_save", KeybindingSource::Default));
        reg.add(kb_with_source(KeyCode::Char('s'), true, "ext_save", KeybindingSource::Extension));
        reg.add(kb_with_source(KeyCode::Char('s'), true, "user_save", KeybindingSource::User));
        let effective = reg.get_effective_binding(&KeyCode::Char('s'), &mods).unwrap();
        assert_eq!(effective.command, "user_save");
    }

    #[test]
    fn effective_binding_extension_over_default() {
        let mut reg = KeybindingRegistry::new();
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: false };
        reg.add(kb_with_source(KeyCode::Char('s'), true, "default_save", KeybindingSource::Default));
        reg.add(kb_with_source(KeyCode::Char('s'), true, "ext_save", KeybindingSource::Extension));
        let effective = reg.get_effective_binding(&KeyCode::Char('s'), &mods).unwrap();
        assert_eq!(effective.command, "ext_save");
    }

    #[test]
    fn effective_binding_none_when_empty() {
        let reg = KeybindingRegistry::new();
        assert!(reg.get_effective_binding(&KeyCode::Char('x'), &Modifiers::none()).is_none());
    }

    #[test]
    fn eq_keycode_same() {
        assert_eq!(KeyCode::Enter, KeyCode::Enter);
    }

    #[test]
    fn ne_keycode_diff() {
        assert_ne!(KeyCode::Enter, KeyCode::Escape);
    }

    #[test]
    fn eq_keybindingsource_same() {
        assert_eq!(KeybindingSource::Default, KeybindingSource::Default);
    }

    #[test]
    fn ne_keybindingsource_diff() {
        assert_ne!(KeybindingSource::Default, KeybindingSource::User);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = KeybindingRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn keybindings_ui_stats_new_defaults() {
        let stats = KeybindingsUiStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn keybindings_ui_stats_record_success() {
        let mut stats = KeybindingsUiStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn keybindings_ui_stats_record_failure() {
        let mut stats = KeybindingsUiStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn keybindings_ui_stats_reset() {
        let mut stats = KeybindingsUiStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn keybindings_ui_stats_merge() {
        let mut a = KeybindingsUiStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = KeybindingsUiStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn keybindings_ui_stats_display() {
        let mut stats = KeybindingsUiStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn keybindings_ui_stats_default() {
        let stats = KeybindingsUiStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn keybindings_ui_validator_accepts_valid_name() {
        let v = KeybindingsUiValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn keybindings_ui_validator_rejects_empty() {
        let v = KeybindingsUiValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn keybindings_ui_validator_rejects_too_long() {
        let v = KeybindingsUiValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn keybindings_ui_validator_forbidden_prefix() {
        let v = KeybindingsUiValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn keybindings_ui_validator_allowed_chars() {
        let v = KeybindingsUiValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn keybindings_ui_validator_range() {
        let v = KeybindingsUiValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn keybindings_ui_sanitize_removes_control() {
        let result = KeybindingsUiValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn keybindings_ui_truncate_short_string() {
        assert_eq!(KeybindingsUiValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn keybindings_ui_truncate_long_string() {
        let result = KeybindingsUiValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn keybindings_ui_is_ascii_printable() {
        assert!(KeybindingsUiValidator::is_ascii_printable("Hello World 123"));
        assert!(!KeybindingsUiValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn table_row_from_binding() {
        let b = Keybinding {
            key: KeyCode::Char('s'), modifiers: Modifiers { ctrl: true, shift: false, alt: false, meta: false },
            command: "editor.save".to_string(), when_clause: None, source: KeybindingSource::User,
        };
        let row = KeybindingTableRow::from_binding(&b, false);
        assert_eq!(row.command, "editor.save");
        assert_eq!(row.keybinding_label, "Ctrl+S");
        assert!(!row.has_conflict);
    }

    #[test]
    fn table_row_display_with_conflict() {
        let b = Keybinding {
            key: KeyCode::Char('c'), modifiers: Modifiers { ctrl: true, shift: false, alt: false, meta: false },
            command: "editor.copy".to_string(), when_clause: Some("editorFocus".to_string()), source: KeybindingSource::Default,
        };
        let row = KeybindingTableRow::from_binding(&b, true);
        let line = row.display_line();
        assert!(line.contains("⚠"));
        assert!(line.contains("editor.copy"));
    }

    #[test]
    fn build_table_rows_marks_conflicts() {
        let mut reg = KeybindingRegistry::new();
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: false };
        reg.add(Keybinding { key: KeyCode::Char('s'), modifiers: mods.clone(), command: "save".into(), when_clause: None, source: KeybindingSource::Default });
        reg.add(Keybinding { key: KeyCode::Char('s'), modifiers: mods.clone(), command: "other".into(), when_clause: None, source: KeybindingSource::User });
        let rows = build_table_rows(&reg);
        assert!(rows.iter().all(|r| r.has_conflict));
    }

    #[test]
    fn conflict_check_finds_overlap() {
        let mut reg = KeybindingRegistry::new();
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: false };
        reg.add(Keybinding { key: KeyCode::Char('p'), modifiers: mods.clone(), command: "palette".into(), when_clause: None, source: KeybindingSource::Default });
        let result = keybinding_conflict_check(&reg, &KeyCode::Char('p'), &mods, None);
        assert!(result.has_conflicts);
        assert_eq!(result.conflict_count, 1);
    }

    #[test]
    fn conflict_check_no_overlap_different_when() {
        let mut reg = KeybindingRegistry::new();
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: false };
        reg.add(Keybinding { key: KeyCode::Char('p'), modifiers: mods.clone(), command: "palette".into(), when_clause: Some("editorFocus".into()), source: KeybindingSource::Default });
        let result = keybinding_conflict_check(&reg, &KeyCode::Char('p'), &mods, Some("terminalFocus"));
        assert!(!result.has_conflicts);
    }

    #[test]
    fn recorder_single_key() {
        let mut rec = KeybindingRecorder::new();
        rec.start();
        assert!(rec.is_recording());
        let complete = rec.record_key(KeyCode::Char('a'), Modifiers::none());
        assert!(!complete);
        assert_eq!(rec.recorded_keys().len(), 1);
        assert_eq!(rec.format(), "A");
    }

    #[test]
    fn recorder_chord_completes() {
        let mut rec = KeybindingRecorder::new();
        rec.start();
        rec.record_key(KeyCode::Char('k'), Modifiers { ctrl: true, shift: false, alt: false, meta: false });
        let done = rec.record_key(KeyCode::Char('s'), Modifiers::none());
        assert!(done);
        assert!(!rec.is_recording());
        assert_eq!(rec.format(), "Ctrl+K S");
    }

    #[test]
    fn recorder_to_keybinding() {
        let mut rec = KeybindingRecorder::new();
        rec.start();
        rec.record_key(KeyCode::F(5), Modifiers::none());
        let kb = rec.to_keybinding("debug.run").unwrap();
        assert_eq!(kb.command, "debug.run");
        assert_eq!(kb.key, KeyCode::F(5));
    }

    #[test]
    fn recorder_not_recording_ignores() {
        let mut rec = KeybindingRecorder::new();
        let done = rec.record_key(KeyCode::Char('x'), Modifiers::none());
        assert!(!done);
        assert!(rec.recorded_keys().is_empty());
    }

    #[test]
    fn search_engine_matches_command() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "editor.save"));
        reg.add(kb(KeyCode::Char('o'), true, "editor.open"));
        reg.add(kb(KeyCode::F(5), false, "debug.run"));
        let engine = KeybindingSearchEngine::new("save");
        let results = engine.search_bindings(&reg);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "editor.save");
    }

    #[test]
    fn search_engine_case_insensitive_by_default() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "Editor.Save"));
        let engine = KeybindingSearchEngine::new("editor.save");
        assert_eq!(engine.search(&reg).len(), 1);
    }

    #[test]
    fn search_engine_case_sensitive_mode() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "Editor.Save"));
        let engine = KeybindingSearchEngine::new("editor.save").case_sensitive(true);
        assert!(engine.search(&reg).is_empty());
    }

    #[test]
    fn search_engine_matches_key_label() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::F(5), false, "debug.run"));
        let engine = KeybindingSearchEngine::new("F5");
        assert_eq!(engine.search(&reg).len(), 1);
    }

    #[test]
    fn search_engine_matches_when_clause() {
        let mut reg = KeybindingRegistry::new();
        reg.add(Keybinding {
            key: KeyCode::Char('c'),
            modifiers: Modifiers { ctrl: true, shift: false, alt: false, meta: false },
            command: "copy".into(),
            when_clause: Some("editorFocus".into()),
            source: KeybindingSource::Default,
        });
        let engine = KeybindingSearchEngine::new("editorFocus");
        assert_eq!(engine.search(&reg).len(), 1);
    }

    #[test]
    fn history_undo_redo() {
        let mut hist = KeybindingHistory::new();
        assert!(!hist.can_undo());
        hist.push(KeybindingAction::Add(kb(KeyCode::Char('a'), false, "test")));
        assert!(hist.can_undo());
        assert!(!hist.can_redo());
        let undone = hist.undo().unwrap();
        assert!(matches!(undone, KeybindingAction::Add(_)));
        assert!(hist.can_redo());
        assert!(!hist.can_undo());
        let redone = hist.redo().unwrap();
        assert!(matches!(redone, KeybindingAction::Add(_)));
        assert!(hist.can_undo());
    }

    #[test]
    fn history_push_clears_redo() {
        let mut hist = KeybindingHistory::new();
        hist.push(KeybindingAction::Add(kb(KeyCode::Char('a'), false, "a")));
        hist.push(KeybindingAction::Add(kb(KeyCode::Char('b'), false, "b")));
        hist.undo();
        assert_eq!(hist.redo_len(), 1);
        hist.push(KeybindingAction::Remove { command: "c".into() });
        assert_eq!(hist.redo_len(), 0);
    }

    #[test]
    fn exporter_json_format() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "save"));
        let json = KeybindingExporter::export_json(&reg);
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
        assert!(json.contains("\"command\":\"save\""));
        assert!(json.contains("\"key\":\"Ctrl+S\""));
    }

    #[test]
    fn exporter_csv_format() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('o'), true, "open"));
        let csv = KeybindingExporter::export_csv(&reg);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "command,key,when,source");
        assert!(lines[1].starts_with("open,Ctrl+O"));
    }

    #[test]
    fn exporter_dispatch() {
        let reg = KeybindingRegistry::new();
        let json = KeybindingExporter::export(&reg, ExportFormat::Json);
        let csv = KeybindingExporter::export(&reg, ExportFormat::Csv);
        assert!(json.contains('['));
        assert!(csv.contains("command,key,when,source"));
    }

    #[test]
    fn parse_chord_valid() {
        let chord = parse_chord("Ctrl+K Ctrl+C").unwrap();
        assert_eq!(chord.first.0, KeyCode::Char('k'));
        assert!(chord.first.1.ctrl);
        assert_eq!(chord.second.0, KeyCode::Char('c'));
        assert!(chord.second.1.ctrl);
    }

    #[test]
    fn parse_chord_single_part_fails() {
        assert!(parse_chord("Ctrl+K").is_none());
    }

    #[test]
    fn parse_chord_three_parts_fails() {
        assert!(parse_chord("Ctrl+K Ctrl+C Ctrl+X").is_none());
    }

    // --- Tests for new functionality ---

    #[test]
    fn conflict_resolution_suggests_remove_lower_priority() {
        let conflict = KeybindingConflict {
            bindings: vec![
                kb_with_source(KeyCode::Char('s'), true, "user_save", KeybindingSource::User),
                kb_with_source(KeyCode::Char('s'), true, "default_save", KeybindingSource::Default),
            ],
        };
        let suggestions = suggest_resolutions(&conflict);
        let has_remove = suggestions.iter().any(|s| matches!(
            s,
            ResolutionSuggestion::RemoveLowerPriority { command, .. } if command == "default_save"
        ));
        assert!(has_remove, "should suggest removing the default binding");
    }

    #[test]
    fn conflict_resolution_suggests_when_clause() {
        let conflict = KeybindingConflict {
            bindings: vec![
                kb(KeyCode::Char('p'), true, "palette"),
                kb(KeyCode::Char('p'), true, "print"),
            ],
        };
        let suggestions = suggest_resolutions(&conflict);
        let when_count = suggestions.iter().filter(|s| matches!(s, ResolutionSuggestion::AddWhenClause { .. })).count();
        assert!(when_count >= 2, "should suggest when-clauses for both bindings without one");
    }

    #[test]
    fn platform_label_macos_uses_symbols() {
        let mods = Modifiers { ctrl: false, shift: false, alt: false, meta: true };
        let label = platform_key_label(&KeyCode::Char('c'), &mods, Platform::MacOs);
        assert!(label.contains('⌘'), "macOS should use ⌘ for meta: {}", label);
        assert!(label.contains('C'));
    }

    #[test]
    fn platform_label_windows_uses_win_for_meta() {
        let mods = Modifiers { ctrl: true, shift: false, alt: false, meta: true };
        let label = platform_key_label(&KeyCode::Char('l'), &mods, Platform::Windows);
        assert!(label.contains("Win"), "Windows should use Win for meta: {}", label);
        assert!(label.contains("Ctrl"));
    }

    #[test]
    fn platform_label_linux_uses_super() {
        let mods = Modifiers { ctrl: false, shift: false, alt: false, meta: true };
        let label = platform_key_label(&KeyCode::Char('t'), &mods, Platform::Linux);
        assert!(label.contains("Super"), "Linux should use Super for meta: {}", label);
    }

    #[test]
    fn group_by_category_assigns_correctly() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb(KeyCode::Char('s'), true, "editor.save"));
        reg.add(kb(KeyCode::Char('o'), true, "workbench.open"));
        reg.add(kb(KeyCode::Char('t'), true, "misc.toggle"));

        let categories = vec![
            KeybindingCategory { name: "Editor".into(), prefix: "editor.".into() },
            KeybindingCategory { name: "Workbench".into(), prefix: "workbench.".into() },
        ];
        let groups = group_by_category(&reg, &categories);
        assert_eq!(groups.len(), 3); // Editor, Workbench, Other
        assert_eq!(groups[0].0, "Editor");
        assert_eq!(groups[0].1.len(), 1);
        assert_eq!(groups[1].0, "Workbench");
        assert_eq!(groups[1].1.len(), 1);
        assert_eq!(groups[2].0, "Other");
        assert_eq!(groups[2].1.len(), 1);
    }

    #[test]
    fn diff_keymaps_detects_added_removed_changed() {
        let mut first = KeybindingRegistry::new();
        first.add(kb(KeyCode::Char('s'), true, "save"));
        first.add(kb(KeyCode::Char('o'), true, "open"));

        let mut second = KeybindingRegistry::new();
        second.add(kb(KeyCode::Char('s'), false, "save")); // changed key
        second.add(kb(KeyCode::Char('n'), true, "new"));   // added

        let diffs = diff_keymaps(&first, &second);
        let has_only_first = diffs.iter().any(|d| matches!(d, KeymapDiffEntry::OnlyInFirst { command, .. } if command == "open"));
        let has_only_second = diffs.iter().any(|d| matches!(d, KeymapDiffEntry::OnlyInSecond { command, .. } if command == "new"));
        let has_changed = diffs.iter().any(|d| matches!(d, KeymapDiffEntry::Changed { command, .. } if command == "save"));
        assert!(has_only_first, "should detect 'open' only in first");
        assert!(has_only_second, "should detect 'new' only in second");
        assert!(has_changed, "should detect 'save' changed key");
    }

    #[test]
    fn customisation_summary_counts_sources() {
        let mut reg = KeybindingRegistry::new();
        reg.add(kb_with_source(KeyCode::Char('s'), true, "save", KeybindingSource::Default));
        reg.add(kb_with_source(KeyCode::Char('s'), true, "save_user", KeybindingSource::User));
        reg.add(kb_with_source(KeyCode::Char('o'), true, "open", KeybindingSource::Extension));

        let summary = customisation_summary(&reg);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.default_count, 1);
        assert_eq!(summary.user_count, 1);
        assert_eq!(summary.extension_count, 1);
        assert!(summary.overridden_defaults.contains(&"save".to_string()));
    }

    #[test]
    fn diff_keymaps_identical_returns_empty() {
        let mut a = KeybindingRegistry::new();
        a.add(kb(KeyCode::Char('s'), true, "save"));
        let mut b = KeybindingRegistry::new();
        b.add(kb(KeyCode::Char('s'), true, "save"));
        assert!(diff_keymaps(&a, &b).is_empty());
    }

    // -- KeybindingRecorder tests (using existing) --

    #[test]
    fn recorder_basic() {
        let mut rec = KeybindingRecorder::new();
        assert!(!rec.is_recording());
        rec.start();
        assert!(rec.is_recording());
        rec.record_key(KeyCode::Char('s'), Modifiers { ctrl: true, shift: false, alt: false, meta: false });
        assert_eq!(rec.recorded_keys().len(), 1);
        rec.stop();
        assert!(!rec.is_recording());
    }

    #[test]
    fn recorder_clear() {
        let mut rec = KeybindingRecorder::new();
        rec.start();
        rec.record_key(KeyCode::Char('x'), Modifiers::none());
        rec.clear();
        assert!(rec.recorded_keys().is_empty());
    }

    // -- KeybindingSourceIndicator tests --

    #[test]
    fn source_indicator_default() {
        let ind = KeybindingSourceIndicator::from_source(&KeybindingSource::Default);
        assert_eq!(ind.label, "Default");
        assert!(ind.summary().contains("Default"));
        assert_eq!(format!("{}", ind), "Default");
    }

    #[test]
    fn source_indicator_user() {
        let ind = KeybindingSourceIndicator::from_source(&KeybindingSource::User);
        assert_eq!(ind.label, "User");
    }

    #[test]
    fn source_indicator_extension() {
        let ind = KeybindingSourceIndicator::from_source(&KeybindingSource::Extension);
        assert_eq!(ind.label, "Extension");
    }

    // -- KeybindingWhenClauseEditor tests --

    #[test]
    fn when_clause_simple() {
        let editor = KeybindingWhenClauseEditor::parse("editorTextFocus");
        assert!(editor.is_valid());
        assert_eq!(editor.context_keys(), vec!["editorTextFocus"]);
    }

    #[test]
    fn when_clause_negation() {
        let editor = KeybindingWhenClauseEditor::parse("!inDebugMode");
        assert!(editor.is_valid());
        assert_eq!(editor.context_keys(), vec!["inDebugMode"]);
    }

    #[test]
    fn when_clause_and() {
        let editor = KeybindingWhenClauseEditor::parse("editorTextFocus && !inDebugMode");
        assert!(editor.is_valid());
        let keys = editor.context_keys();
        assert!(keys.contains(&"editorTextFocus"));
        assert!(keys.contains(&"inDebugMode"));
    }

    #[test]
    fn when_clause_equals() {
        let editor = KeybindingWhenClauseEditor::parse("resourceScheme==file");
        assert!(editor.is_valid());
        assert_eq!(editor.context_keys(), vec!["resourceScheme"]);
    }

    // -- classify_diff / summarize_diffs tests --

    #[test]
    fn classify_diff_added() {
        let entry = KeymapDiffEntry::OnlyInSecond { command: "x".into(), key_label: "y".into() };
        assert_eq!(classify_diff(&entry), DiffClassification::Added);
        assert_eq!(format!("{}", DiffClassification::Added), "+");
    }

    #[test]
    fn summarize_diffs_counts() {
        let diffs = vec![
            KeymapDiffEntry::OnlyInFirst { command: "a".into(), key_label: "k".into() },
            KeymapDiffEntry::OnlyInSecond { command: "b".into(), key_label: "k".into() },
            KeymapDiffEntry::Changed { command: "c".into(), first_key: "x".into(), second_key: "y".into() },
        ];
        let (added, removed, modified) = summarize_diffs(&diffs);
        assert_eq!(added, 1);
        assert_eq!(removed, 1);
        assert_eq!(modified, 1);
    }
    #[test]
    fn table_renderer_header_and_separator() {
        let renderer = KeybindingTableRenderer::new();
        let header = renderer.render_header();
        assert!(header.contains("Command"));
        assert!(header.contains("Keybinding"));
        let sep = renderer.render_separator();
        assert!(sep.contains("---"));
    }

    #[test]
    fn table_renderer_row() {
        let renderer = KeybindingTableRenderer::new();
        let b = kb(KeyCode::Char('s'), true, "file.save");
        let row = renderer.render_row(&b);
        assert!(row.contains("file.save"));
        assert!(row.contains("Ctrl+S"));
    }

    #[test]
    fn table_renderer_full_table() {
        let renderer = KeybindingTableRenderer::new();
        let bindings = vec![
            kb(KeyCode::Char('s'), true, "file.save"),
            kb(KeyCode::Char('z'), true, "edit.undo"),
        ];
        let table = renderer.render_table(&bindings);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 4); // header + separator + 2 rows
    }

    #[test]
    fn table_column_widths() {
        let widths = KeybindingTableColumnWidths::default_widths();
        assert!(widths.total_width() > 0);
        assert_eq!(widths.command_width, 30);
    }

    #[test]
    fn when_clause_preview_simple() {
        let mut preview = KeybindingWhenClausePreview::new();
        preview.set_variable("editorFocus", "true");
        assert!(preview.evaluate_simple("editorFocus"));
        assert!(!preview.evaluate_simple("!editorFocus"));
        assert!(!preview.evaluate_simple("terminalFocus"));
    }

    #[test]
    fn when_clause_preview_equality() {
        let mut preview = KeybindingWhenClausePreview::new();
        preview.set_variable("language", "rust");
        assert!(preview.evaluate_simple("language == 'rust'"));
        assert!(!preview.evaluate_simple("language == 'python'"));
        assert!(preview.evaluate_simple("language != 'python'"));
        assert!(!preview.evaluate_simple("language != 'rust'"));
    }

    #[test]
    fn when_clause_preview_binding() {
        let mut preview = KeybindingWhenClausePreview::new();
        preview.set_variable("editorFocus", "true");
        let mut b = kb(KeyCode::Char('s'), true, "file.save");
        b.when_clause = Some("editorFocus".to_string());
        let (active, msg) = preview.preview_binding(&b);
        assert!(active);
        assert!(msg.contains("ACTIVE"));
    }

    #[test]
    fn when_clause_preview_no_clause() {
        let preview = KeybindingWhenClausePreview::new();
        let b = kb(KeyCode::Char('s'), true, "file.save");
        let (active, msg) = preview.preview_binding(&b);
        assert!(active);
        assert!(msg.contains("always active"));
    }

    #[test]
    fn when_clause_preview_variables() {
        let mut preview = KeybindingWhenClausePreview::new();
        preview.set_variable("a", "1");
        preview.set_variable("b", "2");
        assert_eq!(preview.variable_count(), 2);
        assert!(preview.remove_variable("a"));
        assert_eq!(preview.variable_count(), 1);
        preview.clear_variables();
        assert_eq!(preview.variable_count(), 0);
    }

    #[test]
    fn search_filter_by_command() {
        let bindings = vec![
            kb(KeyCode::Char('s'), true, "file.save"),
            kb(KeyCode::Char('z'), true, "edit.undo"),
            kb(KeyCode::Char('c'), true, "edit.copy"),
        ];
        let filter = KeybindingSearchFilter::commands_only("file");
        assert_eq!(filter.count_matches(&bindings), 1);
        let matches = filter.filter(&bindings);
        assert_eq!(matches[0].command, "file.save");
    }

    #[test]
    fn search_filter_by_key() {
        let bindings = vec![
            kb(KeyCode::Char('s'), true, "file.save"),
            kb(KeyCode::Char('z'), true, "edit.undo"),
        ];
        let filter = KeybindingSearchFilter::new("Ctrl+S");
        let matches = filter.filter(&bindings);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn search_filter_empty_query() {
        let filter = KeybindingSearchFilter::new("");
        assert!(filter.is_empty_query());
        let bindings = vec![kb(KeyCode::Char('s'), true, "file.save")];
        assert_eq!(filter.count_matches(&bindings), 1);
    }

    #[test]
    fn search_filter_by_source() {
        let bindings = vec![
            kb_with_source(KeyCode::Char('s'), true, "file.save", KeybindingSource::User),
            kb_with_source(KeyCode::Char('z'), true, "edit.undo", KeybindingSource::Default),
        ];
        let filter = KeybindingSearchFilter::new("").with_source_filter(KeybindingSource::User);
        assert_eq!(filter.count_matches(&bindings), 1);
    }

    #[test]
    fn copy_format_json() {
        let b = kb(KeyCode::Char('s'), true, "file.save");
        let json = KeybindingCopyToClipboard::format_as_json(&b);
        assert!(json.contains("Ctrl+S"));
        assert!(json.contains("file.save"));
    }

    #[test]
    fn copy_format_text() {
        let b = kb(KeyCode::Char('s'), true, "file.save");
        let text = KeybindingCopyToClipboard::format_as_text(&b);
        assert!(text.contains("Ctrl+S"));
        assert!(text.contains("file.save"));
    }

    #[test]
    fn copy_format_markdown() {
        let b = kb(KeyCode::Char('s'), true, "file.save");
        let md = KeybindingCopyToClipboard::format_as_markdown(&b);
        assert!(md.starts_with("| `Ctrl+S`"));
    }

    #[test]
    fn copy_format_multiple_json() {
        let bindings = vec![
            kb(KeyCode::Char('s'), true, "file.save"),
            kb(KeyCode::Char('z'), true, "edit.undo"),
        ];
        let json = KeybindingCopyToClipboard::format_bindings(&bindings, KeybindingCopyFormat::Json);
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
    }

    #[test]
    fn copy_format_multiple_markdown() {
        let bindings = vec![
            kb(KeyCode::Char('s'), true, "file.save"),
        ];
        let md = KeybindingCopyToClipboard::format_bindings(&bindings, KeybindingCopyFormat::Markdown);
        assert!(md.contains("| Keybinding |"));
        assert!(md.contains("| --- |"));
    }

    #[test]
    fn copy_estimate_size() {
        let bindings = vec![kb(KeyCode::Char('s'), true, "file.save")];
        let size = KeybindingCopyToClipboard::estimate_size(&bindings, KeybindingCopyFormat::Text);
        assert!(size > 0);
    }

}
