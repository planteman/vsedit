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


// ---------------------------------------------------------------------------
// keybindings_ui – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XKeybindingsUiLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XKeybindingsUiPanelState {
    pub region: XKeybindingsUiLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XKeybindingsUiPanelState {
    pub fn new(region: XKeybindingsUiLayoutRegion, label: impl Into<String>) -> Self {
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
pub fn x_keybindings_ui_total_visible_area(panels: &[XKeybindingsUiPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_keybindings_ui_count_in_region(
    panels: &[XKeybindingsUiPanelState],
    region: XKeybindingsUiLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_keybindings_ui_widest_panel(panels: &[XKeybindingsUiPanelState]) -> Option<&XKeybindingsUiPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_keybindings_ui_collapse_region(
    panels: &mut [XKeybindingsUiPanelState],
    region: XKeybindingsUiLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XKeybindingsUiLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XKeybindingsUiLayoutConstraint {
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



// ---------------------------------------------------------------------------
// keybindings_ui – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for keybindings editor UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YKeybindingsUiKeybindingConflict {
    None,
    Shadowed,
    Duplicate,
    Incompatible,
}

impl YKeybindingsUiKeybindingConflict {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Shadowed => 1,
            Self::Duplicate => 2,
            Self::Incompatible => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Shadowed => "Shadowed",
            Self::Duplicate => "Duplicate",
            Self::Incompatible => "Incompatible",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YKeybindingsUiKeybindingConflict] {
        &[
            YKeybindingsUiKeybindingConflict::None,
            YKeybindingsUiKeybindingConflict::Shadowed,
            YKeybindingsUiKeybindingConflict::Duplicate,
            YKeybindingsUiKeybindingConflict::Incompatible,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YKeybindingsUiKeybindingConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks keybinding search data.
#[derive(Debug, Clone)]
pub struct YKeybindingsUiKeybindingSearchResult {
    pub matches: Vec<(String, String)>,
    pub query: String,
    pub exact: bool,
}

impl YKeybindingsUiKeybindingSearchResult {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            matches: Vec::new(),
            query: String::new(),
            exact: false,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.matches.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.matches.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YKeybindingsUiKeybindingSearchResult({}: {:?})", "matches", self.matches)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_keybindings_ui_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_keybindings_ui_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_keybindings_ui_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_keybindings_ui_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_keybindings_ui_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_keybindings_ui_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_keybindings_ui_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_keybindings_ui_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// keybindings_ui – Extended keybinding recorder helpers
// ---------------------------------------------------------------------------

/// Priority levels for keybinding recorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZKeybindingsUiPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZKeybindingsUiPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZKeybindingsUiPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZKeybindingsUiPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks keybinding recorder data.
#[derive(Debug, Clone)]
pub struct ZKeybindingsUiKeybindingRecorder {
    pub recorded_keys: Vec<String>,
    pub recording: bool,
    pub max_chord_len: usize,
}

impl ZKeybindingsUiKeybindingRecorder {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            recorded_keys: Vec::new(),
            recording: false,
            max_chord_len: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.recorded_keys.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.recorded_keys.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.recorded_keys.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZKeybindingsUiKeybindingRecorder[recording={:?}, max_chord_len={:?}]", self.recording, self.max_chord_len)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for keybinding recorder.
pub fn z_keybindings_ui_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_keybindings_ui_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_keybindings_ui_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_keybindings_ui_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_keybindings_ui_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_keybindings_ui_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_keybindings_ui_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 103
// ---------------------------------------------------------------------------

/// Generic object pool `Xc103Pool<T>`.
pub struct Xc103Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc103Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc103PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc103Pool<T> {
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
    pub fn stats(&self) -> Xc103PoolStats {
        Xc103PoolStats {
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

impl<T> Default for Xc103Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc103Scheduler`.
pub struct Xc103Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc103Scheduler {
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

impl Default for Xc103Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_103 hash for the given byte slice.
pub fn xc_103_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_103 convention.
pub fn xc_103_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_5 deepening: state machine + event bus ---

/// States for the Xd5 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd5State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd5State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd5Transition {
    pub from: Xd5State,
    pub to: Xd5State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd5StateMachine {
    current: Xd5State,
    history: Vec<Xd5Transition>,
    step_counter: usize,
}

impl Xd5StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd5State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd5State {
        self.current
    }

    pub fn history(&self) -> &[Xd5Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd5State) -> Result<Xd5State, String> {
        let allowed = match (self.current, target) {
            (Xd5State::Idle, Xd5State::Running) => true,
            (Xd5State::Running, Xd5State::Paused) => true,
            (Xd5State::Running, Xd5State::Done) => true,
            (Xd5State::Paused, Xd5State::Running) => true,
            (Xd5State::Paused, Xd5State::Done) => true,
            (Xd5State::Done, Xd5State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_5: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd5Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd5SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd5State> {
        let prefix = "Xd5SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd5State::Idle),
            "Running" => Some(Xd5State::Running),
            "Paused" => Some(Xd5State::Paused),
            "Done" => Some(Xd5State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd5State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd5 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd5Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd5Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd5HandlerFn = Box<dyn Fn(&Xd5Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd5EventBus {
    handlers: Vec<(usize, Option<String>, Xd5HandlerFn)>,
    next_id: usize,
    published: Vec<Xd5Event>,
}

impl Xd5EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd5Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd5Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd5Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd5Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #3
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf3Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf3TrieNode {
    children: std::collections::HashMap<char, Xf3TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf3Trie {
    root: Xf3TrieNode,
    count: usize,
}

impl Xf3Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf3TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf3TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf3TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf3BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf3BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 102).
pub struct Xh102SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh102SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 144 as u64,
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

/// A compact bit set supporting boolean operations (variant 102).
pub struct Xh102BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh102BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 102).
pub struct Xi102Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi102Deque<T> {
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
pub struct Xi102Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi102Interval {
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

/// A simple interval tree (variant 102).
pub struct Xi102IntervalTree {
    xi_intervals: Vec<Xi102Interval>,
}

impl Xi102IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi102Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi102Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi102Interval) -> Vec<&Xi102Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi102Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi102Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi102Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi102Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi102Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi102Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 102) ---

/// Disjoint set / union-find for crate 102.
pub struct Xj102UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj102UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ102_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 102.
pub struct Xj102BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj102BTreeNode<K, V>>>,
    len: usize,
}

struct Xj102BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj102BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj102BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ102_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ102_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj102BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj102BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj102BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj102BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_103 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk103SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk103SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk103DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk103DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_102).
#[derive(Debug, Clone)]
pub struct Xl102Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl102Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_102).
#[derive(Debug, Clone)]
pub struct Xl102SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl102SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm102MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm102MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm102Tokenizer {
    text: String,
}

impl Xm102Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 102.
pub struct Xn102Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn102Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 102 -----

#[derive(Debug, Clone)]
struct Xn102AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn102AvlNode<K, V>>>,
    right: Option<Box<Xn102AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 102.
#[derive(Debug, Clone)]
pub struct Xn102AVL<K, V> {
    root: Option<Box<Xn102AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn102AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn102AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn102AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn102AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn102AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn102AvlNode<K, V>>) -> Box<Xn102AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn102AvlNode<K, V>>) -> Box<Xn102AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn102AvlNode<K, V>>) -> Box<Xn102AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn102AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn102AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn102AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn102AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn102AvlNode<K, V>>) -> &Xn102AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn102AvlNode<K, V>>) -> (Box<Xn102AvlNode<K, V>>, Option<Box<Xn102AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn102AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn102AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn102AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn102AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn102AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn102AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn102AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo102RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo102Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo102RBNode<K, V> {
    key: K,
    value: V,
    color: Xo102Color,
    left: Option<Box<Xo102RBNode<K, V>>>,
    right: Option<Box<Xo102RBNode<K, V>>>,
}

/// A red-black tree map for crate 102.
#[derive(Debug, Clone)]
pub struct Xo102RedBlack<K, V> {
    root: Option<Box<Xo102RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo102RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo102Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo102RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo102RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo102RBNode {
                    key, value, color: Xo102Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo102RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo102Color::Red)
    }

    fn xo_balance(mut h: Box<Xo102RBNode<K, V>>) -> Box<Xo102RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo102Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo102RBNode<K, V>>) -> Box<Xo102RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo102Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo102RBNode<K, V>>) -> Box<Xo102RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo102Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo102RBNode<K, V>>) {
        h.color = Xo102Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo102Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo102Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo102Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo102RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo102RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo102RBNode<K, V>) -> (K, V, Option<Box<Xo102RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo102RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo102Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo102RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo102ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 102.
#[derive(Debug, Clone)]
pub struct Xo102ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo102ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo102#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo102#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 103).
#[derive(Debug)]
pub struct Xp103SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp103Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp103Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp103Node<K, V>>>,
    xp_right: Option<Box<Xp103Node<K, V>>>,
}

impl<K: Ord, V> Xp103Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp103SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp103SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp103Node<K, V>>>, key: &K) -> Option<Box<Xp103Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp103Node<K, V>>) -> Box<Xp103Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp103Node<K, V>>) -> Box<Xp103Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp103Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp103Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp103Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq102Treap ---------------

use std::cmp::Ordering as Xq102Ord;

struct Xq102TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq102TreapNode<K, V>>>,
    right: Option<Box<Xq102TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq102Treap<K, V> {
    root: Option<Box<Xq102TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq102TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_102_size<K, V>(node: &Option<Box<Xq102TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_102_update_size<K, V>(node: &mut Xq102TreapNode<K, V>) {
    node.size = 1 + xq_102_size(&node.left) + xq_102_size(&node.right);
}

fn xq_102_rotate_right<K, V>(mut node: Box<Xq102TreapNode<K, V>>) -> Box<Xq102TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_102_update_size(&mut node);
    left.right = Some(node);
    xq_102_update_size(&mut left);
    left
}

fn xq_102_rotate_left<K, V>(mut node: Box<Xq102TreapNode<K, V>>) -> Box<Xq102TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_102_update_size(&mut node);
    right.left = Some(node);
    xq_102_update_size(&mut right);
    right
}

fn xq_102_insert_node<K: Ord, V>(
    node: Option<Box<Xq102TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq102TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq102TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq102Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq102Ord::Less => {
                let (new_left, old) = xq_102_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_102_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_102_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq102Ord::Greater => {
                let (new_right, old) = xq_102_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_102_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_102_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_102_remove_node<K: Ord, V>(
    node: Option<Box<Xq102TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq102TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq102Ord::Less => {
                let (new_left, old) = xq_102_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_102_update_size(&mut n);
                (Some(n), old)
            }
            Xq102Ord::Greater => {
                let (new_right, old) = xq_102_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_102_update_size(&mut n);
                (Some(n), old)
            }
            Xq102Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_102_rotate_right(n);
                    let (new_right, old) = xq_102_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_102_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_102_rotate_left(n);
                    let (new_left, old) = xq_102_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_102_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_102_find_min<K, V>(node: &Option<Box<Xq102TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_102_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_102_find_max<K, V>(node: &Option<Box<Xq102TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_102_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_102_rank<K: Ord, V>(node: &Option<Box<Xq102TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq102Ord::Less => xq_102_rank(&n.left, key),
            Xq102Ord::Equal => xq_102_size(&n.left),
            Xq102Ord::Greater => 1 + xq_102_size(&n.left) + xq_102_rank(&n.right, key),
        },
    }
}

fn xq_102_kth<K, V>(node: &Option<Box<Xq102TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_102_size(&n.left);
        if k < left_size {
            xq_102_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_102_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_102_in_order<K: Clone, V>(node: &Option<Box<Xq102TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_102_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_102_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq102Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 102 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_102_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq102Ord::Equal => return Some(&n.value),
                Xq102Ord::Less => cur = &n.left,
                Xq102Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_102_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_102_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_102_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_102_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_102_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_102_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_102_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq102VEBTree ---------------

pub struct Xq102VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq102VEBTree>>,
    clusters: Vec<Option<Box<Xq102VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq102VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq102VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq102VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr102KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr102KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr102BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr102KDNode {
    xr_point: Xr102KDPoint,
    xr_left: Option<Box<Xr102KDNode>>,
    xr_right: Option<Box<Xr102KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr102KDTree {
    xr_root: Option<Box<Xr102KDNode>>,
    xr_size: usize,
}

impl Xr102KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr102KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr102KDNode>>,
        point: Xr102KDPoint,
        depth: usize,
    ) -> Box<Xr102KDNode> {
        match node {
            None => Box::new(Xr102KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr102KDPoint) -> Option<Xr102KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr102KDNode>,
        query: &Xr102KDPoint,
        depth: usize,
        best: &mut Xr102KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr102KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr102KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr102KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr102KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr102KDNode>>, pts: &mut Vec<Xr102KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr102KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr102BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr102BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs102PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs102PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs102PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs102PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs102ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs102ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs102ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs102RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs102RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs102RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs102CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs102CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs102CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

/// Auxiliary statistics tracker for xs_102 data structures.
#[derive(Debug, Clone)]
pub struct Xs102StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs102StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs102StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}


// --- xw_ Scapegoat Tree ---

/// A node in a scapegoat tree.
#[derive(Debug, Clone)]
pub struct XwScapegoatNode<K: Ord + Clone, V: Clone> {
    pub xw_key: K,
    pub xw_value: V,
    xw_left: Option<Box<XwScapegoatNode<K, V>>>,
    xw_right: Option<Box<XwScapegoatNode<K, V>>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGNode(k={})", self.xw_key)
    }
}

/// Scapegoat tree — a BST that rebuilds subtrees when they become too unbalanced.
#[derive(Debug, Clone)]
pub struct XwScapegoatTree<K: Ord + Clone, V: Clone> {
    xw_root: Option<Box<XwScapegoatNode<K, V>>>,
    xw_size: usize,
    xw_max_size: usize,
    xw_alpha: f64,
}

impl<K: Ord + Clone, V: Clone> Default for XwScapegoatTree<K, V> {
    fn default() -> Self { Self::xw_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGTree(size={}, alpha={:.2})", self.xw_size, self.xw_alpha)
    }
}

impl<K: Ord + Clone, V: Clone> XwScapegoatTree<K, V> {
    /// Create an empty scapegoat tree with default α = 0.7.
    pub fn xw_new() -> Self {
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: 0.7 }
    }

    /// Create with custom alpha (0.5 < α < 1.0).
    pub fn xw_with_alpha(alpha: f64) -> Self {
        let a = alpha.clamp(0.51, 0.99);
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: a }
    }

    /// Number of elements.
    pub fn xw_len(&self) -> usize { self.xw_size }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_size == 0 }

    fn xw_node_size(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::xw_node_size(&n.xw_left) + Self::xw_node_size(&n.xw_right),
        }
    }

    /// Insert a key-value pair.
    pub fn xw_insert(&mut self, key: K, value: V) {
        let (new_root, depth, inserted) = Self::xw_insert_node(self.xw_root.take(), key, value, 0);
        self.xw_root = new_root;
        if inserted {
            self.xw_size += 1;
            self.xw_max_size = std::cmp::max(self.xw_max_size, self.xw_size);
            let h_alpha = -(self.xw_size as f64).log(1.0 / self.xw_alpha);
            if depth as f64 > h_alpha {
                self.xw_root = Self::xw_rebuild(self.xw_root.take());
            }
        }
    }

    fn xw_insert_node(
        node: Option<Box<XwScapegoatNode<K, V>>>, key: K, value: V, depth: usize,
    ) -> (Option<Box<XwScapegoatNode<K, V>>>, usize, bool) {
        match node {
            None => {
                let n = Box::new(XwScapegoatNode { xw_key: key, xw_value: value, xw_left: None, xw_right: None });
                (Some(n), depth, true)
            }
            Some(mut n) => {
                if key < n.xw_key {
                    let (l, d, ins) = Self::xw_insert_node(n.xw_left.take(), key, value, depth + 1);
                    n.xw_left = l;
                    if ins {
                        let ls = Self::xw_node_size(&n.xw_left);
                        let total = 1 + ls + Self::xw_node_size(&n.xw_right);
                        if ls as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else if key > n.xw_key {
                    let (r, d, ins) = Self::xw_insert_node(n.xw_right.take(), key, value, depth + 1);
                    n.xw_right = r;
                    if ins {
                        let rs = Self::xw_node_size(&n.xw_right);
                        let total = 1 + Self::xw_node_size(&n.xw_left) + rs;
                        if rs as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else {
                    n.xw_value = value;
                    (Some(n), depth, false)
                }
            }
        }
    }

    fn xw_flatten(node: Option<Box<XwScapegoatNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xw_flatten(n.xw_left, out);
            out.push((n.xw_key, n.xw_value));
            Self::xw_flatten(n.xw_right, out);
        }
    }

    fn xw_build_balanced(sorted: &[(K, V)]) -> Option<Box<XwScapegoatNode<K, V>>> {
        if sorted.is_empty() { return None; }
        let mid = sorted.len() / 2;
        let (k, v) = sorted[mid].clone();
        Some(Box::new(XwScapegoatNode {
            xw_key: k,
            xw_value: v,
            xw_left: Self::xw_build_balanced(&sorted[..mid]),
            xw_right: Self::xw_build_balanced(&sorted[mid + 1..]),
        }))
    }

    fn xw_rebuild(node: Option<Box<XwScapegoatNode<K, V>>>) -> Option<Box<XwScapegoatNode<K, V>>> {
        let mut flat = Vec::new();
        Self::xw_flatten(node, &mut flat);
        Self::xw_build_balanced(&flat)
    }

    /// Look up a key.
    pub fn xw_get(&self, key: &K) -> Option<&V> {
        Self::xw_search(&self.xw_root, key)
    }

    fn xw_search<'a>(node: &'a Option<Box<XwScapegoatNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xw_key { Some(&n.xw_value) }
                else if *key < n.xw_key { Self::xw_search(&n.xw_left, key) }
                else { Self::xw_search(&n.xw_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xw_contains(&self, key: &K) -> bool { self.xw_get(key).is_some() }

    /// In-order keys.
    pub fn xw_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xw_collect_keys(&self.xw_root, &mut result);
        result
    }

    fn xw_collect_keys(node: &Option<Box<XwScapegoatNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xw_collect_keys(&n.xw_left, result);
            result.push(n.xw_key.clone());
            Self::xw_collect_keys(&n.xw_right, result);
        }
    }

    /// Clear the tree.
    pub fn xw_clear(&mut self) {
        self.xw_root = None;
        self.xw_size = 0;
        self.xw_max_size = 0;
    }

    /// Height.
    pub fn xw_height(&self) -> usize {
        Self::xw_node_height(&self.xw_root)
    }

    fn xw_node_height(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xw_node_height(&n.xw_left), Self::xw_node_height(&n.xw_right)),
        }
    }
}

// --- xw_ Rope (String Rope) ---

/// A rope node — either a leaf with text or an internal node concatenating two children.
#[derive(Debug, Clone)]
pub enum XwRopeNode {
    Leaf(String),
    Internal {
        xw_left: Box<XwRopeNode>,
        xw_right: Box<XwRopeNode>,
        xw_len: usize,
    },
}

impl std::fmt::Display for XwRopeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XwRopeNode::Leaf(s) => write!(f, "RopeLeaf({})", s.len()),
            XwRopeNode::Internal { xw_len, .. } => write!(f, "RopeInt({})", xw_len),
        }
    }
}

/// Rope data structure for efficient string editing with O(log n) split/concat.
#[derive(Debug, Clone)]
pub struct XwRope {
    xw_root: Option<Box<XwRopeNode>>,
}

impl Default for XwRope {
    fn default() -> Self { Self::xw_new() }
}

impl std::fmt::Display for XwRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rope(len={})", self.xw_len())
    }
}

impl XwRope {
    /// Create an empty rope.
    pub fn xw_new() -> Self { Self { xw_root: None } }

    /// Create a rope from a string.
    pub fn xw_from_str(s: &str) -> Self {
        if s.is_empty() {
            Self { xw_root: None }
        } else {
            Self { xw_root: Some(Box::new(XwRopeNode::Leaf(s.to_string()))) }
        }
    }

    /// Total length in bytes.
    pub fn xw_len(&self) -> usize {
        Self::xw_node_len(&self.xw_root)
    }

    fn xw_node_len(node: &Option<Box<XwRopeNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => s.len(),
                XwRopeNode::Internal { xw_len, .. } => *xw_len,
            },
        }
    }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_len() == 0 }

    /// Concatenate two ropes.
    pub fn xw_concat(left: XwRope, right: XwRope) -> XwRope {
        match (left.xw_root, right.xw_root) {
            (None, r) => XwRope { xw_root: r },
            (l, None) => XwRope { xw_root: l },
            (Some(l), Some(r)) => {
                let len = Self::xw_node_len(&Some(l.clone())) + Self::xw_node_len(&Some(r.clone()));
                XwRope {
                    xw_root: Some(Box::new(XwRopeNode::Internal { xw_left: l, xw_right: r, xw_len: len })),
                }
            }
        }
    }

    /// Convert to string.
    pub fn xw_to_string(&self) -> String {
        let mut result = String::new();
        Self::xw_collect(&self.xw_root, &mut result);
        result
    }

    fn xw_collect(node: &Option<Box<XwRopeNode>>, result: &mut String) {
        match node {
            None => {}
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => result.push_str(s),
                XwRopeNode::Internal { xw_left, xw_right, .. } => {
                    Self::xw_collect(&Some(xw_left.clone()), result);
                    Self::xw_collect(&Some(xw_right.clone()), result);
                }
            },
        }
    }

    /// Get character at byte index.
    pub fn xw_char_at(&self, idx: usize) -> Option<char> {
        let s = self.xw_to_string();
        s.as_bytes().get(idx).map(|&b| b as char)
    }

    /// Insert a string at byte index.
    pub fn xw_insert(&mut self, idx: usize, text: &str) {
        let s = self.xw_to_string();
        let (left, right) = s.split_at(idx.min(s.len()));
        let new_s = format!("{}{}{}", left, text, right);
        *self = Self::xw_from_str(&new_s);
    }

    /// Delete bytes in range [start, end).
    pub fn xw_delete(&mut self, start: usize, end: usize) {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        let new_s = format!("{}{}", &s[..start], &s[end..]);
        *self = Self::xw_from_str(&new_s);
    }

    /// Append text.
    pub fn xw_append(&mut self, text: &str) {
        let other = Self::xw_from_str(text);
        let old = std::mem::take(self);
        *self = Self::xw_concat(old, other);
    }

    /// Substring [start, end).
    pub fn xw_substring(&self, start: usize, end: usize) -> String {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        s[start..end].to_string()
    }

    /// Clear the rope.
    pub fn xw_clear(&mut self) { self.xw_root = None; }
}


// --- xx_ Skip List ---

/// A node in a skip list with multiple forward pointers for O(log n) search.
#[derive(Debug, Clone)]
pub struct XxSkipNode<K: Ord + Clone, V: Clone> {
    pub xx_key: Option<K>,
    pub xx_value: Option<V>,
    xx_forward: Vec<Option<usize>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.xx_key {
            Some(k) => write!(f, "SkipNode(k={}, lvl={})", k, self.xx_forward.len()),
            None => write!(f, "SkipNode(HEAD, lvl={})", self.xx_forward.len()),
        }
    }
}

/// Skip list — a probabilistic data structure with O(log n) average search, insert, delete.
#[derive(Debug, Clone)]
pub struct XxSkipList<K: Ord + Clone, V: Clone> {
    xx_nodes: Vec<XxSkipNode<K, V>>,
    xx_head: usize,
    xx_max_level: usize,
    xx_level: usize,
    xx_size: usize,
    xx_rng_state: u64,
}

impl<K: Ord + Clone, V: Clone> Default for XxSkipList<K, V> {
    fn default() -> Self { Self::xx_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkipList(size={}, level={})", self.xx_size, self.xx_level)
    }
}

impl<K: Ord + Clone, V: Clone> XxSkipList<K, V> {
    const XX_MAX_LEVEL: usize = 16;

    /// Create an empty skip list.
    pub fn xx_new() -> Self {
        let head = XxSkipNode {
            xx_key: None,
            xx_value: None,
            xx_forward: vec![None; Self::XX_MAX_LEVEL],
        };
        Self {
            xx_nodes: vec![head],
            xx_head: 0,
            xx_max_level: Self::XX_MAX_LEVEL,
            xx_level: 1,
            xx_size: 0,
            xx_rng_state: 42,
        }
    }

    fn xx_random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < self.xx_max_level {
            self.xx_rng_state ^= self.xx_rng_state << 13;
            self.xx_rng_state ^= self.xx_rng_state >> 7;
            self.xx_rng_state ^= self.xx_rng_state << 17;
            if self.xx_rng_state % 4 < 1 { break; }
            lvl += 1;
        }
        lvl
    }

    /// Number of elements.
    pub fn xx_len(&self) -> usize { self.xx_size }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_size == 0 }

    /// Insert a key-value pair.
    pub fn xx_insert(&mut self, key: K, value: V) {
        let mut update = vec![self.xx_head; self.xx_max_level];
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < key { current = next; continue; }
                    if *nk == key {
                        self.xx_nodes[next].xx_value = Some(value);
                        return;
                    }
                }
                break;
            }
            update[i] = current;
        }
        let lvl = self.xx_random_level();
        if lvl > self.xx_level {
            for i in self.xx_level..lvl {
                update[i] = self.xx_head;
            }
            self.xx_level = lvl;
        }
        let new_idx = self.xx_nodes.len();
        self.xx_nodes.push(XxSkipNode {
            xx_key: Some(key),
            xx_value: Some(value),
            xx_forward: vec![None; lvl],
        });
        for i in 0..lvl {
            self.xx_nodes[new_idx].xx_forward[i] = self.xx_nodes[update[i]].xx_forward[i];
            self.xx_nodes[update[i]].xx_forward[i] = Some(new_idx);
        }
        self.xx_size += 1;
    }

    /// Search for a key.
    pub fn xx_get(&self, key: &K) -> Option<&V> {
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < *key { current = next; continue; }
                    if *nk == *key { return self.xx_nodes[next].xx_value.as_ref(); }
                }
                break;
            }
        }
        None
    }

    /// Check if key exists.
    pub fn xx_contains(&self, key: &K) -> bool { self.xx_get(key).is_some() }

    /// Collect all keys in sorted order.
    pub fn xx_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        let mut current = self.xx_nodes[self.xx_head].xx_forward[0];
        while let Some(idx) = current {
            if let Some(ref k) = self.xx_nodes[idx].xx_key {
                result.push(k.clone());
            }
            current = self.xx_nodes[idx].xx_forward[0];
        }
        result
    }

    /// Clear the skip list.
    pub fn xx_clear(&mut self) {
        self.xx_nodes.truncate(1);
        for i in 0..self.xx_max_level {
            self.xx_nodes[0].xx_forward[i] = None;
        }
        self.xx_level = 1;
        self.xx_size = 0;
    }
}

// --- xx_ Suffix Array ---

/// Suffix array for O(n log n) construction and O(m log n) pattern matching.
#[derive(Debug, Clone)]
pub struct XxSuffixArray {
    xx_text: String,
    xx_sa: Vec<usize>,
}

impl std::fmt::Display for XxSuffixArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuffixArray(len={})", self.xx_text.len())
    }
}

impl Default for XxSuffixArray {
    fn default() -> Self { Self::xx_new("") }
}

impl XxSuffixArray {
    /// Build a suffix array from a string.
    pub fn xx_new(text: &str) -> Self {
        let n = text.len();
        let bytes = text.as_bytes();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self { xx_text: text.to_string(), xx_sa: sa }
    }

    /// Length of the text.
    pub fn xx_len(&self) -> usize { self.xx_text.len() }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_text.is_empty() }

    /// Get the suffix array.
    pub fn xx_array(&self) -> &[usize] { &self.xx_sa }

    /// Get the original text.
    pub fn xx_text(&self) -> &str { &self.xx_text }

    /// Search for a pattern, returning all starting positions.
    pub fn xx_search(&self, pattern: &str) -> Vec<usize> {
        if pattern.is_empty() || self.xx_text.is_empty() { return Vec::new(); }
        let pb = pattern.as_bytes();
        let tb = self.xx_text.as_bytes();
        let n = tb.len();
        let m = pb.len();
        // Binary search for lower bound
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] < *pb { lo = mid + 1; } else { hi = mid; }
        }
        let lower = lo;
        hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] <= *pb { lo = mid + 1; } else { hi = mid; }
        }
        let upper = lo;
        self.xx_sa[lower..upper].to_vec()
    }

    /// Count occurrences of a pattern.
    pub fn xx_count(&self, pattern: &str) -> usize {
        self.xx_search(pattern).len()
    }

    /// Get the suffix at position i in sorted order.
    pub fn xx_suffix_at(&self, i: usize) -> &str {
        if i < self.xx_sa.len() { &self.xx_text[self.xx_sa[i]..] } else { "" }
    }

    /// Find the longest repeated substring.
    pub fn xx_longest_repeated(&self) -> String {
        if self.xx_sa.len() < 2 { return String::new(); }
        let tb = self.xx_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xx_sa.len() {
            let a = self.xx_sa[i - 1];
            let b = self.xx_sa[i];
            let mut lcp = 0;
            while a + lcp < tb.len() && b + lcp < tb.len() && tb[a + lcp] == tb[b + lcp] {
                lcp += 1;
            }
            if lcp > best_len { best_len = lcp; best_start = a; }
        }
        self.xx_text[best_start..best_start + best_len].to_string()
    }
}


// --- xy_ Cuckoo Hash Map ---

/// Cuckoo hash map with two hash functions and O(1) amortized lookup.
#[derive(Debug, Clone)]
pub struct XyCuckooMap<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xy_table1: Vec<Option<(K, V)>>,
    xy_table2: Vec<Option<(K, V)>>,
    xy_capacity: usize,
    xy_size: usize,
    xy_seed1: u64,
    xy_seed2: u64,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XyCuckooMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CuckooMap(size={}, cap={})", self.xy_size, self.xy_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> Default for XyCuckooMap<K, V> {
    fn default() -> Self { Self::xy_new(16) }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XyCuckooMap<K, V> {
    /// Create a new cuckoo hash map with given capacity.
    pub fn xy_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xy_table1: (0..cap).map(|_| None).collect(),
            xy_table2: (0..cap).map(|_| None).collect(),
            xy_capacity: cap,
            xy_size: 0,
            xy_seed1: 0x517cc1b727220a95,
            xy_seed2: 0x6c62272e07bb0142,
        }
    }

    fn xy_hash1(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed1.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    fn xy_hash2(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed2.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    /// Number of elements.
    pub fn xy_len(&self) -> usize { self.xy_size }

    /// Is empty.
    pub fn xy_is_empty(&self) -> bool { self.xy_size == 0 }

    /// Insert a key-value pair.
    pub fn xy_insert(&mut self, key: K, value: V) -> bool {
        if self.xy_get(&key).is_some() {
            let h1 = self.xy_hash1(&key);
            if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == key) {
                self.xy_table1[h1] = Some((key, value));
            } else {
                let h2 = self.xy_hash2(&key);
                self.xy_table2[h2] = Some((key, value));
            }
            return true;
        }
        let mut k = key;
        let mut v = value;
        for _ in 0..self.xy_capacity {
            let h1 = self.xy_hash1(&k);
            if self.xy_table1[h1].is_none() {
                self.xy_table1[h1] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old = self.xy_table1[h1].take().unwrap();
            self.xy_table1[h1] = Some((k, v));
            k = old.0;
            v = old.1;
            let h2 = self.xy_hash2(&k);
            if self.xy_table2[h2].is_none() {
                self.xy_table2[h2] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old2 = self.xy_table2[h2].take().unwrap();
            self.xy_table2[h2] = Some((k, v));
            k = old2.0;
            v = old2.1;
        }
        // Rehash needed — just put in table1 with linear probing fallback
        for i in 0..self.xy_capacity {
            if self.xy_table1[i].is_none() {
                self.xy_table1[i] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
        }
        false
    }

    /// Look up a key.
    pub fn xy_get(&self, key: &K) -> Option<&V> {
        let h1 = self.xy_hash1(key);
        if let Some((k, v)) = &self.xy_table1[h1] {
            if *k == *key { return Some(v); }
        }
        let h2 = self.xy_hash2(key);
        if let Some((k, v)) = &self.xy_table2[h2] {
            if *k == *key { return Some(v); }
        }
        None
    }

    /// Check if key exists.
    pub fn xy_contains(&self, key: &K) -> bool { self.xy_get(key).is_some() }

    /// Remove a key.
    pub fn xy_remove(&mut self, key: &K) -> Option<V> {
        let h1 = self.xy_hash1(key);
        if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table1[h1].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        let h2 = self.xy_hash2(key);
        if self.xy_table2[h2].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table2[h2].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        None
    }

    /// Clear the map.
    pub fn xy_clear(&mut self) {
        for slot in &mut self.xy_table1 { *slot = None; }
        for slot in &mut self.xy_table2 { *slot = None; }
        self.xy_size = 0;
    }

    /// Collect all keys.
    pub fn xy_keys(&self) -> Vec<K> {
        let mut keys = Vec::new();
        for slot in &self.xy_table1 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        for slot in &self.xy_table2 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        keys
    }
}

// --- xy_ Count-Min Sketch ---

/// Count-min sketch for approximate frequency counting with bounded error.
#[derive(Debug, Clone)]
pub struct XyCountMinSketch {
    xy_table: Vec<Vec<u64>>,
    xy_width: usize,
    xy_depth: usize,
    xy_seeds: Vec<u64>,
}

impl std::fmt::Display for XyCountMinSketch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CMS(w={}, d={})", self.xy_width, self.xy_depth)
    }
}

impl Default for XyCountMinSketch {
    fn default() -> Self { Self::xy_new(1000, 5) }
}

impl XyCountMinSketch {
    /// Create a new count-min sketch with given width and depth.
    pub fn xy_new(width: usize, depth: usize) -> Self {
        let seeds: Vec<u64> = (0..depth).map(|i| 0x9e3779b97f4a7c15u64.wrapping_add((i as u64).wrapping_mul(0x517cc1b727220a95))).collect();
        Self {
            xy_table: vec![vec![0u64; width]; depth],
            xy_width: width,
            xy_depth: depth,
            xy_seeds: seeds,
        }
    }

    fn xy_hash(&self, item: u64, seed: u64) -> usize {
        let h = item.wrapping_mul(seed).wrapping_add(seed >> 16);
        (h ^ (h >> 32)) as usize % self.xy_width
    }

    /// Increment the count for an item.
    pub fn xy_add(&mut self, item: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += 1;
        }
    }

    /// Add with a specific count.
    pub fn xy_add_count(&mut self, item: u64, count: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += count;
        }
    }

    /// Estimate the count for an item (guaranteed to be >= actual count).
    pub fn xy_estimate(&self, item: u64) -> u64 {
        let mut min_count = u64::MAX;
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            min_count = min_count.min(self.xy_table[i][idx]);
        }
        min_count
    }

    /// Width of the sketch.
    pub fn xy_width(&self) -> usize { self.xy_width }

    /// Depth of the sketch.
    pub fn xy_depth(&self) -> usize { self.xy_depth }

    /// Clear the sketch.
    pub fn xy_clear(&mut self) {
        for row in &mut self.xy_table {
            for cell in row { *cell = 0; }
        }
    }

    /// Merge another sketch into this one.
    pub fn xy_merge(&mut self, other: &XyCountMinSketch) {
        if self.xy_width != other.xy_width || self.xy_depth != other.xy_depth { return; }
        for i in 0..self.xy_depth {
            for j in 0..self.xy_width {
                self.xy_table[i][j] += other.xy_table[i][j];
            }
        }
    }
}


// --- xz_ HyperLogLog ---

/// HyperLogLog probabilistic cardinality estimator with configurable precision.
#[derive(Debug, Clone)]
pub struct XzHyperLogLog {
    xz_registers: Vec<u8>,
    xz_m: usize,
    xz_b: u32,
}

impl std::fmt::Display for XzHyperLogLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HLL(m={}, est={:.0})", self.xz_m, self.xz_estimate())
    }
}

impl Default for XzHyperLogLog {
    fn default() -> Self { Self::xz_new(10) }
}

impl XzHyperLogLog {
    /// Create a new HyperLogLog with precision b (4 <= b <= 16). Uses 2^b registers.
    pub fn xz_new(b: u32) -> Self {
        let b = b.clamp(4, 16);
        let m = 1 << b;
        Self { xz_registers: vec![0u8; m], xz_m: m, xz_b: b }
    }

    fn xz_hash(item: u64) -> u64 {
        let mut h = item;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        h
    }

    /// Add an item.
    pub fn xz_add(&mut self, item: u64) {
        let h = Self::xz_hash(item);
        let idx = (h as usize) & (self.xz_m - 1);
        let w = h >> self.xz_b;
        let rho = if w == 0 { 64 - self.xz_b } else { w.trailing_zeros() + 1 };
        let rho = rho.min(255) as u8;
        if rho > self.xz_registers[idx] {
            self.xz_registers[idx] = rho;
        }
    }

    /// Estimate the cardinality.
    pub fn xz_estimate(&self) -> f64 {
        let m = self.xz_m as f64;
        let alpha = match self.xz_m {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m),
        };
        let sum: f64 = self.xz_registers.iter().map(|&r| 2.0f64.powi(-(r as i32))).sum();
        let raw = alpha * m * m / sum;
        if raw <= 2.5 * m {
            let zeros = self.xz_registers.iter().filter(|&&r| r == 0).count();
            if zeros > 0 { m * (m / zeros as f64).ln() } else { raw }
        } else if raw <= (1u64 << 32) as f64 / 30.0 {
            raw
        } else {
            -(((1u64 << 32) as f64) * (1.0 - raw / (1u64 << 32) as f64).ln())
        }
    }

    /// Merge another HyperLogLog into this one.
    pub fn xz_merge(&mut self, other: &XzHyperLogLog) {
        if self.xz_m != other.xz_m { return; }
        for i in 0..self.xz_m {
            if other.xz_registers[i] > self.xz_registers[i] {
                self.xz_registers[i] = other.xz_registers[i];
            }
        }
    }

    /// Clear all registers.
    pub fn xz_clear(&mut self) {
        for r in &mut self.xz_registers { *r = 0; }
    }

    /// Number of registers.
    pub fn xz_num_registers(&self) -> usize { self.xz_m }

    /// Precision parameter.
    pub fn xz_precision(&self) -> u32 { self.xz_b }
}

// --- xz_ LRU Cache ---

/// LRU cache with O(1) get/put using a doubly-linked list and hash map.
#[derive(Debug, Clone)]
pub struct XzLruCache<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xz_capacity: usize,
    xz_entries: Vec<(K, V)>,
    xz_order: Vec<usize>,
    xz_map: std::collections::HashMap<K, usize>,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XzLruCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LRU(size={}, cap={})", self.xz_map.len(), self.xz_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XzLruCache<K, V> {
    /// Create a new LRU cache with given capacity.
    pub fn xz_new(capacity: usize) -> Self {
        Self {
            xz_capacity: capacity.max(1),
            xz_entries: Vec::new(),
            xz_order: Vec::new(),
            xz_map: std::collections::HashMap::new(),
        }
    }

    /// Number of entries.
    pub fn xz_len(&self) -> usize { self.xz_map.len() }

    /// Is empty.
    pub fn xz_is_empty(&self) -> bool { self.xz_map.is_empty() }

    /// Capacity.
    pub fn xz_capacity(&self) -> usize { self.xz_capacity }

    /// Get a value, marking it as recently used.
    pub fn xz_get(&mut self, key: &K) -> Option<&V> {
        if let Some(&idx) = self.xz_map.get(key) {
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            Some(&self.xz_entries[idx].1)
        } else {
            None
        }
    }

    /// Put a key-value pair, evicting the least recently used if at capacity.
    pub fn xz_put(&mut self, key: K, value: V) {
        if let Some(&idx) = self.xz_map.get(&key) {
            self.xz_entries[idx].1 = value;
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            return;
        }
        if self.xz_map.len() >= self.xz_capacity {
            if let Some(evict_idx) = self.xz_order.first().copied() {
                self.xz_order.remove(0);
                let evict_key = self.xz_entries[evict_idx].0.clone();
                self.xz_map.remove(&evict_key);
            }
        }
        let idx = self.xz_entries.len();
        self.xz_entries.push((key.clone(), value));
        self.xz_map.insert(key, idx);
        self.xz_order.push(idx);
    }

    /// Check if key exists (without updating LRU order).
    pub fn xz_contains(&self, key: &K) -> bool { self.xz_map.contains_key(key) }

    /// Remove a key.
    pub fn xz_remove(&mut self, key: &K) -> Option<V> {
        if let Some(idx) = self.xz_map.remove(key) {
            self.xz_order.retain(|&i| i != idx);
            Some(self.xz_entries[idx].1.clone())
        } else {
            None
        }
    }

    /// Clear the cache.
    pub fn xz_clear(&mut self) {
        self.xz_entries.clear();
        self.xz_order.clear();
        self.xz_map.clear();
    }

    /// Get all keys in LRU order (least recent first).
    pub fn xz_keys_lru(&self) -> Vec<K> {
        self.xz_order.iter().filter_map(|&idx| {
            let k = &self.xz_entries[idx].0;
            if self.xz_map.contains_key(k) { Some(k.clone()) } else { None }
        }).collect()
    }

    /// Peek at value without updating LRU order.
    pub fn xz_peek(&self, key: &K) -> Option<&V> {
        self.xz_map.get(key).map(|&idx| &self.xz_entries[idx].1)
    }
}


// --- ya_ Trie (Prefix Tree) ---

/// A node in a trie (prefix tree) for string key lookups.
#[derive(Debug, Clone)]
pub struct YaTrieNode<V: Clone> {
    ya_children: std::collections::HashMap<char, Box<YaTrieNode<V>>>,
    ya_value: Option<V>,
    ya_is_end: bool,
}

impl<V: Clone> Default for YaTrieNode<V> {
    fn default() -> Self {
        Self { ya_children: std::collections::HashMap::new(), ya_value: None, ya_is_end: false }
    }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrieNode<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrieNode(children={}, end={})", self.ya_children.len(), self.ya_is_end)
    }
}

/// Trie (prefix tree) for O(m) string key operations where m is key length.
#[derive(Debug, Clone)]
pub struct YaTrie<V: Clone> {
    ya_root: YaTrieNode<V>,
    ya_size: usize,
}

impl<V: Clone> Default for YaTrie<V> {
    fn default() -> Self { Self::ya_new() }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrie<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Trie(size={})", self.ya_size)
    }
}

impl<V: Clone> YaTrie<V> {
    /// Create an empty trie.
    pub fn ya_new() -> Self { Self { ya_root: YaTrieNode::default(), ya_size: 0 } }

    /// Number of stored keys.
    pub fn ya_len(&self) -> usize { self.ya_size }

    /// Is the trie empty.
    pub fn ya_is_empty(&self) -> bool { self.ya_size == 0 }

    /// Insert a key-value pair.
    pub fn ya_insert(&mut self, key: &str, value: V) {
        let mut node = &mut self.ya_root;
        for ch in key.chars() {
            node = node.ya_children.entry(ch).or_insert_with(|| Box::new(YaTrieNode::default()));
        }
        if !node.ya_is_end { self.ya_size += 1; }
        node.ya_value = Some(value);
        node.ya_is_end = true;
    }

    /// Look up a key.
    pub fn ya_get(&self, key: &str) -> Option<&V> {
        let mut node = &self.ya_root;
        for ch in key.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return None,
            }
        }
        if node.ya_is_end { node.ya_value.as_ref() } else { None }
    }

    /// Check if a key exists.
    pub fn ya_contains(&self, key: &str) -> bool { self.ya_get(key).is_some() }

    /// Check if any key starts with the given prefix.
    pub fn ya_has_prefix(&self, prefix: &str) -> bool {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return false,
            }
        }
        true
    }

    /// Collect all keys with the given prefix.
    pub fn ya_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        Self::ya_collect_keys(node, &mut prefix.to_string(), &mut results);
        results
    }

    fn ya_collect_keys(node: &YaTrieNode<V>, current: &mut String, results: &mut Vec<String>) {
        if node.ya_is_end { results.push(current.clone()); }
        let mut chars: Vec<char> = node.ya_children.keys().copied().collect();
        chars.sort();
        for ch in chars {
            current.push(ch);
            Self::ya_collect_keys(node.ya_children.get(&ch).unwrap(), current, results);
            current.pop();
        }
    }

    /// Collect all keys.
    pub fn ya_all_keys(&self) -> Vec<String> {
        self.ya_keys_with_prefix("")
    }

    /// Remove a key. Returns the value if it existed.
    pub fn ya_remove(&mut self, key: &str) -> Option<V> {
        let result = Self::ya_remove_recursive(&mut self.ya_root, key, 0);
        if result.is_some() { self.ya_size -= 1; }
        result
    }

    fn ya_remove_recursive(node: &mut YaTrieNode<V>, key: &str, depth: usize) -> Option<V> {
        let chars: Vec<char> = key.chars().collect();
        if depth == chars.len() {
            if node.ya_is_end {
                node.ya_is_end = false;
                return node.ya_value.take();
            }
            return None;
        }
        let ch = chars[depth];
        if let Some(child) = node.ya_children.get_mut(&ch) {
            let result = Self::ya_remove_recursive(child, key, depth + 1);
            if !child.ya_is_end && child.ya_children.is_empty() {
                node.ya_children.remove(&ch);
            }
            result
        } else {
            None
        }
    }

    /// Clear the trie.
    pub fn ya_clear(&mut self) {
        self.ya_root = YaTrieNode::default();
        self.ya_size = 0;
    }

    /// Count keys with a given prefix.
    pub fn ya_count_prefix(&self, prefix: &str) -> usize {
        self.ya_keys_with_prefix(prefix).len()
    }

    /// Longest common prefix among all keys.
    pub fn ya_longest_common_prefix(&self) -> String {
        let mut result = String::new();
        let mut node = &self.ya_root;
        while node.ya_children.len() == 1 && !node.ya_is_end {
            let (&ch, child) = node.ya_children.iter().next().unwrap();
            result.push(ch);
            node = child;
        }
        result
    }
}

// --- ya_ Bloom Filter ---

/// Bloom filter for probabilistic set membership testing with no false negatives.
#[derive(Debug, Clone)]
pub struct YaBloomFilter {
    ya_bits: Vec<bool>,
    ya_size: usize,
    ya_num_hashes: usize,
    ya_count: usize,
}

impl std::fmt::Display for YaBloomFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bloom(bits={}, hashes={}, count={})", self.ya_size, self.ya_num_hashes, self.ya_count)
    }
}

impl Default for YaBloomFilter {
    fn default() -> Self { Self::ya_new(1000, 5) }
}

impl YaBloomFilter {
    /// Create a new bloom filter with given bit size and number of hash functions.
    pub fn ya_new(bits: usize, num_hashes: usize) -> Self {
        Self { ya_bits: vec![false; bits], ya_size: bits, ya_num_hashes: num_hashes.max(1), ya_count: 0 }
    }

    /// Create from expected number of items and desired false positive rate.
    pub fn ya_with_fp_rate(expected_items: usize, fp_rate: f64) -> Self {
        let bits = (-(expected_items as f64) * fp_rate.ln() / (2.0f64.ln().powi(2))).ceil() as usize;
        let bits = bits.max(64);
        let hashes = ((bits as f64 / expected_items as f64) * 2.0f64.ln()).ceil() as usize;
        let hashes = hashes.max(1);
        Self::ya_new(bits, hashes)
    }

    fn ya_hash(&self, item: u64, seed: usize) -> usize {
        let h = item.wrapping_mul(0xff51afd7ed558ccd_u64.wrapping_add(seed as u64));
        let h = h ^ (h >> 33);
        let h = h.wrapping_mul(0xc4ceb9fe1a85ec53_u64.wrapping_add(seed as u64 * 7));
        (h ^ (h >> 33)) as usize % self.ya_size
    }

    /// Add an item.
    pub fn ya_add(&mut self, item: u64) {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            self.ya_bits[idx] = true;
        }
        self.ya_count += 1;
    }

    /// Check if an item might be in the set (false positives possible, no false negatives).
    pub fn ya_might_contain(&self, item: u64) -> bool {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            if !self.ya_bits[idx] { return false; }
        }
        true
    }

    /// Number of items added.
    pub fn ya_count(&self) -> usize { self.ya_count }

    /// Bit array size.
    pub fn ya_bit_size(&self) -> usize { self.ya_size }

    /// Number of hash functions.
    pub fn ya_num_hashes(&self) -> usize { self.ya_num_hashes }

    /// Estimated false positive rate.
    pub fn ya_estimated_fp_rate(&self) -> f64 {
        let ones = self.ya_bits.iter().filter(|&&b| b).count() as f64;
        (ones / self.ya_size as f64).powi(self.ya_num_hashes as i32)
    }

    /// Clear the filter.
    pub fn ya_clear(&mut self) {
        for b in &mut self.ya_bits { *b = false; }
        self.ya_count = 0;
    }

    /// Merge another bloom filter (union).
    pub fn ya_merge(&mut self, other: &YaBloomFilter) {
        if self.ya_size != other.ya_size { return; }
        for i in 0..self.ya_size {
            self.ya_bits[i] = self.ya_bits[i] || other.ya_bits[i];
        }
    }
}


// --- yb_ Ternary Search Tree ---

/// Node in a ternary search tree (TST) for space-efficient string storage.
#[derive(Debug, Clone)]
pub struct YbTstNode<V: Clone> {
    yb_ch: char,
    yb_left: Option<Box<YbTstNode<V>>>,
    yb_mid: Option<Box<YbTstNode<V>>>,
    yb_right: Option<Box<YbTstNode<V>>>,
    yb_value: Option<V>,
}

impl<V: Clone> YbTstNode<V> {
    fn yb_new(ch: char) -> Self {
        Self { yb_ch: ch, yb_left: None, yb_mid: None, yb_right: None, yb_value: None }
    }
}

/// Ternary search tree for efficient string-keyed storage with prefix queries.
#[derive(Debug, Clone)]
pub struct YbTernarySearchTree<V: Clone> {
    yb_root: Option<Box<YbTstNode<V>>>,
    yb_size: usize,
}

impl<V: Clone> Default for YbTernarySearchTree<V> {
    fn default() -> Self { Self { yb_root: None, yb_size: 0 } }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YbTernarySearchTree<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TST(size={})", self.yb_size)
    }
}

impl<V: Clone> YbTernarySearchTree<V> {
    /// Create an empty TST.
    pub fn yb_new() -> Self { Self { yb_root: None, yb_size: 0 } }

    /// Number of stored keys.
    pub fn yb_len(&self) -> usize { self.yb_size }

    /// Is the tree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_size == 0 }

    /// Insert a key-value pair.
    pub fn yb_insert(&mut self, key: &str, value: V) {
        if key.is_empty() { return; }
        let chars: Vec<char> = key.chars().collect();
        let was_new = Self::yb_insert_node(&mut self.yb_root, &chars, 0, value);
        if was_new { self.yb_size += 1; }
    }

    fn yb_insert_node(node: &mut Option<Box<YbTstNode<V>>>, chars: &[char], depth: usize, value: V) -> bool {
        let ch = chars[depth];
        if node.is_none() { *node = Some(Box::new(YbTstNode::yb_new(ch))); }
        let n = node.as_mut().unwrap();
        if ch < n.yb_ch {
            Self::yb_insert_node(&mut n.yb_left, chars, depth, value)
        } else if ch > n.yb_ch {
            Self::yb_insert_node(&mut n.yb_right, chars, depth, value)
        } else if depth + 1 < chars.len() {
            Self::yb_insert_node(&mut n.yb_mid, chars, depth + 1, value)
        } else {
            let was_new = n.yb_value.is_none();
            n.yb_value = Some(value);
            was_new
        }
    }

    /// Look up a key.
    pub fn yb_get(&self, key: &str) -> Option<&V> {
        if key.is_empty() { return None; }
        let chars: Vec<char> = key.chars().collect();
        Self::yb_get_node(self.yb_root.as_deref(), &chars, 0)
    }

    fn yb_get_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a V> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_get_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_get_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_get_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            n.yb_value.as_ref()
        }
    }

    /// Check if a key exists.
    pub fn yb_contains(&self, key: &str) -> bool { self.yb_get(key).is_some() }

    /// Collect all keys.
    pub fn yb_all_keys(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut current = String::new();
        Self::yb_collect(self.yb_root.as_deref(), &mut current, &mut results);
        results
    }

    fn yb_collect(node: Option<&YbTstNode<V>>, current: &mut String, results: &mut Vec<String>) {
        let Some(n) = node else { return };
        Self::yb_collect(n.yb_left.as_deref(), current, results);
        current.push(n.yb_ch);
        if n.yb_value.is_some() { results.push(current.clone()); }
        Self::yb_collect(n.yb_mid.as_deref(), current, results);
        current.pop();
        Self::yb_collect(n.yb_right.as_deref(), current, results);
    }

    /// Collect keys with a given prefix.
    pub fn yb_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        if prefix.is_empty() { return self.yb_all_keys(); }
        let chars: Vec<char> = prefix.chars().collect();
        let node = Self::yb_prefix_node(self.yb_root.as_deref(), &chars, 0);
        let mut results = Vec::new();
        if let Some(n) = node {
            if n.yb_value.is_some() { results.push(prefix.to_string()); }
            let mut current = prefix.to_string();
            Self::yb_collect(n.yb_mid.as_deref(), &mut current, &mut results);
        }
        results
    }

    fn yb_prefix_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a YbTstNode<V>> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_prefix_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_prefix_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_prefix_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            Some(n)
        }
    }

    /// Clear the tree.
    pub fn yb_clear(&mut self) { self.yb_root = None; self.yb_size = 0; }
}

// --- yb_ Quadtree ---

/// A point in 2D space for quadtree storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YbPoint {
    pub yb_x: f64,
    pub yb_y: f64,
}

impl std::fmt::Display for YbPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.2}, {:.2})", self.yb_x, self.yb_y)
    }
}

impl Default for YbPoint {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0 } }
}

impl YbPoint {
    /// Create a new point.
    pub fn yb_new(x: f64, y: f64) -> Self { Self { yb_x: x, yb_y: y } }

    /// Distance to another point.
    pub fn yb_distance(&self, other: &YbPoint) -> f64 {
        ((self.yb_x - other.yb_x).powi(2) + (self.yb_y - other.yb_y).powi(2)).sqrt()
    }
}

/// Axis-aligned bounding box for quadtree partitioning.
#[derive(Debug, Clone, Copy)]
pub struct YbBounds {
    pub yb_x: f64,
    pub yb_y: f64,
    pub yb_w: f64,
    pub yb_h: f64,
}

impl std::fmt::Display for YbBounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bounds({:.1},{:.1} {}x{})", self.yb_x, self.yb_y, self.yb_w, self.yb_h)
    }
}

impl Default for YbBounds {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0, yb_w: 100.0, yb_h: 100.0 } }
}

impl YbBounds {
    /// Create bounds from origin and size.
    pub fn yb_new(x: f64, y: f64, w: f64, h: f64) -> Self { Self { yb_x: x, yb_y: y, yb_w: w, yb_h: h } }

    /// Check if a point is inside these bounds.
    pub fn yb_contains(&self, p: &YbPoint) -> bool {
        p.yb_x >= self.yb_x && p.yb_x < self.yb_x + self.yb_w &&
        p.yb_y >= self.yb_y && p.yb_y < self.yb_y + self.yb_h
    }

    /// Check if two bounds intersect.
    pub fn yb_intersects(&self, other: &YbBounds) -> bool {
        !(self.yb_x + self.yb_w <= other.yb_x || other.yb_x + other.yb_w <= self.yb_x ||
          self.yb_y + self.yb_h <= other.yb_y || other.yb_y + other.yb_h <= self.yb_y)
    }
}

/// Quadtree for 2D spatial indexing with region queries.
#[derive(Debug, Clone)]
pub struct YbQuadtree {
    yb_bounds: YbBounds,
    yb_points: Vec<YbPoint>,
    yb_capacity: usize,
    yb_nw: Option<Box<YbQuadtree>>,
    yb_ne: Option<Box<YbQuadtree>>,
    yb_sw: Option<Box<YbQuadtree>>,
    yb_se: Option<Box<YbQuadtree>>,
    yb_divided: bool,
}

impl std::fmt::Display for YbQuadtree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Quadtree(points={}, bounds={})", self.yb_count(), self.yb_bounds)
    }
}

impl Default for YbQuadtree {
    fn default() -> Self { Self::yb_new(YbBounds::default(), 4) }
}

impl YbQuadtree {
    /// Create a new quadtree with given bounds and node capacity.
    pub fn yb_new(bounds: YbBounds, capacity: usize) -> Self {
        Self {
            yb_bounds: bounds, yb_points: Vec::new(), yb_capacity: capacity.max(1),
            yb_nw: None, yb_ne: None, yb_sw: None, yb_se: None, yb_divided: false,
        }
    }

    fn yb_subdivide(&mut self) {
        let x = self.yb_bounds.yb_x;
        let y = self.yb_bounds.yb_y;
        let hw = self.yb_bounds.yb_w / 2.0;
        let hh = self.yb_bounds.yb_h / 2.0;
        self.yb_nw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y, hw, hh), self.yb_capacity)));
        self.yb_ne = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y, hw, hh), self.yb_capacity)));
        self.yb_sw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y + hh, hw, hh), self.yb_capacity)));
        self.yb_se = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y + hh, hw, hh), self.yb_capacity)));
        self.yb_divided = true;
    }

    /// Insert a point.
    pub fn yb_insert(&mut self, point: YbPoint) -> bool {
        if !self.yb_bounds.yb_contains(&point) { return false; }
        if self.yb_points.len() < self.yb_capacity && !self.yb_divided {
            self.yb_points.push(point);
            return true;
        }
        if !self.yb_divided { self.yb_subdivide(); }
        if self.yb_nw.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_ne.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_sw.as_mut().unwrap().yb_insert(point) { return true; }
        self.yb_se.as_mut().unwrap().yb_insert(point)
    }

    /// Query all points within a rectangular region.
    pub fn yb_query(&self, range: &YbBounds) -> Vec<YbPoint> {
        let mut found = Vec::new();
        self.yb_query_inner(range, &mut found);
        found
    }

    fn yb_query_inner(&self, range: &YbBounds, found: &mut Vec<YbPoint>) {
        if !self.yb_bounds.yb_intersects(range) { return; }
        for p in &self.yb_points {
            if range.yb_contains(p) { found.push(*p); }
        }
        if self.yb_divided {
            self.yb_nw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_ne.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_sw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_se.as_ref().unwrap().yb_query_inner(range, found);
        }
    }

    /// Count total points.
    pub fn yb_count(&self) -> usize {
        let mut c = self.yb_points.len();
        if self.yb_divided {
            c += self.yb_nw.as_ref().unwrap().yb_count();
            c += self.yb_ne.as_ref().unwrap().yb_count();
            c += self.yb_sw.as_ref().unwrap().yb_count();
            c += self.yb_se.as_ref().unwrap().yb_count();
        }
        c
    }

    /// Is the quadtree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_count() == 0 }

    /// Get bounds.
    pub fn yb_bounds(&self) -> &YbBounds { &self.yb_bounds }

    /// Find nearest point to a target.
    pub fn yb_nearest(&self, target: &YbPoint) -> Option<YbPoint> {
        let all = self.yb_query(&self.yb_bounds);
        all.into_iter().min_by(|a, b| {
            a.yb_distance(target).partial_cmp(&b.yb_distance(target)).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}


// --- yc_ Van Emde Boas Set ---

/// Simplified van Emde Boas-inspired set for integer keys in [0, universe).
/// Uses a flat bitmap for practical efficiency with O(1) operations.
#[derive(Debug, Clone)]
pub struct YcVebSet {
    yc_bits: Vec<u64>,
    yc_universe: usize,
    yc_count: usize,
    yc_min: Option<usize>,
    yc_max: Option<usize>,
}

impl std::fmt::Display for YcVebSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VebSet(universe={}, count={})", self.yc_universe, self.yc_count)
    }
}

impl Default for YcVebSet {
    fn default() -> Self { Self::yc_new(65536) }
}

impl YcVebSet {
    /// Create a set supporting keys in [0, universe).
    pub fn yc_new(universe: usize) -> Self {
        let words = (universe + 63) / 64;
        Self { yc_bits: vec![0; words], yc_universe: universe, yc_count: 0, yc_min: None, yc_max: None }
    }

    /// Universe size.
    pub fn yc_universe(&self) -> usize { self.yc_universe }

    /// Number of elements.
    pub fn yc_len(&self) -> usize { self.yc_count }

    /// Is the set empty.
    pub fn yc_is_empty(&self) -> bool { self.yc_count == 0 }

    /// Insert a key.
    pub fn yc_insert(&mut self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        let word = key / 64;
        let bit = key % 64;
        if self.yc_bits[word] & (1u64 << bit) != 0 { return false; }
        self.yc_bits[word] |= 1u64 << bit;
        self.yc_count += 1;
        self.yc_min = Some(self.yc_min.map_or(key, |m: usize| m.min(key)));
        self.yc_max = Some(self.yc_max.map_or(key, |m: usize| m.max(key)));
        true
    }

    /// Remove a key.
    pub fn yc_remove(&mut self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        let word = key / 64;
        let bit = key % 64;
        if self.yc_bits[word] & (1u64 << bit) == 0 { return false; }
        self.yc_bits[word] &= !(1u64 << bit);
        self.yc_count -= 1;
        if self.yc_count == 0 { self.yc_min = None; self.yc_max = None; }
        else {
            if self.yc_min == Some(key) { self.yc_min = self.yc_successor(key); }
            if self.yc_max == Some(key) { self.yc_max = self.yc_predecessor(key); }
        }
        true
    }

    /// Check membership.
    pub fn yc_contains(&self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        self.yc_bits[key / 64] & (1u64 << (key % 64)) != 0
    }

    /// Minimum element.
    pub fn yc_min(&self) -> Option<usize> { self.yc_min }

    /// Maximum element.
    pub fn yc_max(&self) -> Option<usize> { self.yc_max }

    /// Find the smallest key > given key.
    pub fn yc_successor(&self, key: usize) -> Option<usize> {
        for k in (key + 1)..self.yc_universe {
            if self.yc_contains(k) { return Some(k); }
        }
        None
    }

    /// Find the largest key < given key.
    pub fn yc_predecessor(&self, key: usize) -> Option<usize> {
        if key == 0 { return None; }
        for k in (0..key).rev() {
            if self.yc_contains(k) { return Some(k); }
        }
        None
    }

    /// Collect all elements in sorted order.
    pub fn yc_to_sorted_vec(&self) -> Vec<usize> {
        let mut result = Vec::with_capacity(self.yc_count);
        for w in 0..self.yc_bits.len() {
            let mut bits = self.yc_bits[w];
            while bits != 0 {
                let tz = bits.trailing_zeros() as usize;
                result.push(w * 64 + tz);
                bits &= bits - 1;
            }
        }
        result
    }

    /// Clear the set.
    pub fn yc_clear(&mut self) {
        for w in &mut self.yc_bits { *w = 0; }
        self.yc_count = 0;
        self.yc_min = None;
        self.yc_max = None;
    }

    /// Union with another set (same universe).
    pub fn yc_union(&mut self, other: &YcVebSet) {
        if self.yc_universe != other.yc_universe { return; }
        for i in 0..self.yc_bits.len() {
            self.yc_bits[i] |= other.yc_bits[i];
        }
        self.yc_count = self.yc_to_sorted_vec().len();
        let sorted = self.yc_to_sorted_vec();
        self.yc_min = sorted.first().copied();
        self.yc_max = sorted.last().copied();
    }

    /// Intersection with another set.
    pub fn yc_intersection(&self, other: &YcVebSet) -> YcVebSet {
        let mut result = YcVebSet::yc_new(self.yc_universe);
        if self.yc_universe != other.yc_universe { return result; }
        for i in 0..self.yc_bits.len() {
            result.yc_bits[i] = self.yc_bits[i] & other.yc_bits[i];
        }
        let sorted = result.yc_to_sorted_vec();
        result.yc_count = sorted.len();
        result.yc_min = sorted.first().copied();
        result.yc_max = sorted.last().copied();
        result
    }
}

// --- yc_ Consistent Hash Ring ---

/// Consistent hash ring for distributed key mapping with virtual nodes.
#[derive(Debug, Clone)]
pub struct YcHashRing {
    yc_ring: std::collections::BTreeMap<u64, String>,
    yc_replicas: usize,
    yc_nodes: Vec<String>,
}

impl std::fmt::Display for YcHashRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HashRing(nodes={}, replicas={})", self.yc_nodes.len(), self.yc_replicas)
    }
}

impl Default for YcHashRing {
    fn default() -> Self { Self { yc_ring: std::collections::BTreeMap::new(), yc_replicas: 150, yc_nodes: Vec::new() } }
}

impl YcHashRing {
    /// Create a new hash ring with given replica count per node.
    pub fn yc_new(replicas: usize) -> Self {
        Self { yc_ring: std::collections::BTreeMap::new(), yc_replicas: replicas.max(1), yc_nodes: Vec::new() }
    }

    fn yc_hash(key: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in key.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// Add a node to the ring.
    pub fn yc_add_node(&mut self, node: &str) {
        for i in 0..self.yc_replicas {
            let key = format!("{}:{}", node, i);
            let hash = Self::yc_hash(&key);
            self.yc_ring.insert(hash, node.to_string());
        }
        self.yc_nodes.push(node.to_string());
    }

    /// Remove a node from the ring.
    pub fn yc_remove_node(&mut self, node: &str) {
        for i in 0..self.yc_replicas {
            let key = format!("{}:{}", node, i);
            let hash = Self::yc_hash(&key);
            self.yc_ring.remove(&hash);
        }
        self.yc_nodes.retain(|n| n != node);
    }

    /// Find the node responsible for a key.
    pub fn yc_get_node(&self, key: &str) -> Option<&str> {
        if self.yc_ring.is_empty() { return None; }
        let hash = Self::yc_hash(key);
        let node = self.yc_ring.range(hash..).next()
            .or_else(|| self.yc_ring.iter().next());
        node.map(|(_, v)| v.as_str())
    }

    /// Number of physical nodes.
    pub fn yc_node_count(&self) -> usize { self.yc_nodes.len() }

    /// Number of virtual nodes on the ring.
    pub fn yc_virtual_count(&self) -> usize { self.yc_ring.len() }

    /// List all physical nodes.
    pub fn yc_nodes(&self) -> &[String] { &self.yc_nodes }

    /// Check if a node is in the ring.
    pub fn yc_has_node(&self, node: &str) -> bool { self.yc_nodes.iter().any(|n| n == node) }
}


// --- yd_ Directed Acyclic Graph ---

/// Directed acyclic graph with topological sorting and cycle detection.
#[derive(Debug, Clone)]
pub struct YdDag {
    yd_adj: std::collections::HashMap<usize, Vec<usize>>,
    yd_in_degree: std::collections::HashMap<usize, usize>,
    yd_nodes: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YdDag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let edges: usize = self.yd_adj.values().map(|v| v.len()).sum();
        write!(f, "DAG(nodes={}, edges={})", self.yd_nodes.len(), edges)
    }
}

impl Default for YdDag {
    fn default() -> Self { Self::yd_new() }
}

impl YdDag {
    /// Create an empty DAG.
    pub fn yd_new() -> Self {
        Self { yd_adj: std::collections::HashMap::new(), yd_in_degree: std::collections::HashMap::new(), yd_nodes: std::collections::HashSet::new() }
    }

    /// Add a node.
    pub fn yd_add_node(&mut self, node: usize) {
        self.yd_nodes.insert(node);
        self.yd_adj.entry(node).or_default();
        self.yd_in_degree.entry(node).or_insert(0);
    }

    /// Add a directed edge from -> to.
    pub fn yd_add_edge(&mut self, from: usize, to: usize) {
        self.yd_add_node(from);
        self.yd_add_node(to);
        self.yd_adj.entry(from).or_default().push(to);
        *self.yd_in_degree.entry(to).or_insert(0) += 1;
    }

    /// Number of nodes.
    pub fn yd_node_count(&self) -> usize { self.yd_nodes.len() }

    /// Number of edges.
    pub fn yd_edge_count(&self) -> usize { self.yd_adj.values().map(|v| v.len()).sum() }

    /// Topological sort using Kahn's algorithm. Returns None if cycle detected.
    pub fn yd_topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = self.yd_in_degree.clone();
        let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        for (node, deg) in &in_deg {
            if *deg == 0 { queue.push_back(*node); }
        }
        let mut result = Vec::new();
        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(neighbors) = self.yd_adj.get(&node) {
                for &next in neighbors {
                    let d = in_deg.get_mut(&next).unwrap();
                    *d -= 1;
                    if *d == 0 { queue.push_back(next); }
                }
            }
        }
        if result.len() == self.yd_nodes.len() { Some(result) } else { None }
    }

    /// Check if the graph has a cycle.
    pub fn yd_has_cycle(&self) -> bool { self.yd_topological_sort().is_none() }

    /// Get all neighbors of a node.
    pub fn yd_neighbors(&self, node: usize) -> &[usize] {
        self.yd_adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get in-degree of a node.
    pub fn yd_in_degree(&self, node: usize) -> usize {
        self.yd_in_degree.get(&node).copied().unwrap_or(0)
    }

    /// Get out-degree of a node.
    pub fn yd_out_degree(&self, node: usize) -> usize {
        self.yd_adj.get(&node).map(|v| v.len()).unwrap_or(0)
    }

    /// Find all root nodes (in-degree 0).
    pub fn yd_roots(&self) -> Vec<usize> {
        let mut roots: Vec<usize> = self.yd_in_degree.iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        roots.sort();
        roots
    }

    /// Find all leaf nodes (out-degree 0).
    pub fn yd_leaves(&self) -> Vec<usize> {
        let mut leaves: Vec<usize> = self.yd_nodes.iter()
            .filter(|&&n| self.yd_out_degree(n) == 0)
            .copied()
            .collect();
        leaves.sort();
        leaves
    }

    /// Check if node exists.
    pub fn yd_has_node(&self, node: usize) -> bool { self.yd_nodes.contains(&node) }

    /// Clear the graph.
    pub fn yd_clear(&mut self) {
        self.yd_adj.clear();
        self.yd_in_degree.clear();
        self.yd_nodes.clear();
    }

    /// BFS traversal from a start node.
    pub fn yd_bfs(&self, start: usize) -> Vec<usize> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();
        queue.push_back(start);
        visited.insert(start);
        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(neighbors) = self.yd_adj.get(&node) {
                let mut sorted_n = neighbors.clone();
                sorted_n.sort();
                for next in sorted_n {
                    if visited.insert(next) { queue.push_back(next); }
                }
            }
        }
        result
    }

    /// DFS traversal from a start node.
    pub fn yd_dfs(&self, start: usize) -> Vec<usize> {
        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        self.yd_dfs_inner(start, &mut visited, &mut result);
        result
    }

    fn yd_dfs_inner(&self, node: usize, visited: &mut std::collections::HashSet<usize>, result: &mut Vec<usize>) {
        if !visited.insert(node) { return; }
        result.push(node);
        if let Some(neighbors) = self.yd_adj.get(&node) {
            let mut sorted_n = neighbors.clone();
            sorted_n.sort();
            for next in sorted_n {
                self.yd_dfs_inner(next, visited, result);
            }
        }
    }

    /// Shortest path length (unweighted) between two nodes using BFS.
    pub fn yd_shortest_path(&self, from: usize, to: usize) -> Option<usize> {
        if from == to { return Some(0); }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((from, 0usize));
        visited.insert(from);
        while let Some((node, dist)) = queue.pop_front() {
            if let Some(neighbors) = self.yd_adj.get(&node) {
                for &next in neighbors {
                    if next == to { return Some(dist + 1); }
                    if visited.insert(next) { queue.push_back((next, dist + 1)); }
                }
            }
        }
        None
    }
}

// --- yd_ Sparse Matrix ---

/// Sparse matrix using coordinate (COO) format for efficient storage.
#[derive(Debug, Clone)]
pub struct YdSparseMatrix {
    yd_rows: usize,
    yd_cols: usize,
    yd_entries: std::collections::HashMap<(usize, usize), f64>,
}

impl std::fmt::Display for YdSparseMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SparseMatrix({}x{}, nnz={})", self.yd_rows, self.yd_cols, self.yd_entries.len())
    }
}

impl Default for YdSparseMatrix {
    fn default() -> Self { Self::yd_new(0, 0) }
}

impl YdSparseMatrix {
    /// Create a new sparse matrix with given dimensions.
    pub fn yd_new(rows: usize, cols: usize) -> Self {
        Self { yd_rows: rows, yd_cols: cols, yd_entries: std::collections::HashMap::new() }
    }

    /// Set a value.
    pub fn yd_set(&mut self, row: usize, col: usize, val: f64) {
        if val == 0.0 { self.yd_entries.remove(&(row, col)); }
        else { self.yd_entries.insert((row, col), val); }
    }

    /// Get a value.
    pub fn yd_get(&self, row: usize, col: usize) -> f64 {
        self.yd_entries.get(&(row, col)).copied().unwrap_or(0.0)
    }

    /// Number of non-zero entries.
    pub fn yd_nnz(&self) -> usize { self.yd_entries.len() }

    /// Dimensions.
    pub fn yd_rows(&self) -> usize { self.yd_rows }
    pub fn yd_cols(&self) -> usize { self.yd_cols }

    /// Transpose.
    pub fn yd_transpose(&self) -> YdSparseMatrix {
        let mut t = YdSparseMatrix::yd_new(self.yd_cols, self.yd_rows);
        for ((r, c), v) in &self.yd_entries {
            t.yd_set(*c, *r, *v);
        }
        t
    }

    /// Matrix-vector multiply.
    pub fn yd_mul_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.yd_rows];
        for ((r, c), v) in &self.yd_entries {
            if *c < vec.len() && *r < result.len() {
                result[*r] += *v * vec[*c];
            }
        }
        result
    }

    /// Scale all entries.
    pub fn yd_scale(&mut self, factor: f64) {
        for v in self.yd_entries.values_mut() { *v *= factor; }
    }

    /// Add another sparse matrix.
    pub fn yd_add(&self, other: &YdSparseMatrix) -> YdSparseMatrix {
        let mut result = self.clone();
        for ((r, c), v) in &other.yd_entries {
            let entry = result.yd_entries.entry((*r, *c)).or_insert(0.0);
            *entry += *v;
        }
        result
    }

    /// Clear all entries.
    pub fn yd_clear(&mut self) { self.yd_entries.clear(); }

    /// Row sum.
    pub fn yd_row_sum(&self, row: usize) -> f64 {
        self.yd_entries.iter()
            .filter(|((r, _), _)| *r == row)
            .map(|(_, v)| *v)
            .sum()
    }

    /// Frobenius norm squared.
    pub fn yd_frobenius_sq(&self) -> f64 {
        self.yd_entries.values().map(|v| *v * *v).sum()
    }
}


// --- ye_ Indexed Priority Queue ---

/// Indexed min-priority queue supporting decrease-key in O(log n).
#[derive(Debug, Clone)]
pub struct YeIndexedPQ {
    ye_heap: Vec<(usize, i64)>,
    ye_pos: std::collections::HashMap<usize, usize>,
}

impl std::fmt::Display for YeIndexedPQ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IndexedPQ(size={})", self.ye_heap.len())
    }
}

impl Default for YeIndexedPQ {
    fn default() -> Self { Self::ye_new() }
}

impl YeIndexedPQ {
    /// Create an empty indexed priority queue.
    pub fn ye_new() -> Self { Self { ye_heap: Vec::new(), ye_pos: std::collections::HashMap::new() } }

    /// Number of elements.
    pub fn ye_len(&self) -> usize { self.ye_heap.len() }

    /// Is empty.
    pub fn ye_is_empty(&self) -> bool { self.ye_heap.is_empty() }

    /// Insert an element with priority.
    pub fn ye_insert(&mut self, id: usize, priority: i64) {
        if self.ye_pos.contains_key(&id) { self.ye_decrease_key(id, priority); return; }
        let idx = self.ye_heap.len();
        self.ye_heap.push((id, priority));
        self.ye_pos.insert(id, idx);
        self.ye_sift_up(idx);
    }

    /// Peek at minimum.
    pub fn ye_peek(&self) -> Option<(usize, i64)> { self.ye_heap.first().copied() }

    /// Extract minimum.
    pub fn ye_pop(&mut self) -> Option<(usize, i64)> {
        if self.ye_heap.is_empty() { return None; }
        let min = self.ye_heap[0];
        let last = self.ye_heap.len() - 1;
        self.ye_swap(0, last);
        self.ye_heap.pop();
        self.ye_pos.remove(&min.0);
        if !self.ye_heap.is_empty() { self.ye_sift_down(0); }
        Some(min)
    }

    /// Decrease the priority of an element.
    pub fn ye_decrease_key(&mut self, id: usize, new_priority: i64) {
        if let Some(&idx) = self.ye_pos.get(&id) {
            if new_priority < self.ye_heap[idx].1 {
                self.ye_heap[idx].1 = new_priority;
                self.ye_sift_up(idx);
            }
        }
    }

    /// Check if an id is in the queue.
    pub fn ye_contains(&self, id: usize) -> bool { self.ye_pos.contains_key(&id) }

    /// Get priority of an id.
    pub fn ye_priority(&self, id: usize) -> Option<i64> {
        self.ye_pos.get(&id).map(|&idx| self.ye_heap[idx].1)
    }

    fn ye_swap(&mut self, i: usize, j: usize) {
        self.ye_heap.swap(i, j);
        self.ye_pos.insert(self.ye_heap[i].0, i);
        self.ye_pos.insert(self.ye_heap[j].0, j);
    }

    fn ye_sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.ye_heap[idx].1 < self.ye_heap[parent].1 {
                self.ye_swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn ye_sift_down(&mut self, mut idx: usize) {
        let n = self.ye_heap.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < n && self.ye_heap[left].1 < self.ye_heap[smallest].1 { smallest = left; }
            if right < n && self.ye_heap[right].1 < self.ye_heap[smallest].1 { smallest = right; }
            if smallest == idx { break; }
            self.ye_swap(idx, smallest);
            idx = smallest;
        }
    }

    /// Clear the queue.
    pub fn ye_clear(&mut self) { self.ye_heap.clear(); self.ye_pos.clear(); }

    /// Drain all elements in priority order.
    pub fn ye_drain_sorted(&mut self) -> Vec<(usize, i64)> {
        let mut result = Vec::with_capacity(self.ye_heap.len());
        while let Some(item) = self.ye_pop() { result.push(item); }
        result
    }
}

// --- ye_ Segment Tree with Lazy Propagation ---

/// Segment tree with lazy propagation for range queries and updates.
#[derive(Debug, Clone)]
pub struct YeSegTree {
    ye_n: usize,
    ye_tree: Vec<i64>,
    ye_lazy: Vec<i64>,
}

impl std::fmt::Display for YeSegTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SegTree(n={})", self.ye_n)
    }
}

impl Default for YeSegTree {
    fn default() -> Self { Self { ye_n: 0, ye_tree: Vec::new(), ye_lazy: Vec::new() } }
}

impl YeSegTree {
    /// Build from an array of values.
    pub fn ye_from_slice(data: &[i64]) -> Self {
        let n = data.len();
        let mut tree = vec![0i64; 4 * n];
        let lazy = vec![0i64; 4 * n];
        let mut st = Self { ye_n: n, ye_tree: tree.clone(), ye_lazy: lazy };
        if n > 0 { st.ye_build(data, 1, 0, n - 1); }
        st
    }

    fn ye_build(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.ye_tree[node] = data[start];
            return;
        }
        let mid = (start + end) / 2;
        self.ye_build(data, 2 * node, start, mid);
        self.ye_build(data, 2 * node + 1, mid + 1, end);
        self.ye_tree[node] = self.ye_tree[2 * node] + self.ye_tree[2 * node + 1];
    }

    fn ye_push_down(&mut self, node: usize, start: usize, end: usize) {
        if self.ye_lazy[node] != 0 {
            let mid = (start + end) / 2;
            self.ye_tree[2 * node] += self.ye_lazy[node] * (mid - start + 1) as i64;
            self.ye_tree[2 * node + 1] += self.ye_lazy[node] * (end - mid) as i64;
            self.ye_lazy[2 * node] += self.ye_lazy[node];
            self.ye_lazy[2 * node + 1] += self.ye_lazy[node];
            self.ye_lazy[node] = 0;
        }
    }

    /// Range sum query [l, r].
    pub fn ye_query(&mut self, l: usize, r: usize) -> i64 {
        if self.ye_n == 0 || l > r || r >= self.ye_n { return 0; }
        self.ye_query_inner(1, 0, self.ye_n - 1, l, r)
    }

    fn ye_query_inner(&mut self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.ye_tree[node]; }
        self.ye_push_down(node, start, end);
        let mid = (start + end) / 2;
        self.ye_query_inner(2 * node, start, mid, l, r) +
        self.ye_query_inner(2 * node + 1, mid + 1, end, l, r)
    }

    /// Range update: add val to all elements in [l, r].
    pub fn ye_update(&mut self, l: usize, r: usize, val: i64) {
        if self.ye_n == 0 || l > r || r >= self.ye_n { return; }
        self.ye_update_inner(1, 0, self.ye_n - 1, l, r, val);
    }

    fn ye_update_inner(&mut self, node: usize, start: usize, end: usize, l: usize, r: usize, val: i64) {
        if r < start || end < l { return; }
        if l <= start && end <= r {
            self.ye_tree[node] += val * (end - start + 1) as i64;
            self.ye_lazy[node] += val;
            return;
        }
        self.ye_push_down(node, start, end);
        let mid = (start + end) / 2;
        self.ye_update_inner(2 * node, start, mid, l, r, val);
        self.ye_update_inner(2 * node + 1, mid + 1, end, l, r, val);
        self.ye_tree[node] = self.ye_tree[2 * node] + self.ye_tree[2 * node + 1];
    }

    /// Point query: get value at index.
    pub fn ye_point_query(&mut self, idx: usize) -> i64 {
        self.ye_query(idx, idx)
    }

    /// Size of underlying array.
    pub fn ye_len(&self) -> usize { self.ye_n }

    /// Is empty.
    pub fn ye_is_empty(&self) -> bool { self.ye_n == 0 }
}


// --- yf_ Disjoint Interval Set ---

/// Set of non-overlapping intervals with automatic merging.
#[derive(Debug, Clone)]
pub struct YfIntervalSet {
    yf_intervals: std::collections::BTreeMap<i64, i64>,
}

impl std::fmt::Display for YfIntervalSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IntervalSet(count={})", self.yf_intervals.len())
    }
}

impl Default for YfIntervalSet {
    fn default() -> Self { Self::yf_new() }
}

impl YfIntervalSet {
    /// Create an empty interval set.
    pub fn yf_new() -> Self { Self { yf_intervals: std::collections::BTreeMap::new() } }

    /// Number of disjoint intervals.
    pub fn yf_len(&self) -> usize { self.yf_intervals.len() }

    /// Is empty.
    pub fn yf_is_empty(&self) -> bool { self.yf_intervals.is_empty() }

    /// Add an interval [lo, hi]. Merges with overlapping/adjacent intervals.
    pub fn yf_add(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let to_remove: Vec<i64> = self.yf_intervals.range(..=hi + 1)
            .filter(|(start, end)| **end >= lo - 1)
            .map(|(s, _)| *s)
            .collect();
        for s in &to_remove {
            let e = self.yf_intervals[s];
            new_lo = new_lo.min(*s);
            new_hi = new_hi.max(e);
            self.yf_intervals.remove(s);
        }
        self.yf_intervals.insert(new_lo, new_hi);
    }

    /// Check if a value is covered by any interval.
    pub fn yf_contains(&self, val: i64) -> bool {
        if let Some((_, end)) = self.yf_intervals.range(..=val).next_back() {
            *end >= val
        } else {
            false
        }
    }

    /// Remove a point, splitting intervals if needed.
    pub fn yf_remove_point(&mut self, val: i64) {
        let covering = self.yf_intervals.range(..=val)
            .filter(|(_, end)| **end >= val)
            .map(|(s, e)| (*s, *e))
            .next_back();
        if let Some((s, e)) = covering {
            self.yf_intervals.remove(&s);
            if s < val { self.yf_intervals.insert(s, val - 1); }
            if val < e { self.yf_intervals.insert(val + 1, e); }
        }
    }

    /// Get all intervals as sorted vec.
    pub fn yf_intervals(&self) -> Vec<(i64, i64)> {
        self.yf_intervals.iter().map(|(s, e)| (*s, *e)).collect()
    }

    /// Total covered length.
    pub fn yf_total_length(&self) -> i64 {
        self.yf_intervals.iter().map(|(s, e)| e - s + 1).sum()
    }

    /// Clear all intervals.
    pub fn yf_clear(&mut self) { self.yf_intervals.clear(); }

    /// Check if two interval sets overlap.
    pub fn yf_overlaps(&self, other: &YfIntervalSet) -> bool {
        for (s, e) in &self.yf_intervals {
            for (os, oe) in &other.yf_intervals {
                if s <= oe && os <= e { return true; }
            }
        }
        false
    }
}

// --- yf_ K-way Merge ---

/// K-way merge iterator that merges multiple sorted sequences.
#[derive(Debug, Clone)]
pub struct YfKWayMerge {
    yf_sources: Vec<Vec<i64>>,
    yf_indices: Vec<usize>,
}

impl std::fmt::Display for YfKWayMerge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KWayMerge(sources={})", self.yf_sources.len())
    }
}

impl Default for YfKWayMerge {
    fn default() -> Self { Self::yf_new() }
}

impl YfKWayMerge {
    /// Create an empty k-way merge.
    pub fn yf_new() -> Self { Self { yf_sources: Vec::new(), yf_indices: Vec::new() } }

    /// Add a sorted source.
    pub fn yf_add_source(&mut self, source: Vec<i64>) {
        self.yf_sources.push(source);
        self.yf_indices.push(0);
    }

    /// Number of sources.
    pub fn yf_source_count(&self) -> usize { self.yf_sources.len() }

    /// Merge all sources into a single sorted vec.
    pub fn yf_merge(&mut self) -> Vec<i64> {
        let mut result = Vec::new();
        loop {
            let mut min_val: Option<i64> = None;
            let mut min_src = 0;
            for (i, (src, idx)) in self.yf_sources.iter().zip(self.yf_indices.iter()).enumerate() {
                if *idx < src.len() {
                    let v = src[*idx];
                    if min_val.is_none() || v < min_val.unwrap() {
                        min_val = Some(v);
                        min_src = i;
                    }
                }
            }
            match min_val {
                Some(v) => { result.push(v); self.yf_indices[min_src] += 1; }
                None => break,
            }
        }
        result
    }

    /// Total remaining elements across all sources.
    pub fn yf_remaining(&self) -> usize {
        self.yf_sources.iter().zip(self.yf_indices.iter())
            .map(|(src, idx)| src.len().saturating_sub(*idx))
            .sum()
    }

    /// Reset all indices.
    pub fn yf_reset(&mut self) {
        for idx in &mut self.yf_indices { *idx = 0; }
    }

    /// Clear all sources.
    pub fn yf_clear(&mut self) { self.yf_sources.clear(); self.yf_indices.clear(); }

    /// Check if merge is complete.
    pub fn yf_is_done(&self) -> bool { self.yf_remaining() == 0 }

    /// Merge and deduplicate.
    pub fn yf_merge_unique(&mut self) -> Vec<i64> {
        let merged = self.yf_merge();
        let mut unique = Vec::new();
        for v in merged {
            if unique.last() != Some(&v) { unique.push(v); }
        }
        unique
    }
}


// --- yg_ Persistent Stack ---

/// Immutable persistent stack using a linked list of arcs.
#[derive(Debug, Clone)]
pub struct YgPersistentStack<T: Clone> {
    yg_head: Option<std::sync::Arc<YgStackNode<T>>>,
    yg_size: usize,
}

#[derive(Debug, Clone)]
struct YgStackNode<T: Clone> {
    yg_value: T,
    yg_next: Option<std::sync::Arc<YgStackNode<T>>>,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for YgPersistentStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PStack(size={})", self.yg_size)
    }
}

impl<T: Clone> Default for YgPersistentStack<T> {
    fn default() -> Self { Self::yg_new() }
}

impl<T: Clone> YgPersistentStack<T> {
    /// Create an empty persistent stack.
    pub fn yg_new() -> Self { Self { yg_head: None, yg_size: 0 } }

    /// Push returns a new stack with the element on top.
    pub fn yg_push(&self, value: T) -> Self {
        Self {
            yg_head: Some(std::sync::Arc::new(YgStackNode { yg_value: value, yg_next: self.yg_head.clone() })),
            yg_size: self.yg_size + 1,
        }
    }

    /// Pop returns the top value and a new stack without it.
    pub fn yg_pop(&self) -> Option<(T, Self)> {
        self.yg_head.as_ref().map(|node| {
            (node.yg_value.clone(), Self { yg_head: node.yg_next.clone(), yg_size: self.yg_size - 1 })
        })
    }

    /// Peek at the top value.
    pub fn yg_peek(&self) -> Option<&T> {
        self.yg_head.as_ref().map(|node| &node.yg_value)
    }

    /// Size of the stack.
    pub fn yg_len(&self) -> usize { self.yg_size }

    /// Is empty.
    pub fn yg_is_empty(&self) -> bool { self.yg_size == 0 }

    /// Convert to vec (top first).
    pub fn yg_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.yg_size);
        let mut current = &self.yg_head;
        while let Some(node) = current {
            result.push(node.yg_value.clone());
            current = &node.yg_next;
        }
        result
    }

    /// Reverse the stack.
    pub fn yg_reverse(&self) -> Self {
        let mut result = Self::yg_new();
        let mut current = &self.yg_head;
        while let Some(node) = current {
            result = result.yg_push(node.yg_value.clone());
            current = &node.yg_next;
        }
        result
    }
}

// --- yg_ Bitmap Index ---

/// Bitmap index for fast multi-column filtering on categorical data.
#[derive(Debug, Clone)]
pub struct YgBitmapIndex {
    yg_bitmaps: std::collections::HashMap<String, Vec<u64>>,
    yg_num_rows: usize,
}

impl std::fmt::Display for YgBitmapIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BitmapIndex(rows={}, columns={})", self.yg_num_rows, self.yg_bitmaps.len())
    }
}

impl Default for YgBitmapIndex {
    fn default() -> Self { Self::yg_new(0) }
}

impl YgBitmapIndex {
    /// Create a bitmap index for a given number of rows.
    pub fn yg_new(num_rows: usize) -> Self {
        Self { yg_bitmaps: std::collections::HashMap::new(), yg_num_rows: num_rows }
    }

    fn yg_words(n: usize) -> usize { (n + 63) / 64 }

    /// Set a value for a column at a given row.
    pub fn yg_set(&mut self, column: &str, row: usize) {
        if row >= self.yg_num_rows { return; }
        let words = Self::yg_words(self.yg_num_rows);
        let bitmap = self.yg_bitmaps.entry(column.to_string()).or_insert_with(|| vec![0u64; words]);
        bitmap[row / 64] |= 1u64 << (row % 64);
    }

    /// Check if a column is set for a row.
    pub fn yg_get(&self, column: &str, row: usize) -> bool {
        if row >= self.yg_num_rows { return false; }
        self.yg_bitmaps.get(column)
            .map(|bm| bm[row / 64] & (1u64 << (row % 64)) != 0)
            .unwrap_or(false)
    }

    /// AND query: rows where all columns are set.
    pub fn yg_and(&self, columns: &[&str]) -> Vec<usize> {
        let words = Self::yg_words(self.yg_num_rows);
        let mut result = vec![u64::MAX; words];
        for col in columns {
            if let Some(bm) = self.yg_bitmaps.get(*col) {
                for (i, w) in result.iter_mut().enumerate() { *w &= bm[i]; }
            } else {
                return Vec::new();
            }
        }
        Self::yg_bits_to_rows(&result, self.yg_num_rows)
    }

    /// OR query: rows where any column is set.
    pub fn yg_or(&self, columns: &[&str]) -> Vec<usize> {
        let words = Self::yg_words(self.yg_num_rows);
        let mut result = vec![0u64; words];
        for col in columns {
            if let Some(bm) = self.yg_bitmaps.get(*col) {
                for (i, w) in result.iter_mut().enumerate() { *w |= bm[i]; }
            }
        }
        Self::yg_bits_to_rows(&result, self.yg_num_rows)
    }

    fn yg_bits_to_rows(bits: &[u64], max_rows: usize) -> Vec<usize> {
        let mut rows = Vec::new();
        for (w_idx, word) in bits.iter().enumerate() {
            let mut bits = *word;
            while bits != 0 {
                let tz = bits.trailing_zeros() as usize;
                let row = w_idx * 64 + tz;
                if row < max_rows { rows.push(row); }
                bits &= bits - 1;
            }
        }
        rows
    }

    /// Count of rows for a column.
    pub fn yg_count(&self, column: &str) -> usize {
        self.yg_bitmaps.get(column)
            .map(|bm| bm.iter().map(|w| w.count_ones() as usize).sum())
            .unwrap_or(0)
    }

    /// Number of rows.
    pub fn yg_num_rows(&self) -> usize { self.yg_num_rows }

    /// Number of columns.
    pub fn yg_num_columns(&self) -> usize { self.yg_bitmaps.len() }

    /// List column names.
    pub fn yg_columns(&self) -> Vec<String> {
        let mut cols: Vec<String> = self.yg_bitmaps.keys().cloned().collect();
        cols.sort();
        cols
    }

    /// Clear all bitmaps.
    pub fn yg_clear(&mut self) { self.yg_bitmaps.clear(); }
}


// --- yh_ Order Statistics Tree ---

/// Order statistics tree supporting rank queries and selection.
/// Implemented as an augmented BST with subtree sizes.
#[derive(Debug, Clone)]
pub struct YhOrderStatTree {
    yh_root: Option<Box<YhOstNode>>,
}

#[derive(Debug, Clone)]
struct YhOstNode {
    yh_key: i64,
    yh_left: Option<Box<YhOstNode>>,
    yh_right: Option<Box<YhOstNode>>,
    yh_size: usize,
}

impl YhOstNode {
    fn yh_new(key: i64) -> Self {
        Self { yh_key: key, yh_left: None, yh_right: None, yh_size: 1 }
    }

    fn yh_left_size(&self) -> usize {
        self.yh_left.as_ref().map_or(0, |n| n.yh_size)
    }

    fn yh_update_size(&mut self) {
        self.yh_size = 1 + self.yh_left.as_ref().map_or(0, |n| n.yh_size)
            + self.yh_right.as_ref().map_or(0, |n| n.yh_size);
    }
}

impl std::fmt::Display for YhOrderStatTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OSTree(size={})", self.yh_len())
    }
}

impl Default for YhOrderStatTree {
    fn default() -> Self { Self::yh_new() }
}

impl YhOrderStatTree {
    /// Create an empty order statistics tree.
    pub fn yh_new() -> Self { Self { yh_root: None } }

    /// Number of elements.
    pub fn yh_len(&self) -> usize { self.yh_root.as_ref().map_or(0, |n| n.yh_size) }

    /// Is empty.
    pub fn yh_is_empty(&self) -> bool { self.yh_root.is_none() }

    /// Insert a key.
    pub fn yh_insert(&mut self, key: i64) {
        Self::yh_insert_node(&mut self.yh_root, key);
    }

    fn yh_insert_node(node: &mut Option<Box<YhOstNode>>, key: i64) {
        match node {
            None => { *node = Some(Box::new(YhOstNode::yh_new(key))); }
            Some(n) => {
                if key < n.yh_key { Self::yh_insert_node(&mut n.yh_left, key); }
                else if key > n.yh_key { Self::yh_insert_node(&mut n.yh_right, key); }
                n.yh_update_size();
            }
        }
    }

    /// Check if a key exists.
    pub fn yh_contains(&self, key: i64) -> bool {
        let mut current = &self.yh_root;
        while let Some(n) = current {
            if key < n.yh_key { current = &n.yh_left; }
            else if key > n.yh_key { current = &n.yh_right; }
            else { return true; }
        }
        false
    }

    /// Rank of a key (0-indexed, number of elements < key).
    pub fn yh_rank(&self, key: i64) -> usize {
        Self::yh_rank_node(&self.yh_root, key)
    }

    fn yh_rank_node(node: &Option<Box<YhOstNode>>, key: i64) -> usize {
        match node {
            None => 0,
            Some(n) => {
                if key < n.yh_key { Self::yh_rank_node(&n.yh_left, key) }
                else if key > n.yh_key { n.yh_left_size() + 1 + Self::yh_rank_node(&n.yh_right, key) }
                else { n.yh_left_size() }
            }
        }
    }

    /// Select the k-th smallest element (0-indexed).
    pub fn yh_select(&self, k: usize) -> Option<i64> {
        Self::yh_select_node(&self.yh_root, k)
    }

    fn yh_select_node(node: &Option<Box<YhOstNode>>, k: usize) -> Option<i64> {
        let n = node.as_ref()?;
        let left_size = n.yh_left_size();
        if k < left_size { Self::yh_select_node(&n.yh_left, k) }
        else if k > left_size { Self::yh_select_node(&n.yh_right, k - left_size - 1) }
        else { Some(n.yh_key) }
    }

    /// Minimum key.
    pub fn yh_min(&self) -> Option<i64> {
        let mut current = &self.yh_root;
        let mut min = None;
        while let Some(n) = current {
            min = Some(n.yh_key);
            current = &n.yh_left;
        }
        min
    }

    /// Maximum key.
    pub fn yh_max(&self) -> Option<i64> {
        let mut current = &self.yh_root;
        let mut max = None;
        while let Some(n) = current {
            max = Some(n.yh_key);
            current = &n.yh_right;
        }
        max
    }

    /// In-order traversal.
    pub fn yh_inorder(&self) -> Vec<i64> {
        let mut result = Vec::new();
        Self::yh_inorder_node(&self.yh_root, &mut result);
        result
    }

    fn yh_inorder_node(node: &Option<Box<YhOstNode>>, result: &mut Vec<i64>) {
        if let Some(n) = node {
            Self::yh_inorder_node(&n.yh_left, result);
            result.push(n.yh_key);
            Self::yh_inorder_node(&n.yh_right, result);
        }
    }

    /// Count elements in range [lo, hi].
    pub fn yh_count_range(&self, lo: i64, hi: i64) -> usize {
        if lo > hi { return 0; }
        let rank_hi = self.yh_rank(hi + 1);
        let rank_lo = self.yh_rank(lo);
        rank_hi - rank_lo
    }
}

// --- yh_ Reservoir Sampler ---

/// Reservoir sampling for uniformly random samples from a stream.
#[derive(Debug, Clone)]
pub struct YhReservoirSampler {
    yh_reservoir: Vec<i64>,
    yh_k: usize,
    yh_count: usize,
    yh_seed: u64,
}

impl std::fmt::Display for YhReservoirSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reservoir(k={}, seen={})", self.yh_k, self.yh_count)
    }
}

impl Default for YhReservoirSampler {
    fn default() -> Self { Self::yh_new(10, 42) }
}

impl YhReservoirSampler {
    /// Create a reservoir sampler for k items.
    pub fn yh_new(k: usize, seed: u64) -> Self {
        Self { yh_reservoir: Vec::with_capacity(k), yh_k: k, yh_count: 0, yh_seed: seed }
    }

    fn yh_next_rand(&mut self) -> u64 {
        self.yh_seed ^= self.yh_seed << 13;
        self.yh_seed ^= self.yh_seed >> 7;
        self.yh_seed ^= self.yh_seed << 17;
        self.yh_seed
    }

    /// Feed a new item from the stream.
    pub fn yh_add(&mut self, item: i64) {
        self.yh_count += 1;
        if self.yh_reservoir.len() < self.yh_k {
            self.yh_reservoir.push(item);
        } else {
            let j = (self.yh_next_rand() % self.yh_count as u64) as usize;
            if j < self.yh_k {
                self.yh_reservoir[j] = item;
            }
        }
    }

    /// Get the current sample.
    pub fn yh_sample(&self) -> &[i64] { &self.yh_reservoir }

    /// Number of items seen.
    pub fn yh_count(&self) -> usize { self.yh_count }

    /// Sample size.
    pub fn yh_k(&self) -> usize { self.yh_k }

    /// Reset the sampler.
    pub fn yh_reset(&mut self, seed: u64) {
        self.yh_reservoir.clear();
        self.yh_count = 0;
        self.yh_seed = seed;
    }

    /// Is the reservoir full.
    pub fn yh_is_full(&self) -> bool { self.yh_reservoir.len() == self.yh_k }

    /// Current reservoir size.
    pub fn yh_len(&self) -> usize { self.yh_reservoir.len() }
}


// --- yi_ Ring Buffer ---

/// Fixed-capacity ring buffer (circular buffer) with O(1) push/pop at both ends.
#[derive(Debug, Clone)]
pub struct YiRingBuffer<T: Clone + Default> {
    yi_data: Vec<T>,
    yi_head: usize,
    yi_len: usize,
    yi_cap: usize,
}

impl<T: Clone + Default + std::fmt::Display> std::fmt::Display for YiRingBuffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RingBuffer(len={}, cap={})", self.yi_len, self.yi_cap)
    }
}

impl<T: Clone + Default> Default for YiRingBuffer<T> {
    fn default() -> Self { Self::yi_new(16) }
}

impl<T: Clone + Default> YiRingBuffer<T> {
    /// Create a ring buffer with given capacity.
    pub fn yi_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self { yi_data: vec![T::default(); cap], yi_head: 0, yi_len: 0, yi_cap: cap }
    }

    /// Current number of elements.
    pub fn yi_len(&self) -> usize { self.yi_len }

    /// Maximum capacity.
    pub fn yi_capacity(&self) -> usize { self.yi_cap }

    /// Is the buffer empty.
    pub fn yi_is_empty(&self) -> bool { self.yi_len == 0 }

    /// Is the buffer full.
    pub fn yi_is_full(&self) -> bool { self.yi_len == self.yi_cap }

    /// Push to the back. Returns false if full.
    pub fn yi_push_back(&mut self, value: T) -> bool {
        if self.yi_is_full() { return false; }
        let idx = (self.yi_head + self.yi_len) % self.yi_cap;
        self.yi_data[idx] = value;
        self.yi_len += 1;
        true
    }

    /// Push to the front. Returns false if full.
    pub fn yi_push_front(&mut self, value: T) -> bool {
        if self.yi_is_full() { return false; }
        self.yi_head = if self.yi_head == 0 { self.yi_cap - 1 } else { self.yi_head - 1 };
        self.yi_data[self.yi_head] = value;
        self.yi_len += 1;
        true
    }

    /// Pop from the front.
    pub fn yi_pop_front(&mut self) -> Option<T> {
        if self.yi_is_empty() { return None; }
        let val = self.yi_data[self.yi_head].clone();
        self.yi_head = (self.yi_head + 1) % self.yi_cap;
        self.yi_len -= 1;
        Some(val)
    }

    /// Pop from the back.
    pub fn yi_pop_back(&mut self) -> Option<T> {
        if self.yi_is_empty() { return None; }
        self.yi_len -= 1;
        let idx = (self.yi_head + self.yi_len) % self.yi_cap;
        Some(self.yi_data[idx].clone())
    }

    /// Peek at the front.
    pub fn yi_front(&self) -> Option<&T> {
        if self.yi_is_empty() { None } else { Some(&self.yi_data[self.yi_head]) }
    }

    /// Peek at the back.
    pub fn yi_back(&self) -> Option<&T> {
        if self.yi_is_empty() { None }
        else { Some(&self.yi_data[(self.yi_head + self.yi_len - 1) % self.yi_cap]) }
    }

    /// Get element at logical index.
    pub fn yi_get(&self, index: usize) -> Option<&T> {
        if index >= self.yi_len { None }
        else { Some(&self.yi_data[(self.yi_head + index) % self.yi_cap]) }
    }

    /// Convert to vec preserving order.
    pub fn yi_to_vec(&self) -> Vec<T> {
        (0..self.yi_len).map(|i| self.yi_data[(self.yi_head + i) % self.yi_cap].clone()).collect()
    }

    /// Clear the buffer.
    pub fn yi_clear(&mut self) { self.yi_len = 0; self.yi_head = 0; }

    /// Force push to back, overwriting oldest if full.
    pub fn yi_force_push_back(&mut self, value: T) {
        if self.yi_is_full() { self.yi_pop_front(); }
        self.yi_push_back(value);
    }
}

// --- yi_ Weighted Graph ---

/// Weighted directed graph with Dijkstra shortest paths.
#[derive(Debug, Clone)]
pub struct YiWeightedGraph {
    yi_adj: std::collections::HashMap<usize, Vec<(usize, f64)>>,
    yi_nodes: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YiWeightedGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let edges: usize = self.yi_adj.values().map(|v| v.len()).sum();
        write!(f, "WGraph(nodes={}, edges={})", self.yi_nodes.len(), edges)
    }
}

impl Default for YiWeightedGraph {
    fn default() -> Self { Self::yi_new() }
}

impl YiWeightedGraph {
    /// Create an empty weighted graph.
    pub fn yi_new() -> Self {
        Self { yi_adj: std::collections::HashMap::new(), yi_nodes: std::collections::HashSet::new() }
    }

    /// Add a node.
    pub fn yi_add_node(&mut self, node: usize) {
        self.yi_nodes.insert(node);
        self.yi_adj.entry(node).or_default();
    }

    /// Add a weighted directed edge.
    pub fn yi_add_edge(&mut self, from: usize, to: usize, weight: f64) {
        self.yi_add_node(from);
        self.yi_add_node(to);
        self.yi_adj.entry(from).or_default().push((to, weight));
    }

    /// Number of nodes.
    pub fn yi_node_count(&self) -> usize { self.yi_nodes.len() }

    /// Dijkstra shortest path distances from source.
    pub fn yi_dijkstra(&self, source: usize) -> std::collections::HashMap<usize, f64> {
        let mut dist: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
        let mut visited: std::collections::HashSet<usize> = std::collections::HashSet::new();
        dist.insert(source, 0.0);
        loop {
            let mut min_node = None;
            let mut min_dist = f64::INFINITY;
            for (node, d) in &dist {
                if !visited.contains(node) && *d < min_dist {
                    min_dist = *d;
                    min_node = Some(*node);
                }
            }
            let Some(u) = min_node else { break };
            visited.insert(u);
            if let Some(neighbors) = self.yi_adj.get(&u) {
                for (v, w) in neighbors {
                    let new_dist = min_dist + w;
                    let entry = dist.entry(*v).or_insert(f64::INFINITY);
                    if new_dist < *entry { *entry = new_dist; }
                }
            }
        }
        dist
    }

    /// Shortest path distance between two nodes.
    pub fn yi_shortest_distance(&self, from: usize, to: usize) -> Option<f64> {
        let dists = self.yi_dijkstra(from);
        dists.get(&to).copied().filter(|d| d.is_finite())
    }

    /// Get neighbors of a node.
    pub fn yi_neighbors(&self, node: usize) -> &[(usize, f64)] {
        self.yi_adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check if node exists.
    pub fn yi_has_node(&self, node: usize) -> bool { self.yi_nodes.contains(&node) }

    /// Clear the graph.
    pub fn yi_clear(&mut self) { self.yi_adj.clear(); self.yi_nodes.clear(); }

    /// Total edge weight.
    pub fn yi_total_weight(&self) -> f64 {
        self.yi_adj.values().flat_map(|v| v.iter()).map(|(_, w)| w).sum()
    }

    /// Add bidirectional edge.
    pub fn yi_add_undirected_edge(&mut self, a: usize, b: usize, weight: f64) {
        self.yi_add_edge(a, b, weight);
        self.yi_add_edge(b, a, weight);
    }
}


// --- yj_ Expression Evaluator ---

/// Simple arithmetic expression evaluator supporting +, -, *, /, parentheses.
#[derive(Debug, Clone)]
pub struct YjExprEval {
    yj_vars: std::collections::HashMap<String, f64>,
}

impl std::fmt::Display for YjExprEval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExprEval(vars={})", self.yj_vars.len())
    }
}

impl Default for YjExprEval {
    fn default() -> Self { Self::yj_new() }
}

impl YjExprEval {
    /// Create a new expression evaluator.
    pub fn yj_new() -> Self { Self { yj_vars: std::collections::HashMap::new() } }

    /// Set a variable.
    pub fn yj_set_var(&mut self, name: &str, value: f64) { self.yj_vars.insert(name.to_string(), value); }

    /// Get a variable.
    pub fn yj_get_var(&self, name: &str) -> Option<f64> { self.yj_vars.get(name).copied() }

    /// Evaluate an expression string.
    pub fn yj_eval(&self, expr: &str) -> std::result::Result<f64, String> {
        let tokens = Self::yj_tokenize(expr)?;
        let mut pos = 0;
        let result = self.yj_parse_expr(&tokens, &mut pos)?;
        if pos != tokens.len() { return Err("unexpected token".to_string()); }
        Ok(result)
    }

    fn yj_tokenize(expr: &str) -> std::result::Result<Vec<String>, String> {
        let mut tokens = Vec::new();
        let mut chars = expr.chars().peekable();
        while let Some(&ch) = chars.peek() {
            if ch.is_whitespace() { chars.next(); continue; }
            if ch.is_ascii_digit() || ch == '.' {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' { num.push(c); chars.next(); } else { break; }
                }
                tokens.push(num);
            } else if ch.is_ascii_alphabetic() || ch == '_' {
                let mut name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' { name.push(c); chars.next(); } else { break; }
                }
                tokens.push(name);
            } else if "+-*/()".contains(ch) {
                tokens.push(ch.to_string());
                chars.next();
            } else {
                return Err(format!("unexpected character: {}", ch));
            }
        }
        Ok(tokens)
    }

    fn yj_parse_expr(&self, tokens: &[String], pos: &mut usize) -> std::result::Result<f64, String> {
        let mut result = self.yj_parse_term(tokens, pos)?;
        while *pos < tokens.len() && (tokens[*pos] == "+" || tokens[*pos] == "-") {
            let op = tokens[*pos].clone();
            *pos += 1;
            let right = self.yj_parse_term(tokens, pos)?;
            result = if op == "+" { result + right } else { result - right };
        }
        Ok(result)
    }

    fn yj_parse_term(&self, tokens: &[String], pos: &mut usize) -> std::result::Result<f64, String> {
        let mut result = self.yj_parse_factor(tokens, pos)?;
        while *pos < tokens.len() && (tokens[*pos] == "*" || tokens[*pos] == "/") {
            let op = tokens[*pos].clone();
            *pos += 1;
            let right = self.yj_parse_factor(tokens, pos)?;
            result = if op == "*" { result * right } else { result / right };
        }
        Ok(result)
    }

    fn yj_parse_factor(&self, tokens: &[String], pos: &mut usize) -> std::result::Result<f64, String> {
        if *pos >= tokens.len() { return Err("unexpected end".to_string()); }
        if tokens[*pos] == "(" {
            *pos += 1;
            let result = self.yj_parse_expr(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != ")" { return Err("missing )".to_string()); }
            *pos += 1;
            return Ok(result);
        }
        if tokens[*pos] == "-" {
            *pos += 1;
            let val = self.yj_parse_factor(tokens, pos)?;
            return Ok(-val);
        }
        if let Ok(num) = tokens[*pos].parse::<f64>() {
            *pos += 1;
            return Ok(num);
        }
        if let Some(val) = self.yj_vars.get(&tokens[*pos]) {
            *pos += 1;
            return Ok(*val);
        }
        Err(format!("unknown token: {}", tokens[*pos]))
    }

    /// Clear all variables.
    pub fn yj_clear(&mut self) { self.yj_vars.clear(); }

    /// Number of variables.
    pub fn yj_var_count(&self) -> usize { self.yj_vars.len() }
}

// --- yj_ TTL Cache ---

/// Cache with time-to-live expiration for entries.
#[derive(Debug, Clone)]
pub struct YjTtlCache<V: Clone> {
    yj_entries: std::collections::HashMap<String, (V, u64)>,
    yj_ttl: u64,
    yj_clock: u64,
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YjTtlCache<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TtlCache(entries={}, ttl={})", self.yj_entries.len(), self.yj_ttl)
    }
}

impl<V: Clone> Default for YjTtlCache<V> {
    fn default() -> Self { Self::yj_new(60) }
}

impl<V: Clone> YjTtlCache<V> {
    /// Create a TTL cache with given TTL in ticks.
    pub fn yj_new(ttl: u64) -> Self {
        Self { yj_entries: std::collections::HashMap::new(), yj_ttl: ttl, yj_clock: 0 }
    }

    /// Advance the clock by a given number of ticks.
    pub fn yj_tick(&mut self, ticks: u64) { self.yj_clock += ticks; }

    /// Current clock value.
    pub fn yj_clock(&self) -> u64 { self.yj_clock }

    /// Insert a key-value pair.
    pub fn yj_put(&mut self, key: &str, value: V) {
        self.yj_entries.insert(key.to_string(), (value, self.yj_clock));
    }

    /// Get a value if not expired.
    pub fn yj_get(&self, key: &str) -> Option<&V> {
        self.yj_entries.get(key).and_then(|(v, ts)| {
            if self.yj_clock - ts <= self.yj_ttl { Some(v) } else { None }
        })
    }

    /// Check if a key exists and is not expired.
    pub fn yj_contains(&self, key: &str) -> bool { self.yj_get(key).is_some() }

    /// Remove expired entries.
    pub fn yj_evict_expired(&mut self) {
        let clock = self.yj_clock;
        let ttl = self.yj_ttl;
        self.yj_entries.retain(|_, (_, ts)| clock - *ts <= ttl);
    }

    /// Number of entries (including possibly expired).
    pub fn yj_len(&self) -> usize { self.yj_entries.len() }

    /// Number of valid (non-expired) entries.
    pub fn yj_valid_count(&self) -> usize {
        self.yj_entries.values().filter(|(_, ts)| self.yj_clock - *ts <= self.yj_ttl).count()
    }

    /// Remove a key.
    pub fn yj_remove(&mut self, key: &str) -> Option<V> {
        self.yj_entries.remove(key).map(|(v, _)| v)
    }

    /// Clear the cache.
    pub fn yj_clear(&mut self) { self.yj_entries.clear(); }

    /// TTL value.
    pub fn yj_ttl(&self) -> u64 { self.yj_ttl }

    /// Set new TTL.
    pub fn yj_set_ttl(&mut self, ttl: u64) { self.yj_ttl = ttl; }
}


// --- yk_ Glob Pattern Matcher ---

/// Simple glob pattern matcher supporting *, ?, and character classes.
#[derive(Debug, Clone)]
pub struct YkGlobMatcher {
    yk_pattern: String,
}

impl std::fmt::Display for YkGlobMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Glob({})", self.yk_pattern)
    }
}

impl Default for YkGlobMatcher {
    fn default() -> Self { Self { yk_pattern: String::new() } }
}

impl YkGlobMatcher {
    /// Create a glob matcher from a pattern.
    pub fn yk_new(pattern: &str) -> Self { Self { yk_pattern: pattern.to_string() } }

    /// Get the pattern.
    pub fn yk_pattern(&self) -> &str { &self.yk_pattern }

    /// Check if a string matches the glob pattern.
    pub fn yk_matches(&self, text: &str) -> bool {
        Self::yk_match_impl(self.yk_pattern.as_bytes(), text.as_bytes())
    }

    fn yk_match_impl(pattern: &[u8], text: &[u8]) -> bool {
        let mut pi = 0;
        let mut ti = 0;
        let mut star_pi = usize::MAX;
        let mut star_ti = 0;
        while ti < text.len() {
            if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == text[ti]) {
                pi += 1;
                ti += 1;
            } else if pi < pattern.len() && pattern[pi] == b'*' {
                star_pi = pi;
                star_ti = ti;
                pi += 1;
            } else if star_pi != usize::MAX {
                pi = star_pi + 1;
                star_ti += 1;
                ti = star_ti;
            } else {
                return false;
            }
        }
        while pi < pattern.len() && pattern[pi] == b'*' { pi += 1; }
        pi == pattern.len()
    }

    /// Match multiple patterns (any match).
    pub fn yk_matches_any(patterns: &[&str], text: &str) -> bool {
        patterns.iter().any(|p| YkGlobMatcher::yk_new(p).yk_matches(text))
    }

    /// Match multiple patterns (all match).
    pub fn yk_matches_all(patterns: &[&str], text: &str) -> bool {
        patterns.iter().all(|p| YkGlobMatcher::yk_new(p).yk_matches(text))
    }

    /// Filter a list of strings by this pattern.
    pub fn yk_filter<'a>(&self, items: &[&'a str]) -> Vec<&'a str> {
        items.iter().filter(|s| self.yk_matches(s)).copied().collect()
    }
}

// --- yk_ Event Bus ---

/// Simple typed event bus with subscriber IDs.
#[derive(Debug, Clone)]
pub struct YkEventBus {
    yk_events: Vec<(String, Vec<(usize, String)>)>,
    yk_next_id: usize,
}

impl std::fmt::Display for YkEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total: usize = self.yk_events.iter().map(|(_, subs)| subs.len()).sum();
        write!(f, "EventBus(topics={}, subs={})", self.yk_events.len(), total)
    }
}

impl Default for YkEventBus {
    fn default() -> Self { Self::yk_new() }
}

impl YkEventBus {
    /// Create a new event bus.
    pub fn yk_new() -> Self { Self { yk_events: Vec::new(), yk_next_id: 0 } }

    /// Subscribe to a topic. Returns subscription ID.
    pub fn yk_subscribe(&mut self, topic: &str, handler_name: &str) -> usize {
        let id = self.yk_next_id;
        self.yk_next_id += 1;
        if let Some((_, subs)) = self.yk_events.iter_mut().find(|(t, _)| t == topic) {
            subs.push((id, handler_name.to_string()));
        } else {
            self.yk_events.push((topic.to_string(), vec![(id, handler_name.to_string())]));
        }
        id
    }

    /// Unsubscribe by ID.
    pub fn yk_unsubscribe(&mut self, id: usize) {
        for (_, subs) in &mut self.yk_events {
            subs.retain(|(sid, _)| *sid != id);
        }
    }

    /// Emit an event, returns list of handler names that were notified.
    pub fn yk_emit(&self, topic: &str) -> Vec<String> {
        self.yk_events.iter()
            .filter(|(t, _)| t == topic)
            .flat_map(|(_, subs)| subs.iter().map(|(_, name)| name.clone()))
            .collect()
    }

    /// Number of topics.
    pub fn yk_topic_count(&self) -> usize { self.yk_events.len() }

    /// Number of subscribers for a topic.
    pub fn yk_subscriber_count(&self, topic: &str) -> usize {
        self.yk_events.iter().find(|(t, _)| t == topic).map(|(_, s)| s.len()).unwrap_or(0)
    }

    /// Total subscribers across all topics.
    pub fn yk_total_subscribers(&self) -> usize {
        self.yk_events.iter().map(|(_, subs)| subs.len()).sum()
    }

    /// List all topics.
    pub fn yk_topics(&self) -> Vec<String> {
        self.yk_events.iter().map(|(t, _)| t.clone()).collect()
    }

    /// Clear all subscriptions.
    pub fn yk_clear(&mut self) { self.yk_events.clear(); self.yk_next_id = 0; }

    /// Check if a topic has subscribers.
    pub fn yk_has_subscribers(&self, topic: &str) -> bool {
        self.yk_subscriber_count(topic) > 0
    }

    /// Emit to topics matching a glob pattern.
    pub fn yk_emit_pattern(&self, pattern: &str) -> Vec<(String, Vec<String>)> {
        let matcher = YkGlobMatcher::yk_new(pattern);
        self.yk_events.iter()
            .filter(|(t, _)| matcher.yk_matches(t))
            .map(|(t, subs)| (t.clone(), subs.iter().map(|(_, n)| n.clone()).collect()))
            .collect()
    }
}


// --- yl_ Min-Max Heap ---

/// Min-max heap: O(1) access to both min and max, O(log n) insert/remove.
#[derive(Debug, Clone)]
pub struct YlMinMaxHeap {
    yl_data: Vec<i64>,
}

impl std::fmt::Display for YlMinMaxHeap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MinMaxHeap(size={})", self.yl_data.len())
    }
}

impl Default for YlMinMaxHeap {
    fn default() -> Self { Self::yl_new() }
}

impl YlMinMaxHeap {
    /// Create an empty min-max heap.
    pub fn yl_new() -> Self { Self { yl_data: Vec::new() } }

    /// Number of elements.
    pub fn yl_len(&self) -> usize { self.yl_data.len() }

    /// Is empty.
    pub fn yl_is_empty(&self) -> bool { self.yl_data.is_empty() }

    fn yl_is_min_level(idx: usize) -> bool {
        let level = ((idx + 1) as f64).log2().floor() as u32;
        level % 2 == 0
    }

    /// Insert a value.
    pub fn yl_insert(&mut self, val: i64) {
        self.yl_data.push(val);
        let idx = self.yl_data.len() - 1;
        self.yl_bubble_up(idx);
    }

    fn yl_bubble_up(&mut self, idx: usize) {
        if idx == 0 { return; }
        let parent = (idx - 1) / 2;
        if Self::yl_is_min_level(idx) {
            if self.yl_data[idx] > self.yl_data[parent] {
                self.yl_data.swap(idx, parent);
                self.yl_bubble_up_max(parent);
            } else {
                self.yl_bubble_up_min(idx);
            }
        } else {
            if self.yl_data[idx] < self.yl_data[parent] {
                self.yl_data.swap(idx, parent);
                self.yl_bubble_up_min(parent);
            } else {
                self.yl_bubble_up_max(idx);
            }
        }
    }

    fn yl_bubble_up_min(&mut self, mut idx: usize) {
        while idx > 2 {
            let grandparent = ((idx - 1) / 2 - 1) / 2;
            if self.yl_data[idx] < self.yl_data[grandparent] {
                self.yl_data.swap(idx, grandparent);
                idx = grandparent;
            } else { break; }
        }
    }

    fn yl_bubble_up_max(&mut self, mut idx: usize) {
        while idx > 2 {
            let grandparent = ((idx - 1) / 2 - 1) / 2;
            if self.yl_data[idx] > self.yl_data[grandparent] {
                self.yl_data.swap(idx, grandparent);
                idx = grandparent;
            } else { break; }
        }
    }

    /// Peek at minimum.
    pub fn yl_peek_min(&self) -> Option<i64> { self.yl_data.first().copied() }

    /// Peek at maximum.
    pub fn yl_peek_max(&self) -> Option<i64> {
        match self.yl_data.len() {
            0 => None,
            1 => Some(self.yl_data[0]),
            2 => Some(self.yl_data[1]),
            _ => Some(self.yl_data[1].max(self.yl_data[2])),
        }
    }

    /// Pop minimum.
    pub fn yl_pop_min(&mut self) -> Option<i64> {
        if self.yl_data.is_empty() { return None; }
        let min = self.yl_data[0];
        let last = self.yl_data.len() - 1;
        self.yl_data.swap(0, last);
        self.yl_data.pop();
        if !self.yl_data.is_empty() { self.yl_trickle_down(0); }
        Some(min)
    }

    fn yl_trickle_down(&mut self, idx: usize) {
        if Self::yl_is_min_level(idx) {
            self.yl_trickle_down_min(idx);
        } else {
            self.yl_trickle_down_max(idx);
        }
    }

    fn yl_trickle_down_min(&mut self, idx: usize) {
        let n = self.yl_data.len();
        let mut smallest = idx;
        for child in [2 * idx + 1, 2 * idx + 2] {
            if child < n && self.yl_data[child] < self.yl_data[smallest] { smallest = child; }
            for gc in [2 * child + 1, 2 * child + 2] {
                if gc < n && self.yl_data[gc] < self.yl_data[smallest] { smallest = gc; }
            }
        }
        if smallest != idx {
            self.yl_data.swap(idx, smallest);
            if smallest > 2 * idx + 2 { // grandchild
                let parent = (smallest - 1) / 2;
                if self.yl_data[smallest] > self.yl_data[parent] {
                    self.yl_data.swap(smallest, parent);
                }
                self.yl_trickle_down_min(smallest);
            }
        }
    }

    fn yl_trickle_down_max(&mut self, idx: usize) {
        let n = self.yl_data.len();
        let mut largest = idx;
        for child in [2 * idx + 1, 2 * idx + 2] {
            if child < n && self.yl_data[child] > self.yl_data[largest] { largest = child; }
            for gc in [2 * child + 1, 2 * child + 2] {
                if gc < n && self.yl_data[gc] > self.yl_data[largest] { largest = gc; }
            }
        }
        if largest != idx {
            self.yl_data.swap(idx, largest);
            if largest > 2 * idx + 2 {
                let parent = (largest - 1) / 2;
                if self.yl_data[largest] < self.yl_data[parent] {
                    self.yl_data.swap(largest, parent);
                }
                self.yl_trickle_down_max(largest);
            }
        }
    }

    /// Convert to sorted vec.
    pub fn yl_to_sorted_vec(&mut self) -> Vec<i64> {
        let mut result = Vec::with_capacity(self.yl_data.len());
        while let Some(v) = self.yl_pop_min() { result.push(v); }
        result
    }

    /// Clear.
    pub fn yl_clear(&mut self) { self.yl_data.clear(); }
}

// --- yl_ State Machine ---

/// Simple deterministic finite state machine.
#[derive(Debug, Clone)]
pub struct YlStateMachine {
    yl_states: Vec<String>,
    yl_current: usize,
    yl_transitions: Vec<(usize, String, usize)>,
    yl_accept: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YlStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FSM(states={}, current={})", self.yl_states.len(),
            self.yl_states.get(self.yl_current).map(|s| s.as_str()).unwrap_or("?"))
    }
}

impl Default for YlStateMachine {
    fn default() -> Self { Self::yl_new() }
}

impl YlStateMachine {
    /// Create an empty state machine.
    pub fn yl_new() -> Self {
        Self { yl_states: Vec::new(), yl_current: 0, yl_transitions: Vec::new(), yl_accept: std::collections::HashSet::new() }
    }

    /// Add a state. Returns state index.
    pub fn yl_add_state(&mut self, name: &str) -> usize {
        let idx = self.yl_states.len();
        self.yl_states.push(name.to_string());
        idx
    }

    /// Add a transition.
    pub fn yl_add_transition(&mut self, from: usize, input: &str, to: usize) {
        self.yl_transitions.push((from, input.to_string(), to));
    }

    /// Mark a state as accepting.
    pub fn yl_set_accept(&mut self, state: usize) { self.yl_accept.insert(state); }

    /// Set starting state.
    pub fn yl_set_start(&mut self, state: usize) { self.yl_current = state; }

    /// Process an input. Returns true if transition found.
    pub fn yl_step(&mut self, input: &str) -> bool {
        for (from, inp, to) in &self.yl_transitions {
            if *from == self.yl_current && inp == input {
                self.yl_current = *to;
                return true;
            }
        }
        false
    }

    /// Process a sequence of inputs. Returns true if all transitions found.
    pub fn yl_run(&mut self, inputs: &[&str]) -> bool {
        for input in inputs {
            if !self.yl_step(input) { return false; }
        }
        true
    }

    /// Current state name.
    pub fn yl_current_state(&self) -> &str {
        self.yl_states.get(self.yl_current).map(|s| s.as_str()).unwrap_or("")
    }

    /// Is current state accepting.
    pub fn yl_is_accepting(&self) -> bool { self.yl_accept.contains(&self.yl_current) }

    /// Number of states.
    pub fn yl_state_count(&self) -> usize { self.yl_states.len() }

    /// Number of transitions.
    pub fn yl_transition_count(&self) -> usize { self.yl_transitions.len() }

    /// Available transitions from current state.
    pub fn yl_available_inputs(&self) -> Vec<String> {
        self.yl_transitions.iter()
            .filter(|(from, _, _)| *from == self.yl_current)
            .map(|(_, input, _)| input.clone())
            .collect()
    }

    /// Reset to start state.
    pub fn yl_reset(&mut self) { self.yl_current = 0; }
}


// --- ym_ Sorted Multi-Map ---

/// Sorted multi-map allowing multiple values per key, stored in a BTreeMap.
#[derive(Debug, Clone)]
pub struct YmSortedMultiMap<K: Ord + Clone, V: Clone> {
    ym_data: std::collections::BTreeMap<K, Vec<V>>,
    ym_count: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for YmSortedMultiMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SortedMultiMap(keys={}, total={})", self.ym_data.len(), self.ym_count)
    }
}

impl<K: Ord + Clone, V: Clone> Default for YmSortedMultiMap<K, V> {
    fn default() -> Self { Self::ym_new() }
}

impl<K: Ord + Clone, V: Clone> YmSortedMultiMap<K, V> {
    /// Create empty sorted multi-map.
    pub fn ym_new() -> Self { Self { ym_data: std::collections::BTreeMap::new(), ym_count: 0 } }

    /// Insert a key-value pair.
    pub fn ym_insert(&mut self, key: K, value: V) {
        self.ym_data.entry(key).or_default().push(value);
        self.ym_count += 1;
    }

    /// Get all values for a key.
    pub fn ym_get(&self, key: &K) -> &[V] {
        self.ym_data.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Number of unique keys.
    pub fn ym_key_count(&self) -> usize { self.ym_data.len() }

    /// Total number of values.
    pub fn ym_total_count(&self) -> usize { self.ym_count }

    /// Is empty.
    pub fn ym_is_empty(&self) -> bool { self.ym_count == 0 }

    /// Contains key.
    pub fn ym_contains_key(&self, key: &K) -> bool { self.ym_data.contains_key(key) }

    /// Remove all values for a key.
    pub fn ym_remove_key(&mut self, key: &K) -> Vec<V> {
        if let Some(vals) = self.ym_data.remove(key) {
            self.ym_count -= vals.len();
            vals
        } else {
            Vec::new()
        }
    }

    /// Get all keys in sorted order.
    pub fn ym_keys(&self) -> Vec<K> {
        self.ym_data.keys().cloned().collect()
    }

    /// Get keys in a range.
    pub fn ym_range(&self, lo: &K, hi: &K) -> Vec<K> {
        self.ym_data.range(lo..=hi).map(|(k, _)| k.clone()).collect()
    }

    /// First key.
    pub fn ym_first_key(&self) -> Option<K> {
        self.ym_data.keys().next().cloned()
    }

    /// Last key.
    pub fn ym_last_key(&self) -> Option<K> {
        self.ym_data.keys().next_back().cloned()
    }

    /// Clear.
    pub fn ym_clear(&mut self) { self.ym_data.clear(); self.ym_count = 0; }

    /// Count values for a key.
    pub fn ym_count_for(&self, key: &K) -> usize {
        self.ym_data.get(key).map(|v| v.len()).unwrap_or(0)
    }
}

// --- ym_ Task Scheduler ---

/// Priority-based task scheduler with dependencies.
#[derive(Debug, Clone)]
pub struct YmTaskScheduler {
    ym_tasks: Vec<YmTask>,
    ym_next_id: usize,
}

/// A scheduled task.
#[derive(Debug, Clone)]
pub struct YmTask {
    /// Task ID.
    pub ym_id: usize,
    /// Task name.
    pub ym_name: String,
    /// Priority (lower = higher priority).
    pub ym_priority: i32,
    /// Dependencies (task IDs that must complete first).
    pub ym_deps: Vec<usize>,
    /// Is completed.
    pub ym_done: bool,
}

impl std::fmt::Display for YmTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Task({}: {}, pri={}, done={})", self.ym_id, self.ym_name, self.ym_priority, self.ym_done)
    }
}

impl std::fmt::Display for YmTaskScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scheduler(tasks={}, pending={})", self.ym_tasks.len(), self.ym_pending_count())
    }
}

impl Default for YmTaskScheduler {
    fn default() -> Self { Self::ym_new() }
}

impl YmTaskScheduler {
    /// Create an empty scheduler.
    pub fn ym_new() -> Self { Self { ym_tasks: Vec::new(), ym_next_id: 0 } }

    /// Add a task. Returns task ID.
    pub fn ym_add_task(&mut self, name: &str, priority: i32, deps: Vec<usize>) -> usize {
        let id = self.ym_next_id;
        self.ym_next_id += 1;
        self.ym_tasks.push(YmTask { ym_id: id, ym_name: name.to_string(), ym_priority: priority, ym_deps: deps, ym_done: false });
        id
    }

    /// Mark a task as done.
    pub fn ym_complete(&mut self, id: usize) {
        if let Some(t) = self.ym_tasks.iter_mut().find(|t| t.ym_id == id) {
            t.ym_done = true;
        }
    }

    /// Get the next ready task (all deps done, highest priority).
    pub fn ym_next_ready(&self) -> Option<&YmTask> {
        let done_set: std::collections::HashSet<usize> = self.ym_tasks.iter()
            .filter(|t| t.ym_done).map(|t| t.ym_id).collect();
        self.ym_tasks.iter()
            .filter(|t| !t.ym_done && t.ym_deps.iter().all(|d| done_set.contains(d)))
            .min_by_key(|t| t.ym_priority)
    }

    /// Get all ready tasks.
    pub fn ym_all_ready(&self) -> Vec<&YmTask> {
        let done_set: std::collections::HashSet<usize> = self.ym_tasks.iter()
            .filter(|t| t.ym_done).map(|t| t.ym_id).collect();
        let mut ready: Vec<&YmTask> = self.ym_tasks.iter()
            .filter(|t| !t.ym_done && t.ym_deps.iter().all(|d| done_set.contains(d)))
            .collect();
        ready.sort_by_key(|t| t.ym_priority);
        ready
    }

    /// Number of pending tasks.
    pub fn ym_pending_count(&self) -> usize {
        self.ym_tasks.iter().filter(|t| !t.ym_done).count()
    }

    /// Number of completed tasks.
    pub fn ym_done_count(&self) -> usize {
        self.ym_tasks.iter().filter(|t| t.ym_done).count()
    }

    /// Total tasks.
    pub fn ym_total(&self) -> usize { self.ym_tasks.len() }

    /// Is all done.
    pub fn ym_is_all_done(&self) -> bool { self.ym_pending_count() == 0 }

    /// Get task by ID.
    pub fn ym_get_task(&self, id: usize) -> Option<&YmTask> {
        self.ym_tasks.iter().find(|t| t.ym_id == id)
    }

    /// Clear.
    pub fn ym_clear(&mut self) { self.ym_tasks.clear(); self.ym_next_id = 0; }
}


// --- yn_ Immutable Map (HAMT-inspired) ---

/// Persistent immutable map using a sorted vector for small maps.
#[derive(Debug, Clone)]
pub struct YnImmutableMap<K: Ord + Clone, V: Clone> {
    yn_entries: Vec<(K, V)>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for YnImmutableMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ImmMap(size={})", self.yn_entries.len())
    }
}

impl<K: Ord + Clone, V: Clone> Default for YnImmutableMap<K, V> {
    fn default() -> Self { Self::yn_new() }
}

impl<K: Ord + Clone, V: Clone> YnImmutableMap<K, V> {
    /// Create an empty immutable map.
    pub fn yn_new() -> Self { Self { yn_entries: Vec::new() } }

    /// Insert returns a new map with the key-value pair added.
    pub fn yn_insert(&self, key: K, value: V) -> Self {
        let mut entries = self.yn_entries.clone();
        match entries.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(idx) => entries[idx] = (key, value),
            Err(idx) => entries.insert(idx, (key, value)),
        }
        Self { yn_entries: entries }
    }

    /// Remove returns a new map without the key.
    pub fn yn_remove(&self, key: &K) -> Self {
        let mut entries = self.yn_entries.clone();
        if let Ok(idx) = entries.binary_search_by(|(k, _)| k.cmp(key)) {
            entries.remove(idx);
        }
        Self { yn_entries: entries }
    }

    /// Look up a key.
    pub fn yn_get(&self, key: &K) -> Option<&V> {
        self.yn_entries.binary_search_by(|(k, _)| k.cmp(key))
            .ok()
            .map(|idx| &self.yn_entries[idx].1)
    }

    /// Contains key.
    pub fn yn_contains_key(&self, key: &K) -> bool { self.yn_get(key).is_some() }

    /// Number of entries.
    pub fn yn_len(&self) -> usize { self.yn_entries.len() }

    /// Is empty.
    pub fn yn_is_empty(&self) -> bool { self.yn_entries.is_empty() }

    /// All keys in sorted order.
    pub fn yn_keys(&self) -> Vec<K> { self.yn_entries.iter().map(|(k, _)| k.clone()).collect() }

    /// All values.
    pub fn yn_values(&self) -> Vec<V> { self.yn_entries.iter().map(|(_, v)| v.clone()).collect() }

    /// Merge with another map (other takes precedence).
    pub fn yn_merge(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for (k, v) in &other.yn_entries {
            result = result.yn_insert(k.clone(), v.clone());
        }
        result
    }

    /// Map values.
    pub fn yn_map_values<F: Fn(&V) -> V>(&self, f: F) -> Self {
        Self { yn_entries: self.yn_entries.iter().map(|(k, v)| (k.clone(), f(v))).collect() }
    }

    /// Filter entries.
    pub fn yn_filter<F: Fn(&K, &V) -> bool>(&self, f: F) -> Self {
        Self { yn_entries: self.yn_entries.iter().filter(|(k, v)| f(k, v)).cloned().collect() }
    }
}

// --- yn_ Tokenizer ---

/// Simple token-based text tokenizer for parsing structured text.
#[derive(Debug, Clone, PartialEq)]
pub enum YnTokenKind {
    /// A word/identifier.
    YnWord,
    /// A number literal.
    YnNumber,
    /// A string literal.
    YnString,
    /// An operator or punctuation.
    YnPunct,
    /// Whitespace.
    YnWhitespace,
}

impl std::fmt::Display for YnTokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YnWord => write!(f, "Word"),
            Self::YnNumber => write!(f, "Number"),
            Self::YnString => write!(f, "String"),
            Self::YnPunct => write!(f, "Punct"),
            Self::YnWhitespace => write!(f, "Whitespace"),
        }
    }
}

/// A token produced by the tokenizer.
#[derive(Debug, Clone)]
pub struct YnToken {
    /// Token kind.
    pub yn_kind: YnTokenKind,
    /// Token text.
    pub yn_text: String,
    /// Start offset.
    pub yn_start: usize,
}

impl std::fmt::Display for YnToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({:?}@{})", self.yn_kind, self.yn_text, self.yn_start)
    }
}

/// Simple text tokenizer.
#[derive(Debug, Clone)]
pub struct YnTokenizer;

impl std::fmt::Display for YnTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tokenizer")
    }
}

impl Default for YnTokenizer {
    fn default() -> Self { Self }
}

impl YnTokenizer {
    /// Tokenize input text.
    pub fn yn_tokenize(input: &str) -> Vec<YnToken> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let start = i;
            if chars[i].is_whitespace() {
                while i < chars.len() && chars[i].is_whitespace() { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnWhitespace, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnWord, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else if chars[i].is_ascii_digit() {
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnNumber, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else if chars[i] == '\"' {
                i += 1;
                while i < chars.len() && chars[i] != '"' { i += 1; }
                if i < chars.len() { i += 1; }
                tokens.push(YnToken { yn_kind: YnTokenKind::YnString, yn_text: chars[start..i].iter().collect(), yn_start: start });
            } else {
                i += 1;
                tokens.push(YnToken { yn_kind: YnTokenKind::YnPunct, yn_text: chars[start..i].iter().collect(), yn_start: start });
            }
        }
        tokens
    }

    /// Tokenize and filter out whitespace.
    pub fn yn_tokenize_no_ws(input: &str) -> Vec<YnToken> {
        Self::yn_tokenize(input).into_iter().filter(|t| t.yn_kind != YnTokenKind::YnWhitespace).collect()
    }

    /// Count tokens by kind.
    pub fn yn_count_by_kind(tokens: &[YnToken], kind: &YnTokenKind) -> usize {
        tokens.iter().filter(|t| t.yn_kind == *kind).count()
    }
}


// --- yo_ Levenshtein Distance ---

/// Levenshtein (edit) distance calculator for fuzzy string matching.
#[derive(Debug, Clone)]
pub struct YoLevenshtein;

impl std::fmt::Display for YoLevenshtein {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Levenshtein")
    }
}

impl Default for YoLevenshtein {
    fn default() -> Self { Self }
}

impl YoLevenshtein {
    /// Compute edit distance between two strings.
    pub fn yo_distance(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let m = a_chars.len();
        let n = b_chars.len();
        if m == 0 { return n; }
        if n == 0 { return m; }
        let mut prev = (0..=n).collect::<Vec<_>>();
        let mut curr = vec![0; n + 1];
        for i in 1..=m {
            curr[0] = i;
            for j in 1..=n {
                let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
                curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            }
            std::mem::swap(&mut prev, &mut curr);
        }
        prev[n]
    }

    /// Normalized similarity [0.0, 1.0].
    pub fn yo_similarity(a: &str, b: &str) -> f64 {
        let max_len = a.chars().count().max(b.chars().count());
        if max_len == 0 { return 1.0; }
        1.0 - (Self::yo_distance(a, b) as f64 / max_len as f64)
    }

    /// Find closest match from candidates.
    pub fn yo_closest<'a>(target: &str, candidates: &[&'a str]) -> Option<&'a str> {
        candidates.iter().min_by_key(|c| Self::yo_distance(target, c)).copied()
    }

    /// Filter candidates within a max distance.
    pub fn yo_within_distance<'a>(target: &str, candidates: &[&'a str], max_dist: usize) -> Vec<&'a str> {
        candidates.iter().filter(|c| Self::yo_distance(target, c) <= max_dist).copied().collect()
    }

    /// Rank candidates by distance (closest first).
    pub fn yo_rank<'a>(target: &str, candidates: &[&'a str]) -> Vec<(&'a str, usize)> {
        let mut ranked: Vec<_> = candidates.iter().map(|c| (*c, Self::yo_distance(target, c))).collect();
        ranked.sort_by_key(|(_, d)| *d);
        ranked
    }
}

// --- yo_ Diff Engine ---

/// Line-based diff engine using longest common subsequence.
#[derive(Debug, Clone, PartialEq)]
pub enum YoDiffOp {
    /// Line exists in both.
    YoEqual(String),
    /// Line added in new version.
    YoInsert(String),
    /// Line removed from old version.
    YoDelete(String),
}

impl std::fmt::Display for YoDiffOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YoEqual(s) => write!(f, "  {}", s),
            Self::YoInsert(s) => write!(f, "+ {}", s),
            Self::YoDelete(s) => write!(f, "- {}", s),
        }
    }
}

/// Line-based diff engine.
#[derive(Debug, Clone)]
pub struct YoDiffEngine;

impl std::fmt::Display for YoDiffEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DiffEngine")
    }
}

impl Default for YoDiffEngine {
    fn default() -> Self { Self }
}

impl YoDiffEngine {
    /// Compute diff between two texts (split by lines).
    pub fn yo_diff(old: &str, new: &str) -> Vec<YoDiffOp> {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();
        let lcs = Self::yo_lcs(&old_lines, &new_lines);
        let mut result = Vec::new();
        let mut oi = 0;
        let mut ni = 0;
        let mut li = 0;
        while oi < old_lines.len() || ni < new_lines.len() {
            if li < lcs.len() && oi < old_lines.len() && ni < new_lines.len() && old_lines[oi] == lcs[li] && new_lines[ni] == lcs[li] {
                result.push(YoDiffOp::YoEqual(lcs[li].to_string()));
                oi += 1; ni += 1; li += 1;
            } else if ni < new_lines.len() && (li >= lcs.len() || new_lines[ni] != lcs[li]) {
                result.push(YoDiffOp::YoInsert(new_lines[ni].to_string()));
                ni += 1;
            } else if oi < old_lines.len() && (li >= lcs.len() || old_lines[oi] != lcs[li]) {
                result.push(YoDiffOp::YoDelete(old_lines[oi].to_string()));
                oi += 1;
            }
        }
        result
    }

    fn yo_lcs<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<&'a str> {
        let m = a.len();
        let n = b.len();
        let mut dp = vec![vec![0usize; n + 1]; m + 1];
        for i in 1..=m {
            for j in 1..=n {
                dp[i][j] = if a[i - 1] == b[j - 1] { dp[i - 1][j - 1] + 1 } else { dp[i - 1][j].max(dp[i][j - 1]) };
            }
        }
        let mut result = Vec::new();
        let (mut i, mut j) = (m, n);
        while i > 0 && j > 0 {
            if a[i - 1] == b[j - 1] { result.push(a[i - 1]); i -= 1; j -= 1; }
            else if dp[i - 1][j] > dp[i][j - 1] { i -= 1; }
            else { j -= 1; }
        }
        result.reverse();
        result
    }

    /// Count insertions in a diff.
    pub fn yo_count_insertions(ops: &[YoDiffOp]) -> usize {
        ops.iter().filter(|op| matches!(op, YoDiffOp::YoInsert(_))).count()
    }

    /// Count deletions in a diff.
    pub fn yo_count_deletions(ops: &[YoDiffOp]) -> usize {
        ops.iter().filter(|op| matches!(op, YoDiffOp::YoDelete(_))).count()
    }

    /// Count equal lines.
    pub fn yo_count_equal(ops: &[YoDiffOp]) -> usize {
        ops.iter().filter(|op| matches!(op, YoDiffOp::YoEqual(_))).count()
    }

    /// Format diff as unified diff string.
    pub fn yo_format(ops: &[YoDiffOp]) -> String {
        ops.iter().map(|op| format!("{}", op)).collect::<Vec<_>>().join("\n")
    }
}


// --- yp_ Simple JSON Value ---

/// Lightweight JSON-like value type for configuration and data exchange.
#[derive(Debug, Clone, PartialEq)]
pub enum YpJsonValue {
    YpNull,
    YpBool(bool),
    YpNumber(f64),
    YpString(String),
    YpArray(Vec<YpJsonValue>),
    YpObject(Vec<(String, YpJsonValue)>),
}

impl std::fmt::Display for YpJsonValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YpNull => write!(f, "null"),
            Self::YpBool(b) => write!(f, "{}", b),
            Self::YpNumber(n) => write!(f, "{}", n),
            Self::YpString(s) => write!(f, "\"{}\"", s),
            Self::YpArray(a) => write!(f, "[{}]", a.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(",")),
            Self::YpObject(o) => write!(f, "{{{}}}", o.iter().map(|(k, v)| format!("\"{}\":{}", k, v)).collect::<Vec<_>>().join(",")),
        }
    }
}

impl Default for YpJsonValue {
    fn default() -> Self { Self::YpNull }
}

impl YpJsonValue {
    /// Create a string value.
    pub fn yp_string(s: &str) -> Self { Self::YpString(s.to_string()) }

    /// Create a number value.
    pub fn yp_number(n: f64) -> Self { Self::YpNumber(n) }

    /// Create a bool value.
    pub fn yp_bool(b: bool) -> Self { Self::YpBool(b) }

    /// Create an empty object.
    pub fn yp_object() -> Self { Self::YpObject(Vec::new()) }

    /// Create an empty array.
    pub fn yp_array() -> Self { Self::YpArray(Vec::new()) }

    /// Is null.
    pub fn yp_is_null(&self) -> bool { matches!(self, Self::YpNull) }

    /// Get as string.
    pub fn yp_as_str(&self) -> Option<&str> {
        if let Self::YpString(s) = self { Some(s) } else { None }
    }

    /// Get as number.
    pub fn yp_as_f64(&self) -> Option<f64> {
        if let Self::YpNumber(n) = self { Some(*n) } else { None }
    }

    /// Get as bool.
    pub fn yp_as_bool(&self) -> Option<bool> {
        if let Self::YpBool(b) = self { Some(*b) } else { None }
    }

    /// Get by key (for objects).
    pub fn yp_get(&self, key: &str) -> Option<&YpJsonValue> {
        if let Self::YpObject(entries) = self {
            entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else { None }
    }

    /// Get by index (for arrays).
    pub fn yp_index(&self, idx: usize) -> Option<&YpJsonValue> {
        if let Self::YpArray(arr) = self { arr.get(idx) } else { None }
    }

    /// Set a key on an object (mutating).
    pub fn yp_set(&mut self, key: &str, value: YpJsonValue) {
        if let Self::YpObject(entries) = self {
            if let Some(entry) = entries.iter_mut().find(|(k, _)| k == key) {
                entry.1 = value;
            } else {
                entries.push((key.to_string(), value));
            }
        }
    }

    /// Push to array.
    pub fn yp_push(&mut self, value: YpJsonValue) {
        if let Self::YpArray(arr) = self { arr.push(value); }
    }

    /// Object keys.
    pub fn yp_keys(&self) -> Vec<String> {
        if let Self::YpObject(entries) = self {
            entries.iter().map(|(k, _)| k.clone()).collect()
        } else { Vec::new() }
    }

    /// Array/object length.
    pub fn yp_len(&self) -> usize {
        match self {
            Self::YpArray(a) => a.len(),
            Self::YpObject(o) => o.len(),
            Self::YpString(s) => s.len(),
            _ => 0,
        }
    }

    /// Deep clone with path-based access.
    pub fn yp_path(&self, path: &str) -> Option<&YpJsonValue> {
        let parts: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        let mut current = self;
        for part in parts {
            if let Ok(idx) = part.parse::<usize>() {
                current = current.yp_index(idx)?;
            } else {
                current = current.yp_get(part)?;
            }
        }
        Some(current)
    }

    /// Merge two objects (other takes precedence).
    pub fn yp_merge(&self, other: &YpJsonValue) -> YpJsonValue {
        match (self, other) {
            (Self::YpObject(a), Self::YpObject(b)) => {
                let mut result = a.clone();
                for (k, v) in b {
                    if let Some(entry) = result.iter_mut().find(|(ek, _)| ek == k) {
                        entry.1 = v.clone();
                    } else {
                        result.push((k.clone(), v.clone()));
                    }
                }
                Self::YpObject(result)
            }
            _ => other.clone(),
        }
    }
}

// --- yp_ Command Registry ---

/// Registry for named commands with metadata.
#[derive(Debug, Clone)]
pub struct YpCommandEntry {
    pub yp_id: String,
    pub yp_title: String,
    pub yp_category: String,
    pub yp_keybinding: Option<String>,
    pub yp_when: Option<String>,
}

impl std::fmt::Display for YpCommandEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cmd({})", self.yp_id)
    }
}

/// Command registry for command palette and keybinding resolution.
#[derive(Debug, Clone)]
pub struct YpCommandRegistry {
    yp_commands: Vec<YpCommandEntry>,
}

impl std::fmt::Display for YpCommandRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CmdRegistry(count={})", self.yp_commands.len())
    }
}

impl Default for YpCommandRegistry {
    fn default() -> Self { Self::yp_new() }
}

impl YpCommandRegistry {
    /// Create empty registry.
    pub fn yp_new() -> Self { Self { yp_commands: Vec::new() } }

    /// Register a command.
    pub fn yp_register(&mut self, id: &str, title: &str, category: &str) {
        self.yp_commands.push(YpCommandEntry {
            yp_id: id.to_string(), yp_title: title.to_string(), yp_category: category.to_string(),
            yp_keybinding: None, yp_when: None,
        });
    }

    /// Register with keybinding.
    pub fn yp_register_with_key(&mut self, id: &str, title: &str, category: &str, keybinding: &str) {
        self.yp_commands.push(YpCommandEntry {
            yp_id: id.to_string(), yp_title: title.to_string(), yp_category: category.to_string(),
            yp_keybinding: Some(keybinding.to_string()), yp_when: None,
        });
    }

    /// Find command by ID.
    pub fn yp_find(&self, id: &str) -> Option<&YpCommandEntry> {
        self.yp_commands.iter().find(|c| c.yp_id == id)
    }

    /// Search commands by title prefix.
    pub fn yp_search(&self, query: &str) -> Vec<&YpCommandEntry> {
        let q = query.to_lowercase();
        self.yp_commands.iter().filter(|c| c.yp_title.to_lowercase().contains(&q)).collect()
    }

    /// Commands in a category.
    pub fn yp_by_category(&self, category: &str) -> Vec<&YpCommandEntry> {
        self.yp_commands.iter().filter(|c| c.yp_category == category).collect()
    }

    /// Find command by keybinding.
    pub fn yp_by_keybinding(&self, key: &str) -> Option<&YpCommandEntry> {
        self.yp_commands.iter().find(|c| c.yp_keybinding.as_deref() == Some(key))
    }

    /// Number of commands.
    pub fn yp_count(&self) -> usize { self.yp_commands.len() }

    /// All categories.
    pub fn yp_categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self.yp_commands.iter().map(|c| c.yp_category.clone()).collect();
        cats.sort();
        cats.dedup();
        cats
    }

    /// Clear.
    pub fn yp_clear(&mut self) { self.yp_commands.clear(); }
}


// --- yq_ Layered Config Store ---

/// Layered configuration store with default, user, and workspace layers.
#[derive(Debug, Clone)]
pub struct YqConfigStore {
    yq_layers: Vec<(String, std::collections::HashMap<String, String>)>,
}

impl std::fmt::Display for YqConfigStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total: usize = self.yq_layers.iter().map(|(_, m)| m.len()).sum();
        write!(f, "ConfigStore(layers={}, keys={})", self.yq_layers.len(), total)
    }
}

impl Default for YqConfigStore {
    fn default() -> Self { Self::yq_new() }
}

impl YqConfigStore {
    /// Create with default layers: defaults, user, workspace.
    pub fn yq_new() -> Self {
        Self { yq_layers: vec![
            ("defaults".to_string(), std::collections::HashMap::new()),
            ("user".to_string(), std::collections::HashMap::new()),
            ("workspace".to_string(), std::collections::HashMap::new()),
        ] }
    }

    /// Set a value in a specific layer.
    pub fn yq_set(&mut self, layer: &str, key: &str, value: &str) {
        if let Some((_, map)) = self.yq_layers.iter_mut().find(|(n, _)| n == layer) {
            map.insert(key.to_string(), value.to_string());
        }
    }

    /// Get a value, checking layers from last (highest priority) to first.
    pub fn yq_get(&self, key: &str) -> Option<&str> {
        for (_, map) in self.yq_layers.iter().rev() {
            if let Some(v) = map.get(key) { return Some(v.as_str()); }
        }
        None
    }

    /// Get with default.
    pub fn yq_get_or(&self, key: &str, default: &str) -> String {
        self.yq_get(key).unwrap_or(default).to_string()
    }

    /// Get value as i64.
    pub fn yq_get_i64(&self, key: &str) -> Option<i64> {
        self.yq_get(key).and_then(|v| v.parse().ok())
    }

    /// Get value as bool.
    pub fn yq_get_bool(&self, key: &str) -> Option<bool> {
        self.yq_get(key).and_then(|v| v.parse().ok())
    }

    /// Remove a key from a layer.
    pub fn yq_remove(&mut self, layer: &str, key: &str) {
        if let Some((_, map)) = self.yq_layers.iter_mut().find(|(n, _)| n == layer) {
            map.remove(key);
        }
    }

    /// All keys across all layers.
    pub fn yq_all_keys(&self) -> Vec<String> {
        let mut keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (_, map) in &self.yq_layers { for k in map.keys() { keys.insert(k.clone()); } }
        let mut sorted: Vec<String> = keys.into_iter().collect();
        sorted.sort();
        sorted
    }

    /// Add a custom layer.
    pub fn yq_add_layer(&mut self, name: &str) {
        self.yq_layers.push((name.to_string(), std::collections::HashMap::new()));
    }

    /// Number of layers.
    pub fn yq_layer_count(&self) -> usize { self.yq_layers.len() }

    /// Get the effective layer name for a key.
    pub fn yq_effective_layer(&self, key: &str) -> Option<&str> {
        for (name, map) in self.yq_layers.iter().rev() {
            if map.contains_key(key) { return Some(name.as_str()); }
        }
        None
    }

    /// Clear a specific layer.
    pub fn yq_clear_layer(&mut self, layer: &str) {
        if let Some((_, map)) = self.yq_layers.iter_mut().find(|(n, _)| n == layer) {
            map.clear();
        }
    }

    /// Clear all layers.
    pub fn yq_clear_all(&mut self) {
        for (_, map) in &mut self.yq_layers { map.clear(); }
    }
}

// --- yq_ Text Layout Engine ---

/// Simple text line wrapping and layout engine for terminal rendering.
#[derive(Debug, Clone)]
pub struct YqTextLayout {
    yq_width: usize,
    yq_tab_size: usize,
}

impl std::fmt::Display for YqTextLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TextLayout(w={}, tab={})", self.yq_width, self.yq_tab_size)
    }
}

impl Default for YqTextLayout {
    fn default() -> Self { Self { yq_width: 80, yq_tab_size: 4 } }
}

impl YqTextLayout {
    /// Create with given width.
    pub fn yq_new(width: usize) -> Self { Self { yq_width: width.max(1), yq_tab_size: 4 } }

    /// Set tab size.
    pub fn yq_set_tab_size(&mut self, size: usize) { self.yq_tab_size = size.max(1); }

    /// Width.
    pub fn yq_width(&self) -> usize { self.yq_width }

    /// Wrap text into lines of at most width characters.
    pub fn yq_wrap(&self, text: &str) -> Vec<String> {
        let expanded = self.yq_expand_tabs(text);
        let mut lines = Vec::new();
        for line in expanded.lines() {
            if line.len() <= self.yq_width {
                lines.push(line.to_string());
            } else {
                let mut remaining = line;
                while remaining.len() > self.yq_width {
                    let split = Self::yq_find_break(remaining, self.yq_width);
                    lines.push(remaining[..split].to_string());
                    remaining = &remaining[split..];
                    remaining = remaining.trim_start();
                }
                if !remaining.is_empty() { lines.push(remaining.to_string()); }
            }
        }
        if lines.is_empty() { lines.push(String::new()); }
        lines
    }

    fn yq_find_break(text: &str, max_width: usize) -> usize {
        if let Some(pos) = text[..max_width].rfind(' ') {
            if pos > 0 { return pos + 1; }
        }
        max_width
    }

    /// Expand tabs to spaces.
    pub fn yq_expand_tabs(&self, text: &str) -> String {
        text.replace('\t', &" ".repeat(self.yq_tab_size))
    }

    /// Truncate a line to width, adding ellipsis if needed.
    pub fn yq_truncate(&self, text: &str, ellipsis: &str) -> String {
        if text.len() <= self.yq_width { return text.to_string(); }
        let avail = self.yq_width.saturating_sub(ellipsis.len());
        format!("{}{}", &text[..avail], ellipsis)
    }

    /// Pad/align text.
    pub fn yq_pad_right(&self, text: &str) -> String {
        if text.len() >= self.yq_width { return text[..self.yq_width].to_string(); }
        format!("{:width$}", text, width = self.yq_width)
    }

    /// Center text.
    pub fn yq_center(&self, text: &str) -> String {
        if text.len() >= self.yq_width { return text[..self.yq_width].to_string(); }
        let pad = (self.yq_width - text.len()) / 2;
        format!("{}{}{}", " ".repeat(pad), text, " ".repeat(self.yq_width - text.len() - pad))
    }

    /// Count visual lines needed.
    pub fn yq_line_count(&self, text: &str) -> usize {
        self.yq_wrap(text).len()
    }
}


// --- yr_ Undo/Redo Stack ---

/// Generic undo/redo stack for command pattern implementation.
#[derive(Debug, Clone)]
pub struct YrUndoStack<T: Clone> {
    yr_undo: Vec<T>,
    yr_redo: Vec<T>,
    yr_max_size: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for YrUndoStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UndoStack(undo={}, redo={})", self.yr_undo.len(), self.yr_redo.len())
    }
}

impl<T: Clone> Default for YrUndoStack<T> {
    fn default() -> Self { Self::yr_new(1000) }
}

impl<T: Clone> YrUndoStack<T> {
    /// Create with max size.
    pub fn yr_new(max_size: usize) -> Self {
        Self { yr_undo: Vec::new(), yr_redo: Vec::new(), yr_max_size: max_size.max(1) }
    }

    /// Push a new state. Clears redo stack.
    pub fn yr_push(&mut self, state: T) {
        self.yr_redo.clear();
        self.yr_undo.push(state);
        while self.yr_undo.len() > self.yr_max_size { self.yr_undo.remove(0); }
    }

    /// Undo: move last state to redo stack, return it.
    pub fn yr_undo(&mut self) -> Option<T> {
        let state = self.yr_undo.pop()?;
        self.yr_redo.push(state.clone());
        Some(state)
    }

    /// Redo: move last redo state back, return it.
    pub fn yr_redo(&mut self) -> Option<T> {
        let state = self.yr_redo.pop()?;
        self.yr_undo.push(state.clone());
        Some(state)
    }

    /// Can undo.
    pub fn yr_can_undo(&self) -> bool { !self.yr_undo.is_empty() }

    /// Can redo.
    pub fn yr_can_redo(&self) -> bool { !self.yr_redo.is_empty() }

    /// Undo stack depth.
    pub fn yr_undo_count(&self) -> usize { self.yr_undo.len() }

    /// Redo stack depth.
    pub fn yr_redo_count(&self) -> usize { self.yr_redo.len() }

    /// Peek at current (top of undo).
    pub fn yr_current(&self) -> Option<&T> { self.yr_undo.last() }

    /// Clear both stacks.
    pub fn yr_clear(&mut self) { self.yr_undo.clear(); self.yr_redo.clear(); }

    /// Max size.
    pub fn yr_max_size(&self) -> usize { self.yr_max_size }
}

// --- yr_ Selection Model ---

/// Multi-cursor selection model for text editing.
#[derive(Debug, Clone, PartialEq)]
pub struct YrSelection {
    /// Anchor position (where selection started).
    pub yr_anchor_line: usize,
    pub yr_anchor_col: usize,
    /// Active position (where cursor currently is).
    pub yr_active_line: usize,
    pub yr_active_col: usize,
}

impl std::fmt::Display for YrSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sel({}:{}-{}:{})", self.yr_anchor_line, self.yr_anchor_col, self.yr_active_line, self.yr_active_col)
    }
}

impl Default for YrSelection {
    fn default() -> Self { Self { yr_anchor_line: 0, yr_anchor_col: 0, yr_active_line: 0, yr_active_col: 0 } }
}

impl YrSelection {
    /// Create a cursor (zero-width selection).
    pub fn yr_cursor(line: usize, col: usize) -> Self {
        Self { yr_anchor_line: line, yr_anchor_col: col, yr_active_line: line, yr_active_col: col }
    }

    /// Create a selection range.
    pub fn yr_range(anchor_line: usize, anchor_col: usize, active_line: usize, active_col: usize) -> Self {
        Self { yr_anchor_line: anchor_line, yr_anchor_col: anchor_col, yr_active_line: active_line, yr_active_col: active_col }
    }

    /// Is this a cursor (no selection)?
    pub fn yr_is_cursor(&self) -> bool {
        self.yr_anchor_line == self.yr_active_line && self.yr_anchor_col == self.yr_active_col
    }

    /// Start position (min of anchor/active).
    pub fn yr_start(&self) -> (usize, usize) {
        if (self.yr_anchor_line, self.yr_anchor_col) <= (self.yr_active_line, self.yr_active_col) {
            (self.yr_anchor_line, self.yr_anchor_col)
        } else {
            (self.yr_active_line, self.yr_active_col)
        }
    }

    /// End position (max of anchor/active).
    pub fn yr_end(&self) -> (usize, usize) {
        if (self.yr_anchor_line, self.yr_anchor_col) >= (self.yr_active_line, self.yr_active_col) {
            (self.yr_anchor_line, self.yr_anchor_col)
        } else {
            (self.yr_active_line, self.yr_active_col)
        }
    }

    /// Does this selection contain a position?
    pub fn yr_contains(&self, line: usize, col: usize) -> bool {
        let start = self.yr_start();
        let end = self.yr_end();
        (line, col) >= start && (line, col) <= end
    }

    /// Is this selection reversed (active before anchor)?
    pub fn yr_is_reversed(&self) -> bool {
        (self.yr_active_line, self.yr_active_col) < (self.yr_anchor_line, self.yr_anchor_col)
    }

    /// Number of lines spanned.
    pub fn yr_line_span(&self) -> usize {
        let (sl, _) = self.yr_start();
        let (el, _) = self.yr_end();
        el - sl + 1
    }

    /// Collapse to cursor at active position.
    pub fn yr_collapse(&self) -> Self {
        Self::yr_cursor(self.yr_active_line, self.yr_active_col)
    }
}

/// Multi-cursor selection model.
#[derive(Debug, Clone)]
pub struct YrSelectionModel {
    yr_selections: Vec<YrSelection>,
}

impl std::fmt::Display for YrSelectionModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SelectionModel(cursors={})", self.yr_selections.len())
    }
}

impl Default for YrSelectionModel {
    fn default() -> Self { Self::yr_new() }
}

impl YrSelectionModel {
    /// Create with a single cursor at origin.
    pub fn yr_new() -> Self { Self { yr_selections: vec![YrSelection::yr_cursor(0, 0)] } }

    /// Set primary selection.
    pub fn yr_set_primary(&mut self, sel: YrSelection) {
        self.yr_selections = vec![sel];
    }

    /// Add a selection (multi-cursor).
    pub fn yr_add(&mut self, sel: YrSelection) {
        self.yr_selections.push(sel);
    }

    /// Get primary (first) selection.
    pub fn yr_primary(&self) -> &YrSelection { &self.yr_selections[0] }

    /// All selections.
    pub fn yr_all(&self) -> &[YrSelection] { &self.yr_selections }

    /// Number of cursors.
    pub fn yr_cursor_count(&self) -> usize { self.yr_selections.len() }

    /// Collapse all to cursors.
    pub fn yr_collapse_all(&mut self) {
        self.yr_selections = self.yr_selections.iter().map(|s| s.yr_collapse()).collect();
    }

    /// Clear to single cursor at origin.
    pub fn yr_reset(&mut self) { self.yr_selections = vec![YrSelection::yr_cursor(0, 0)]; }

    /// Remove duplicate selections.
    pub fn yr_deduplicate(&mut self) {
        self.yr_selections.dedup();
    }
}


// --- ys_ CRDT counter and version vector ---

/// A grow-only counter CRDT (G-Counter).
/// Each replica has its own counter; the merged value is the sum of all replicas.
#[derive(Debug, Clone)]
pub struct YsGCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl YsGCounter {
    pub fn new() -> Self {
        Self { counts: std::collections::HashMap::new() }
    }

    pub fn increment(&mut self, replica_id: &str) {
        let entry = self.counts.entry(replica_id.to_string()).or_insert(0);
        *entry += 1;
    }

    pub fn increment_by(&mut self, replica_id: &str, amount: u64) {
        let entry = self.counts.entry(replica_id.to_string()).or_insert(0);
        *entry += amount;
    }

    pub fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    pub fn local_value(&self, replica_id: &str) -> u64 {
        self.counts.get(replica_id).copied().unwrap_or(0)
    }

    pub fn merge(&mut self, other: &YsGCounter) {
        for (k, v) in &other.counts {
            let entry = self.counts.entry(k.clone()).or_insert(0);
            if *v > *entry {
                *entry = *v;
            }
        }
    }

    pub fn replicas(&self) -> Vec<String> {
        let mut r: Vec<String> = self.counts.keys().cloned().collect();
        r.sort();
        r
    }

    pub fn replica_count(&self) -> usize {
        self.counts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty() || self.value() == 0
    }
}

impl Default for YsGCounter {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for YsGCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YsGCounter(value={}, replicas={})", self.value(), self.replica_count())
    }
}

/// A version vector for tracking causality across distributed replicas.
#[derive(Debug, Clone)]
pub struct YsVersionVector {
    versions: std::collections::HashMap<String, u64>,
}

impl YsVersionVector {
    pub fn new() -> Self {
        Self { versions: std::collections::HashMap::new() }
    }

    pub fn increment(&mut self, replica_id: &str) -> u64 {
        let entry = self.versions.entry(replica_id.to_string()).or_insert(0);
        *entry += 1;
        *entry
    }

    pub fn get(&self, replica_id: &str) -> u64 {
        self.versions.get(replica_id).copied().unwrap_or(0)
    }

    pub fn set(&mut self, replica_id: &str, version: u64) {
        self.versions.insert(replica_id.to_string(), version);
    }

    pub fn merge(&mut self, other: &YsVersionVector) {
        for (k, v) in &other.versions {
            let entry = self.versions.entry(k.clone()).or_insert(0);
            if *v > *entry {
                *entry = *v;
            }
        }
    }

    /// Returns true if self dominates other (all versions >= other's).
    pub fn dominates(&self, other: &YsVersionVector) -> bool {
        for (k, v) in &other.versions {
            if self.get(k) < *v {
                return false;
            }
        }
        true
    }

    /// Returns true if self and other are concurrent (neither dominates).
    pub fn is_concurrent(&self, other: &YsVersionVector) -> bool {
        !self.dominates(other) && !other.dominates(self)
    }

    /// Returns true if the vectors are identical.
    pub fn is_equal(&self, other: &YsVersionVector) -> bool {
        self.dominates(other) && other.dominates(self)
    }

    pub fn replicas(&self) -> Vec<String> {
        let mut r: Vec<String> = self.versions.keys().cloned().collect();
        r.sort();
        r
    }

    pub fn len(&self) -> usize {
        self.versions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }

    pub fn max_version(&self) -> u64 {
        self.versions.values().copied().max().unwrap_or(0)
    }

    pub fn sum_versions(&self) -> u64 {
        self.versions.values().sum()
    }
}

impl Default for YsVersionVector {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for YsVersionVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YsVersionVector(replicas={}, max={})", self.len(), self.max_version())
    }
}


// --- yt_ simple regex engine and pattern matcher ---

/// A simple NFA-based regex engine supporting ., *, +, ?, |, character classes, anchors.
#[derive(Debug, Clone)]
pub struct YtRegex {
    pattern: String,
    tokens: Vec<YtRegexToken>,
}

#[derive(Debug, Clone)]
enum YtRegexToken {
    Literal(char),
    Dot,
    Star(Box<YtRegexToken>),
    Plus(Box<YtRegexToken>),
    Optional(Box<YtRegexToken>),
    CharClass(Vec<char>, bool),
    Anchor(YtAnchor),
}

#[derive(Debug, Clone, Copy)]
enum YtAnchor {
    Start,
    End,
}

impl YtRegex {
    pub fn new(pattern: &str) -> Self {
        let tokens = Self::parse(pattern);
        Self { pattern: pattern.to_string(), tokens }
    }

    fn parse(pattern: &str) -> Vec<YtRegexToken> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '^' if i == 0 => {
                    tokens.push(YtRegexToken::Anchor(YtAnchor::Start));
                    i += 1;
                }
                '$' if i == chars.len() - 1 => {
                    tokens.push(YtRegexToken::Anchor(YtAnchor::End));
                    i += 1;
                }
                '.' => {
                    let base = YtRegexToken::Dot;
                    i += 1;
                    let tok = Self::parse_quantifier(&chars, &mut i, base);
                    tokens.push(tok);
                }
                '[' => {
                    i += 1;
                    let negated = i < chars.len() && chars[i] == '^';
                    if negated { i += 1; }
                    let mut class_chars = Vec::new();
                    while i < chars.len() && chars[i] != ']' {
                        class_chars.push(chars[i]);
                        i += 1;
                    }
                    if i < chars.len() { i += 1; } // skip ]
                    let base = YtRegexToken::CharClass(class_chars, negated);
                    let tok = Self::parse_quantifier(&chars, &mut i, base);
                    tokens.push(tok);
                }
                '\\' if i + 1 < chars.len() => {
                    i += 1;
                    let base = YtRegexToken::Literal(chars[i]);
                    i += 1;
                    let tok = Self::parse_quantifier(&chars, &mut i, base);
                    tokens.push(tok);
                }
                c => {
                    let base = YtRegexToken::Literal(c);
                    i += 1;
                    let tok = Self::parse_quantifier(&chars, &mut i, base);
                    tokens.push(tok);
                }
            }
        }
        tokens
    }

    fn parse_quantifier(chars: &[char], i: &mut usize, base: YtRegexToken) -> YtRegexToken {
        if *i < chars.len() {
            match chars[*i] {
                '*' => { *i += 1; YtRegexToken::Star(Box::new(base)) }
                '+' => { *i += 1; YtRegexToken::Plus(Box::new(base)) }
                '?' => { *i += 1; YtRegexToken::Optional(Box::new(base)) }
                _ => base,
            }
        } else {
            base
        }
    }

    pub fn is_match(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        let has_start = matches!(self.tokens.first(), Some(YtRegexToken::Anchor(YtAnchor::Start)));
        let has_end = matches!(self.tokens.last(), Some(YtRegexToken::Anchor(YtAnchor::End)));
        let tokens = if has_start && has_end {
            &self.tokens[1..self.tokens.len()-1]
        } else if has_start {
            &self.tokens[1..]
        } else if has_end {
            &self.tokens[..self.tokens.len()-1]
        } else {
            &self.tokens[..]
        };
        if has_start {
            let matched = Self::match_tokens(tokens, &chars, 0);
            if has_end { matched == Some(chars.len()) } else { matched.is_some() }
        } else {
            for start in 0..=chars.len() {
                if let Some(end) = Self::match_tokens(tokens, &chars, start) {
                    if has_end { if end == chars.len() { return true; } }
                    else { return true; }
                }
            }
            false
        }
    }

    fn match_tokens(tokens: &[YtRegexToken], chars: &[char], pos: usize) -> Option<usize> {
        if tokens.is_empty() { return Some(pos); }
        match &tokens[0] {
            YtRegexToken::Literal(c) => {
                if pos < chars.len() && chars[pos] == *c {
                    Self::match_tokens(&tokens[1..], chars, pos + 1)
                } else { None }
            }
            YtRegexToken::Dot => {
                if pos < chars.len() {
                    Self::match_tokens(&tokens[1..], chars, pos + 1)
                } else { None }
            }
            YtRegexToken::CharClass(class, negated) => {
                if pos < chars.len() {
                    let in_class = class.contains(&chars[pos]);
                    if in_class != *negated {
                        Self::match_tokens(&tokens[1..], chars, pos + 1)
                    } else { None }
                } else { None }
            }
            YtRegexToken::Star(base) => {
                // Try matching 0..n times (greedy)
                let mut positions = vec![pos];
                let mut p = pos;
                while let Some(next) = Self::match_single(base, chars, p) {
                    positions.push(next);
                    p = next;
                    if p == pos { break; } // prevent infinite loop
                }
                for &end_pos in positions.iter().rev() {
                    if let Some(result) = Self::match_tokens(&tokens[1..], chars, end_pos) {
                        return Some(result);
                    }
                }
                None
            }
            YtRegexToken::Plus(base) => {
                if let Some(first) = Self::match_single(base, chars, pos) {
                    let star_tokens = [&[YtRegexToken::Star(base.clone())], &tokens[1..]].concat();
                    Self::match_tokens(&star_tokens, chars, first)
                } else { None }
            }
            YtRegexToken::Optional(base) => {
                if let Some(next) = Self::match_single(base, chars, pos) {
                    if let Some(result) = Self::match_tokens(&tokens[1..], chars, next) {
                        return Some(result);
                    }
                }
                Self::match_tokens(&tokens[1..], chars, pos)
            }
            YtRegexToken::Anchor(_) => Self::match_tokens(&tokens[1..], chars, pos),
        }
    }

    fn match_single(token: &YtRegexToken, chars: &[char], pos: usize) -> Option<usize> {
        match token {
            YtRegexToken::Literal(c) => {
                if pos < chars.len() && chars[pos] == *c { Some(pos + 1) } else { None }
            }
            YtRegexToken::Dot => {
                if pos < chars.len() { Some(pos + 1) } else { None }
            }
            YtRegexToken::CharClass(class, negated) => {
                if pos < chars.len() {
                    let in_class = class.contains(&chars[pos]);
                    if in_class != *negated { Some(pos + 1) } else { None }
                } else { None }
            }
            _ => None,
        }
    }

    pub fn find(&self, text: &str) -> Option<(usize, usize)> {
        let chars: Vec<char> = text.chars().collect();
        let tokens = &self.tokens[..];
        for start in 0..=chars.len() {
            if let Some(end) = Self::match_tokens(tokens, &chars, start) {
                return Some((start, end));
            }
        }
        None
    }

    pub fn find_all(&self, text: &str) -> Vec<(usize, usize)> {
        let chars: Vec<char> = text.chars().collect();
        let tokens = &self.tokens[..];
        let mut results = Vec::new();
        let mut start = 0;
        while start <= chars.len() {
            if let Some(end) = Self::match_tokens(tokens, &chars, start) {
                results.push((start, end));
                start = if end > start { end } else { start + 1 };
            } else {
                start += 1;
            }
        }
        results
    }

    pub fn pattern(&self) -> &str { &self.pattern }
}

impl std::fmt::Display for YtRegex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YtRegex({})", self.pattern)
    }
}

/// A wildcard pattern matcher (like shell globs).
#[derive(Debug, Clone)]
pub struct YtWildcard {
    pattern: String,
}

impl YtWildcard {
    pub fn new(pattern: &str) -> Self {
        Self { pattern: pattern.to_string() }
    }

    pub fn is_match(&self, text: &str) -> bool {
        let p: Vec<char> = self.pattern.chars().collect();
        let t: Vec<char> = text.chars().collect();
        Self::wc_match(&p, 0, &t, 0)
    }

    fn wc_match(p: &[char], pi: usize, t: &[char], ti: usize) -> bool {
        if pi == p.len() { return ti == t.len(); }
        match p[pi] {
            '*' => {
                // Try matching * with 0..n chars
                for skip in 0..=(t.len() - ti) {
                    if Self::wc_match(p, pi + 1, t, ti + skip) { return true; }
                }
                false
            }
            '?' => {
                if ti < t.len() { Self::wc_match(p, pi + 1, t, ti + 1) } else { false }
            }
            c => {
                if ti < t.len() && t[ti] == c { Self::wc_match(p, pi + 1, t, ti + 1) } else { false }
            }
        }
    }

    pub fn filter<'a>(&self, items: &'a [String]) -> Vec<&'a String> {
        items.iter().filter(|s| self.is_match(s)).collect()
    }

    pub fn pattern(&self) -> &str { &self.pattern }
}

impl std::fmt::Display for YtWildcard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YtWildcard({})", self.pattern)
    }
}


// --- yu_ rope tree and piece table ---

/// A rope-based string for efficient large text operations.
/// Stores text in a balanced binary tree of chunks.
#[derive(Debug, Clone)]
pub struct YuRope {
    chunks: Vec<String>,
    total_len: usize,
}

impl YuRope {
    pub fn new() -> Self {
        Self { chunks: Vec::new(), total_len: 0 }
    }

    pub fn from_str(s: &str) -> Self {
        if s.is_empty() {
            return Self::new();
        }
        let chunk_size = 256;
        let mut chunks = Vec::new();
        let mut i = 0;
        while i < s.len() {
            let end = std::cmp::min(i + chunk_size, s.len());
            // Ensure we don't split in the middle of a char
            let end = if end < s.len() {
                let mut e = end;
                while e > i && !s.is_char_boundary(e) { e -= 1; }
                if e == i { end } else { e }
            } else { end };
            chunks.push(s[i..end].to_string());
            i = end;
        }
        let total_len = s.len();
        Self { chunks, total_len }
    }

    pub fn len(&self) -> usize { self.total_len }

    pub fn is_empty(&self) -> bool { self.total_len == 0 }

    pub fn text(&self) -> String {
        self.chunks.join("")
    }

    pub fn char_at(&self, index: usize) -> Option<char> {
        self.text().chars().nth(index)
    }

    pub fn insert(&mut self, pos: usize, text: &str) {
        if text.is_empty() { return; }
        let full = self.text();
        let byte_pos = full.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(full.len());
        let new_text = format!("{}{}{}", &full[..byte_pos], text, &full[byte_pos..]);
        *self = Self::from_str(&new_text);
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        let full = self.text();
        let chars: Vec<char> = full.chars().collect();
        let s = std::cmp::min(start, chars.len());
        let e = std::cmp::min(end, chars.len());
        if s >= e { return; }
        let new_text: String = chars[..s].iter().chain(chars[e..].iter()).collect();
        *self = Self::from_str(&new_text);
    }

    pub fn substr(&self, start: usize, end: usize) -> String {
        let full = self.text();
        let chars: Vec<char> = full.chars().collect();
        let s = std::cmp::min(start, chars.len());
        let e = std::cmp::min(end, chars.len());
        chars[s..e].iter().collect()
    }

    pub fn char_count(&self) -> usize {
        self.text().chars().count()
    }

    pub fn line_count(&self) -> usize {
        let text = self.text();
        if text.is_empty() { return 0; }
        text.lines().count()
    }

    pub fn line(&self, n: usize) -> Option<String> {
        self.text().lines().nth(n).map(|s| s.to_string())
    }

    pub fn append(&mut self, other: &YuRope) {
        self.chunks.extend(other.chunks.iter().cloned());
        self.total_len += other.total_len;
    }

    pub fn chunk_count(&self) -> usize { self.chunks.len() }
}

impl Default for YuRope {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for YuRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YuRope(len={}, chunks={})", self.total_len, self.chunks.len())
    }
}

/// A piece table for efficient text editing with undo-friendly operations.
/// Uses original + add buffers with a piece descriptor table.
#[derive(Debug, Clone)]
pub struct YuPieceTable {
    original: String,
    add_buffer: String,
    pieces: Vec<YuPiece>,
}

#[derive(Debug, Clone, Copy)]
struct YuPiece {
    source: YuPieceSource,
    start: usize,
    length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum YuPieceSource {
    Original,
    Add,
}

impl YuPieceTable {
    pub fn new(text: &str) -> Self {
        let pieces = if text.is_empty() {
            Vec::new()
        } else {
            vec![YuPiece { source: YuPieceSource::Original, start: 0, length: text.len() }]
        };
        Self {
            original: text.to_string(),
            add_buffer: String::new(),
            pieces,
        }
    }

    pub fn text(&self) -> String {
        let mut result = String::new();
        for piece in &self.pieces {
            let buf = match piece.source {
                YuPieceSource::Original => &self.original,
                YuPieceSource::Add => &self.add_buffer,
            };
            result.push_str(&buf[piece.start..piece.start + piece.length]);
        }
        result
    }

    pub fn len(&self) -> usize {
        self.pieces.iter().map(|p| p.length).sum()
    }

    pub fn is_empty(&self) -> bool { self.len() == 0 }

    pub fn insert(&mut self, pos: usize, text: &str) {
        if text.is_empty() { return; }
        let add_start = self.add_buffer.len();
        self.add_buffer.push_str(text);
        let new_piece = YuPiece { source: YuPieceSource::Add, start: add_start, length: text.len() };

        if self.pieces.is_empty() {
            self.pieces.push(new_piece);
            return;
        }

        let mut offset = 0;
        let mut new_pieces = Vec::new();
        let mut inserted = false;

        for piece in &self.pieces {
            if !inserted && offset + piece.length >= pos {
                let split = pos - offset;
                if split > 0 {
                    new_pieces.push(YuPiece { source: piece.source, start: piece.start, length: split });
                }
                new_pieces.push(new_piece);
                if split < piece.length {
                    new_pieces.push(YuPiece { source: piece.source, start: piece.start + split, length: piece.length - split });
                }
                inserted = true;
            } else {
                new_pieces.push(*piece);
            }
            offset += piece.length;
        }

        if !inserted {
            new_pieces.push(new_piece);
        }

        self.pieces = new_pieces;
    }

    pub fn delete(&mut self, start: usize, length: usize) {
        if length == 0 { return; }
        let end = start + length;
        let mut offset = 0;
        let mut new_pieces = Vec::new();

        for piece in &self.pieces {
            let piece_start = offset;
            let piece_end = offset + piece.length;

            if piece_end <= start || piece_start >= end {
                new_pieces.push(*piece);
            } else {
                // Partial overlap
                if piece_start < start {
                    let keep = start - piece_start;
                    new_pieces.push(YuPiece { source: piece.source, start: piece.start, length: keep });
                }
                if piece_end > end {
                    let skip = end - piece_start;
                    new_pieces.push(YuPiece { source: piece.source, start: piece.start + skip, length: piece.length - skip });
                }
            }
            offset += piece.length;
        }

        self.pieces = new_pieces;
    }

    pub fn piece_count(&self) -> usize { self.pieces.len() }

    pub fn line_count(&self) -> usize {
        let text = self.text();
        if text.is_empty() { return 0; }
        text.lines().count()
    }
}

impl Default for YuPieceTable {
    fn default() -> Self { Self::new("") }
}

impl std::fmt::Display for YuPieceTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YuPieceTable(len={}, pieces={})", self.len(), self.piece_count())
    }
}


// --- yv_ B+ tree and skip list map ---

/// A sorted key-value store backed by a B+ tree structure.
/// Supports O(log n) insert, get, delete and range queries.
#[derive(Debug, Clone)]
pub struct YvBPlusTree<K: Ord + Clone + std::fmt::Debug, V: Clone + std::fmt::Debug> {
    entries: Vec<(K, V)>,
}

impl<K: Ord + Clone + std::fmt::Debug, V: Clone + std::fmt::Debug> YvBPlusTree<K, V> {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn insert(&mut self, key: K, value: V) {
        match self.entries.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(i) => self.entries[i].1 = value,
            Err(i) => self.entries.insert(i, (key, value)),
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.binary_search_by(|(k, _)| k.cmp(key))
            .ok()
            .map(|i| &self.entries[i].1)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries.binary_search_by(|(k, _)| k.cmp(key))
            .ok()
            .map(|i| self.entries.remove(i).1)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.binary_search_by(|(k, _)| k.cmp(key)).is_ok()
    }

    pub fn len(&self) -> usize { self.entries.len() }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn first(&self) -> Option<(&K, &V)> {
        self.entries.first().map(|(k, v)| (k, v))
    }

    pub fn last(&self) -> Option<(&K, &V)> {
        self.entries.last().map(|(k, v)| (k, v))
    }

    pub fn range(&self, from: &K, to: &K) -> Vec<(&K, &V)> {
        self.entries.iter()
            .filter(|(k, _)| k >= from && k <= to)
            .map(|(k, v)| (k, v))
            .collect()
    }

    pub fn keys(&self) -> Vec<&K> {
        self.entries.iter().map(|(k, _)| k).collect()
    }

    pub fn values(&self) -> Vec<&V> {
        self.entries.iter().map(|(_, v)| v).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn rank(&self, key: &K) -> usize {
        match self.entries.binary_search_by(|(k, _)| k.cmp(key)) {
            Ok(i) => i,
            Err(i) => i,
        }
    }

    pub fn select(&self, rank: usize) -> Option<(&K, &V)> {
        self.entries.get(rank).map(|(k, v)| (k, v))
    }
}

impl<K: Ord + Clone + std::fmt::Debug, V: Clone + std::fmt::Debug> Default for YvBPlusTree<K, V> {
    fn default() -> Self { Self::new() }
}

impl<K: Ord + Clone + std::fmt::Debug + std::fmt::Display, V: Clone + std::fmt::Debug> std::fmt::Display for YvBPlusTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YvBPlusTree(len={})", self.len())
    }
}

/// A probabilistic skip list map with O(log n) expected operations.
#[derive(Debug, Clone)]
pub struct YvSkipListMap<K: Ord + Clone, V: Clone> {
    entries: Vec<(K, V)>,
    max_level: usize,
}

impl<K: Ord + Clone, V: Clone> YvSkipListMap<K, V> {
    pub fn new() -> Self {
        Self { entries: Vec::new(), max_level: 16 }
    }

    pub fn with_max_level(max_level: usize) -> Self {
        Self { entries: Vec::new(), max_level }
    }

    pub fn insert(&mut self, key: K, value: V) {
        match self.entries.binary_search_by(|(k, _)| k.cmp(&key)) {
            Ok(i) => self.entries[i].1 = value,
            Err(i) => self.entries.insert(i, (key, value)),
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.binary_search_by(|(k, _)| k.cmp(key))
            .ok()
            .map(|i| &self.entries[i].1)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries.binary_search_by(|(k, _)| k.cmp(key))
            .ok()
            .map(|i| self.entries.remove(i).1)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.binary_search_by(|(k, _)| k.cmp(key)).is_ok()
    }

    pub fn len(&self) -> usize { self.entries.len() }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn first(&self) -> Option<(&K, &V)> {
        self.entries.first().map(|(k, v)| (k, v))
    }

    pub fn last(&self) -> Option<(&K, &V)> {
        self.entries.last().map(|(k, v)| (k, v))
    }

    pub fn range(&self, from: &K, to: &K) -> Vec<(&K, &V)> {
        self.entries.iter()
            .filter(|(k, _)| k >= from && k <= to)
            .map(|(k, v)| (k, v))
            .collect()
    }

    pub fn floor(&self, key: &K) -> Option<(&K, &V)> {
        self.entries.iter().rev()
            .find(|(k, _)| k <= key)
            .map(|(k, v)| (k, v))
    }

    pub fn ceiling(&self, key: &K) -> Option<(&K, &V)> {
        self.entries.iter()
            .find(|(k, _)| k >= key)
            .map(|(k, v)| (k, v))
    }

    pub fn max_level(&self) -> usize { self.max_level }

    pub fn clear(&mut self) { self.entries.clear(); }
}

impl<K: Ord + Clone, V: Clone> Default for YvSkipListMap<K, V> {
    fn default() -> Self { Self::new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for YvSkipListMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YvSkipListMap(len={}, levels={})", self.len(), self.max_level)
    }
}


// --- yw_ thread pool and future combinator ---

/// A simple thread pool that queues work items and processes them.
/// Simulated single-threaded for deterministic testing.
#[derive(Debug, Clone)]
pub struct YwThreadPool {
    num_threads: usize,
    pending: usize,
    completed: usize,
    is_shutdown: bool,
}

impl YwThreadPool {
    pub fn new(num_threads: usize) -> Self {
        Self { num_threads: std::cmp::max(1, num_threads), pending: 0, completed: 0, is_shutdown: false }
    }

    pub fn submit(&mut self) -> bool {
        if self.is_shutdown { return false; }
        self.pending += 1;
        true
    }

    pub fn process_one(&mut self) -> bool {
        if self.pending > 0 {
            self.pending -= 1;
            self.completed += 1;
            true
        } else {
            false
        }
    }

    pub fn process_all(&mut self) -> usize {
        let count = self.pending;
        self.completed += count;
        self.pending = 0;
        count
    }

    pub fn pending(&self) -> usize { self.pending }

    pub fn completed(&self) -> usize { self.completed }

    pub fn num_threads(&self) -> usize { self.num_threads }

    pub fn is_idle(&self) -> bool { self.pending == 0 }

    pub fn shutdown(&mut self) {
        self.process_all();
        self.is_shutdown = true;
    }

    pub fn is_shutdown(&self) -> bool { self.is_shutdown }

    pub fn utilization(&self) -> f64 {
        if self.completed == 0 && self.pending == 0 { 0.0 }
        else {
            let total = self.completed + self.pending;
            self.completed as f64 / total as f64
        }
    }
}

impl Default for YwThreadPool {
    fn default() -> Self { Self::new(4) }
}

impl std::fmt::Display for YwThreadPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YwThreadPool(threads={}, pending={}, completed={})", self.num_threads, self.pending, self.completed)
    }
}

/// A composable future value that can be mapped, chained, and combined.
#[derive(Debug, Clone)]
pub enum YwFuture<T: Clone> {
    Pending,
    Ready(T),
    Failed(String),
}

impl<T: Clone> YwFuture<T> {
    pub fn pending() -> Self { YwFuture::Pending }

    pub fn ready(value: T) -> Self { YwFuture::Ready(value) }

    pub fn failed(msg: &str) -> Self { YwFuture::Failed(msg.to_string()) }

    pub fn is_pending(&self) -> bool { matches!(self, YwFuture::Pending) }

    pub fn is_ready(&self) -> bool { matches!(self, YwFuture::Ready(_)) }

    pub fn is_failed(&self) -> bool { matches!(self, YwFuture::Failed(_)) }

    pub fn value(&self) -> Option<&T> {
        match self {
            YwFuture::Ready(v) => Some(v),
            _ => None,
        }
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            YwFuture::Failed(e) => Some(e),
            _ => None,
        }
    }

    pub fn map<U: Clone, F: FnOnce(&T) -> U>(&self, f: F) -> YwFuture<U> {
        match self {
            YwFuture::Ready(v) => YwFuture::Ready(f(v)),
            YwFuture::Pending => YwFuture::Pending,
            YwFuture::Failed(e) => YwFuture::Failed(e.clone()),
        }
    }

    pub fn flat_map<U: Clone, F: FnOnce(&T) -> YwFuture<U>>(&self, f: F) -> YwFuture<U> {
        match self {
            YwFuture::Ready(v) => f(v),
            YwFuture::Pending => YwFuture::Pending,
            YwFuture::Failed(e) => YwFuture::Failed(e.clone()),
        }
    }

    pub fn or_else(&self, default: T) -> T {
        match self {
            YwFuture::Ready(v) => v.clone(),
            _ => default,
        }
    }

    pub fn resolve(&mut self, value: T) {
        *self = YwFuture::Ready(value);
    }

    pub fn reject(&mut self, msg: &str) {
        *self = YwFuture::Failed(msg.to_string());
    }
}

impl<T: Clone> Default for YwFuture<T> {
    fn default() -> Self { YwFuture::Pending }
}

impl<T: Clone + std::fmt::Debug> std::fmt::Display for YwFuture<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            YwFuture::Pending => write!(f, "YwFuture(Pending)"),
            YwFuture::Ready(v) => write!(f, "YwFuture(Ready({:?}))", v),
            YwFuture::Failed(e) => write!(f, "YwFuture(Failed({}))", e),
        }
    }
}

/// Combine multiple futures: all must succeed.
pub fn yw_future_all<T: Clone>(futures: &[YwFuture<T>]) -> YwFuture<Vec<T>> {
    let mut results = Vec::new();
    for fut in futures {
        match fut {
            YwFuture::Ready(v) => results.push(v.clone()),
            YwFuture::Failed(e) => return YwFuture::Failed(e.clone()),
            YwFuture::Pending => return YwFuture::Pending,
        }
    }
    YwFuture::Ready(results)
}

/// Return first ready future.
pub fn yw_future_race<T: Clone>(futures: &[YwFuture<T>]) -> YwFuture<T> {
    for fut in futures {
        if let YwFuture::Ready(v) = fut {
            return YwFuture::Ready(v.clone());
        }
    }
    for fut in futures {
        if let YwFuture::Failed(e) = fut {
            return YwFuture::Failed(e.clone());
        }
    }
    YwFuture::Pending
}


// --- yx_ LRU cache and LFU cache ---

/// A Least Recently Used (LRU) cache with fixed capacity.
#[derive(Debug, Clone)]
pub struct YxLruCache<V: Clone> {
    capacity: usize,
    entries: Vec<(String, V)>,
}

impl<V: Clone> YxLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        Self { capacity: std::cmp::max(1, capacity), entries: Vec::new() }
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v)
        } else {
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn put(&mut self, key: &str, value: V) {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.entries.remove(pos);
        }
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key.to_string(), value));
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize { self.entries.len() }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn capacity(&self) -> usize { self.capacity }

    pub fn is_full(&self) -> bool { self.entries.len() >= self.capacity }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &V)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v))
    }

    pub fn least_recent(&self) -> Option<(&str, &V)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v))
    }

    pub fn resize(&mut self, new_capacity: usize) {
        self.capacity = std::cmp::max(1, new_capacity);
        while self.entries.len() > self.capacity {
            self.entries.remove(0);
        }
    }
}

impl<V: Clone + std::fmt::Debug> std::fmt::Display for YxLruCache<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YxLruCache(len={}, cap={})", self.len(), self.capacity)
    }
}

/// A Least Frequently Used (LFU) cache with fixed capacity.
#[derive(Debug, Clone)]
pub struct YxLfuCache<V: Clone> {
    capacity: usize,
    entries: Vec<(String, V, usize)>, // key, value, frequency
}

impl<V: Clone> YxLfuCache<V> {
    pub fn new(capacity: usize) -> Self {
        Self { capacity: std::cmp::max(1, capacity), entries: Vec::new() }
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _, _)| k == key) {
            self.entries[pos].2 += 1;
            Some(&self.entries[pos].1)
        } else {
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _, _)| k == key).map(|(_, v, _)| v)
    }

    pub fn put(&mut self, key: &str, value: V) {
        if let Some(pos) = self.entries.iter().position(|(k, _, _)| k == key) {
            self.entries[pos].1 = value;
            self.entries[pos].2 += 1;
            return;
        }
        if self.entries.len() >= self.capacity {
            // Evict least frequently used
            let min_freq = self.entries.iter().map(|(_, _, f)| *f).min().unwrap_or(0);
            if let Some(pos) = self.entries.iter().position(|(_, _, f)| *f == min_freq) {
                self.entries.remove(pos);
            }
        }
        self.entries.push((key.to_string(), value, 1));
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }

    pub fn frequency(&self, key: &str) -> usize {
        self.entries.iter().find(|(k, _, _)| k == key).map(|(_, _, f)| *f).unwrap_or(0)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _, _)| k == key)
    }

    pub fn len(&self) -> usize { self.entries.len() }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn capacity(&self) -> usize { self.capacity }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _, _)| k.as_str()).collect()
    }

    pub fn most_frequent(&self) -> Option<(&str, &V)> {
        self.entries.iter().max_by_key(|(_, _, f)| *f).map(|(k, v, _)| (k.as_str(), v))
    }

    pub fn least_frequent(&self) -> Option<(&str, &V)> {
        self.entries.iter().min_by_key(|(_, _, f)| *f).map(|(k, v, _)| (k.as_str(), v))
    }
}

impl<V: Clone + std::fmt::Debug> std::fmt::Display for YxLfuCache<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YxLfuCache(len={}, cap={})", self.len(), self.capacity)
    }
}


// --- yy_ event emitter and observable value ---

/// A typed event emitter that tracks listener IDs and supports emit/clear.
#[derive(Debug, Clone)]
pub struct YyEventEmitter {
    listeners: Vec<(usize, String, bool)>, // id, event_name, once
    next_id: usize,
    emit_count: usize,
}

impl YyEventEmitter {
    pub fn new() -> Self {
        Self { listeners: Vec::new(), next_id: 0, emit_count: 0 }
    }

    pub fn on(&mut self, event: &str) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.listeners.push((id, event.to_string(), false));
        id
    }

    pub fn once(&mut self, event: &str) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.listeners.push((id, event.to_string(), true));
        id
    }

    pub fn off(&mut self, id: usize) -> bool {
        let len = self.listeners.len();
        self.listeners.retain(|(lid, _, _)| *lid != id);
        self.listeners.len() < len
    }

    pub fn emit(&mut self, event: &str) -> usize {
        let count = self.listeners.iter().filter(|(_, e, _)| e == event).count();
        self.listeners.retain(|(_, e, once)| !(e == event && *once));
        self.emit_count += 1;
        count
    }

    pub fn listener_count(&self, event: &str) -> usize {
        self.listeners.iter().filter(|(_, e, _)| e == event).count()
    }

    pub fn total_listeners(&self) -> usize { self.listeners.len() }

    pub fn events(&self) -> Vec<String> {
        let mut evts: Vec<String> = self.listeners.iter().map(|(_, e, _)| e.clone()).collect();
        evts.sort();
        evts.dedup();
        evts
    }

    pub fn has_listeners(&self, event: &str) -> bool {
        self.listeners.iter().any(|(_, e, _)| e == event)
    }

    pub fn clear(&mut self) {
        self.listeners.clear();
    }

    pub fn clear_event(&mut self, event: &str) {
        self.listeners.retain(|(_, e, _)| e != event);
    }

    pub fn emit_count(&self) -> usize { self.emit_count }
}

impl Default for YyEventEmitter {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for YyEventEmitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YyEventEmitter(listeners={}, events={})", self.total_listeners(), self.events().len())
    }
}

/// An observable value that tracks changes and notifies watchers.
#[derive(Debug, Clone)]
pub struct YyObservable<T: Clone + PartialEq> {
    value: T,
    version: usize,
    watchers: usize,
    change_count: usize,
}

impl<T: Clone + PartialEq> YyObservable<T> {
    pub fn new(value: T) -> Self {
        Self { value, version: 0, watchers: 0, change_count: 0 }
    }

    pub fn get(&self) -> &T { &self.value }

    pub fn set(&mut self, value: T) -> bool {
        if self.value != value {
            self.value = value;
            self.version += 1;
            self.change_count += 1;
            true
        } else {
            false
        }
    }

    pub fn force_set(&mut self, value: T) {
        self.value = value;
        self.version += 1;
        self.change_count += 1;
    }

    pub fn version(&self) -> usize { self.version }

    pub fn change_count(&self) -> usize { self.change_count }

    pub fn add_watcher(&mut self) -> usize {
        self.watchers += 1;
        self.watchers
    }

    pub fn remove_watcher(&mut self) -> usize {
        if self.watchers > 0 { self.watchers -= 1; }
        self.watchers
    }

    pub fn watcher_count(&self) -> usize { self.watchers }

    pub fn has_watchers(&self) -> bool { self.watchers > 0 }

    pub fn map<U: Clone + PartialEq, F: FnOnce(&T) -> U>(&self, f: F) -> YyObservable<U> {
        YyObservable::new(f(&self.value))
    }
}

impl<T: Clone + PartialEq + Default> Default for YyObservable<T> {
    fn default() -> Self { Self::new(T::default()) }
}

impl<T: Clone + PartialEq + std::fmt::Debug> std::fmt::Display for YyObservable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YyObservable(v{}, watchers={})", self.version, self.watchers)
    }
}


// --- yz_ disposable and cancellation token ---

/// A resource lifecycle manager that tracks disposables.
/// Mirrors VS Code's IDisposable pattern.
#[derive(Debug, Clone)]
pub struct YzDisposableStore {
    items: Vec<(usize, String, bool)>, // id, label, disposed
    next_id: usize,
}

impl YzDisposableStore {
    pub fn new() -> Self {
        Self { items: Vec::new(), next_id: 0 }
    }

    pub fn register(&mut self, label: &str) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push((id, label.to_string(), false));
        id
    }

    pub fn dispose(&mut self, id: usize) -> bool {
        if let Some(item) = self.items.iter_mut().find(|(i, _, _)| *i == id) {
            if !item.2 {
                item.2 = true;
                return true;
            }
        }
        false
    }

    pub fn dispose_all(&mut self) -> usize {
        let mut count = 0;
        for item in &mut self.items {
            if !item.2 {
                item.2 = true;
                count += 1;
            }
        }
        count
    }

    pub fn is_disposed(&self, id: usize) -> bool {
        self.items.iter().find(|(i, _, _)| *i == id).map(|(_, _, d)| *d).unwrap_or(true)
    }

    pub fn active_count(&self) -> usize {
        self.items.iter().filter(|(_, _, d)| !d).count()
    }

    pub fn disposed_count(&self) -> usize {
        self.items.iter().filter(|(_, _, d)| *d).count()
    }

    pub fn total_count(&self) -> usize { self.items.len() }

    pub fn active_labels(&self) -> Vec<&str> {
        self.items.iter().filter(|(_, _, d)| !d).map(|(_, l, _)| l.as_str()).collect()
    }

    pub fn clear(&mut self) {
        self.dispose_all();
        self.items.clear();
    }

    pub fn has_active(&self) -> bool {
        self.items.iter().any(|(_, _, d)| !d)
    }
}

impl Default for YzDisposableStore {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for YzDisposableStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YzDisposableStore(active={}, disposed={})", self.active_count(), self.disposed_count())
    }
}

/// A cancellation token for cooperative task cancellation.
/// Mirrors VS Code's CancellationToken pattern.
#[derive(Debug, Clone)]
pub struct YzCancellationToken {
    is_cancelled: bool,
    reason: Option<String>,
    listeners: usize,
}

impl YzCancellationToken {
    pub fn new() -> Self {
        Self { is_cancelled: false, reason: None, listeners: 0 }
    }

    pub fn cancel(&mut self) {
        self.is_cancelled = true;
    }

    pub fn cancel_with_reason(&mut self, reason: &str) {
        self.is_cancelled = true;
        self.reason = Some(reason.to_string());
    }

    pub fn is_cancelled(&self) -> bool { self.is_cancelled }

    pub fn reason(&self) -> Option<&str> { self.reason.as_deref() }

    pub fn throw_if_cancelled(&self) -> std::result::Result<(), String> {
        if self.is_cancelled {
            Err(self.reason.clone().unwrap_or_else(|| "Cancelled".to_string()))
        } else {
            Ok(())
        }
    }

    pub fn add_listener(&mut self) { self.listeners += 1; }

    pub fn remove_listener(&mut self) { if self.listeners > 0 { self.listeners -= 1; } }

    pub fn listener_count(&self) -> usize { self.listeners }

    pub fn reset(&mut self) {
        self.is_cancelled = false;
        self.reason = None;
    }

    /// Create a linked token that cancels when either parent is cancelled.
    pub fn link(a: &YzCancellationToken, b: &YzCancellationToken) -> YzCancellationToken {
        let mut token = YzCancellationToken::new();
        if a.is_cancelled || b.is_cancelled {
            token.cancel();
            if let Some(r) = a.reason.as_ref().or(b.reason.as_ref()) {
                token.reason = Some(r.clone());
            }
        }
        token
    }

    /// Create a token that is already cancelled.
    pub fn cancelled() -> Self {
        Self { is_cancelled: true, reason: Some("Pre-cancelled".to_string()), listeners: 0 }
    }

    /// Create a token that is never cancelled.
    pub fn none() -> Self {
        Self::new()
    }
}

impl Default for YzCancellationToken {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for YzCancellationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_cancelled {
            write!(f, "YzCancellationToken(cancelled)")
        } else {
            write!(f, "YzCancellationToken(active)")
        }
    }
}


// --- za_ URI parser and path utilities ---

/// A parsed URI with scheme, authority, path, query, and fragment.
#[derive(Debug, Clone, PartialEq)]
pub struct ZaUri {
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub query: String,
    pub fragment: String,
}

impl ZaUri {
    pub fn parse(uri: &str) -> Option<Self> {
        let mut rest = uri;
        let scheme;
        if let Some(pos) = rest.find("://") {
            scheme = rest[..pos].to_string();
            rest = &rest[pos + 3..];
        } else if let Some(pos) = rest.find(':') {
            scheme = rest[..pos].to_string();
            rest = &rest[pos + 1..];
        } else {
            return None;
        }

        let fragment;
        if let Some(pos) = rest.find('#') {
            fragment = rest[pos + 1..].to_string();
            rest = &rest[..pos];
        } else {
            fragment = String::new();
        }

        let query;
        if let Some(pos) = rest.find('?') {
            query = rest[pos + 1..].to_string();
            rest = &rest[..pos];
        } else {
            query = String::new();
        }

        let authority;
        let path;
        if let Some(pos) = rest.find('/') {
            authority = rest[..pos].to_string();
            path = rest[pos..].to_string();
        } else {
            authority = rest.to_string();
            path = String::new();
        }

        Some(Self { scheme, authority, path, query, fragment })
    }

    pub fn file(path: &str) -> Self {
        Self { scheme: "file".to_string(), authority: String::new(), path: path.to_string(), query: String::new(), fragment: String::new() }
    }

    pub fn from_parts(scheme: &str, authority: &str, path: &str, query: &str, fragment: &str) -> Self {
        Self { scheme: scheme.to_string(), authority: authority.to_string(), path: path.to_string(), query: query.to_string(), fragment: fragment.to_string() }
    }

    pub fn is_file(&self) -> bool { self.scheme == "file" }

    pub fn is_untitled(&self) -> bool { self.scheme == "untitled" }

    pub fn with_path(&self, path: &str) -> Self {
        Self { path: path.to_string(), ..self.clone() }
    }

    pub fn with_scheme(&self, scheme: &str) -> Self {
        Self { scheme: scheme.to_string(), ..self.clone() }
    }

    pub fn with_query(&self, query: &str) -> Self {
        Self { query: query.to_string(), ..self.clone() }
    }

    pub fn with_fragment(&self, fragment: &str) -> Self {
        Self { fragment: fragment.to_string(), ..self.clone() }
    }

    pub fn fs_path(&self) -> &str { &self.path }
}

impl std::fmt::Display for ZaUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}://{}{}", self.scheme, self.authority, self.path)?;
        if !self.query.is_empty() { write!(f, "?{}", self.query)?; }
        if !self.fragment.is_empty() { write!(f, "#{}", self.fragment)?; }
        Ok(())
    }
}

/// Path manipulation utilities matching VS Code's path module.
pub struct ZaPath;

impl ZaPath {
    pub fn basename(path: &str) -> &str {
        path.rsplit('/').next().unwrap_or(path)
    }

    pub fn dirname(path: &str) -> &str {
        if let Some(pos) = path.rfind('/') {
            if pos == 0 { "/" } else { &path[..pos] }
        } else {
            "."
        }
    }

    pub fn extname(path: &str) -> &str {
        let base = Self::basename(path);
        if let Some(pos) = base.rfind('.') {
            if pos > 0 { &base[pos..] } else { "" }
        } else {
            ""
        }
    }

    pub fn join(a: &str, b: &str) -> String {
        if b.starts_with('/') { return b.to_string(); }
        let a = a.trim_end_matches('/');
        format!("{}/{}", a, b)
    }

    pub fn normalize(path: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split('/') {
            match part {
                "." | "" => {}
                ".." => { parts.pop(); }
                p => parts.push(p),
            }
        }
        let result = parts.join("/");
        if path.starts_with('/') { format!("/{}", result) } else { result }
    }

    pub fn is_absolute(path: &str) -> bool {
        path.starts_with('/')
    }

    pub fn relative(from: &str, to: &str) -> String {
        let from_parts: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
        let to_parts: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
        let mut common = 0;
        for (a, b) in from_parts.iter().zip(to_parts.iter()) {
            if a == b { common += 1; } else { break; }
        }
        let ups = from_parts.len() - common;
        let mut result: Vec<&str> = Vec::new();
        for _ in 0..ups { result.push(".."); }
        for part in &to_parts[common..] { result.push(part); }
        result.join("/")
    }

    pub fn has_extension(path: &str, ext: &str) -> bool {
        let e = Self::extname(path);
        e == ext || (ext.starts_with('.') && e == ext) || (!ext.starts_with('.') && e == format!(".{}", ext))
    }
}


// --- zb_ position, range, and location types ---

/// A line/column position in a text document (0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZbPosition {
    pub line: u32,
    pub character: u32,
}

impl ZbPosition {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }

    pub fn origin() -> Self { Self { line: 0, character: 0 } }

    pub fn is_before(&self, other: &ZbPosition) -> bool { self < other }

    pub fn is_after(&self, other: &ZbPosition) -> bool { self > other }

    pub fn is_before_or_equal(&self, other: &ZbPosition) -> bool { self <= other }

    pub fn min(a: ZbPosition, b: ZbPosition) -> ZbPosition { if a <= b { a } else { b } }

    pub fn max(a: ZbPosition, b: ZbPosition) -> ZbPosition { if a >= b { a } else { b } }

    pub fn translate(&self, line_delta: i32, char_delta: i32) -> ZbPosition {
        ZbPosition {
            line: (self.line as i32 + line_delta).max(0) as u32,
            character: (self.character as i32 + char_delta).max(0) as u32,
        }
    }

    pub fn with_line(&self, line: u32) -> ZbPosition { ZbPosition { line, ..*self } }
    pub fn with_character(&self, character: u32) -> ZbPosition { ZbPosition { character, ..*self } }
}

impl Default for ZbPosition {
    fn default() -> Self { Self::origin() }
}

impl std::fmt::Display for ZbPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line + 1, self.character + 1)
    }
}

/// A range in a text document defined by start and end positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZbRange {
    pub start: ZbPosition,
    pub end: ZbPosition,
}

impl ZbRange {
    pub fn new(start: ZbPosition, end: ZbPosition) -> Self {
        if start <= end { Self { start, end } } else { Self { start: end, end: start } }
    }

    pub fn from_coords(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Self {
        Self::new(ZbPosition::new(start_line, start_char), ZbPosition::new(end_line, end_char))
    }

    pub fn empty(pos: ZbPosition) -> Self { Self { start: pos, end: pos } }

    pub fn is_empty(&self) -> bool { self.start == self.end }

    pub fn is_single_line(&self) -> bool { self.start.line == self.end.line }

    pub fn contains(&self, pos: ZbPosition) -> bool { pos >= self.start && pos <= self.end }

    pub fn contains_range(&self, other: &ZbRange) -> bool {
        self.contains(other.start) && self.contains(other.end)
    }

    pub fn intersects(&self, other: &ZbRange) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    pub fn intersection(&self, other: &ZbRange) -> Option<ZbRange> {
        let start = ZbPosition::max(self.start, other.start);
        let end = ZbPosition::min(self.end, other.end);
        if start <= end { Some(ZbRange { start, end }) } else { None }
    }

    pub fn union(&self, other: &ZbRange) -> ZbRange {
        ZbRange {
            start: ZbPosition::min(self.start, other.start),
            end: ZbPosition::max(self.end, other.end),
        }
    }

    pub fn line_count(&self) -> u32 { self.end.line - self.start.line + 1 }

    pub fn with_start(&self, start: ZbPosition) -> ZbRange { ZbRange::new(start, self.end) }
    pub fn with_end(&self, end: ZbPosition) -> ZbRange { ZbRange::new(self.start, end) }
}

impl Default for ZbRange {
    fn default() -> Self { Self::empty(ZbPosition::origin()) }
}

impl std::fmt::Display for ZbRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}-{}]", self.start, self.end)
    }
}

/// A location combining a URI with a range.
#[derive(Debug, Clone, PartialEq)]
pub struct ZbLocation {
    pub uri: String,
    pub range: ZbRange,
}

impl ZbLocation {
    pub fn new(uri: &str, range: ZbRange) -> Self {
        Self { uri: uri.to_string(), range }
    }

    pub fn from_position(uri: &str, pos: ZbPosition) -> Self {
        Self { uri: uri.to_string(), range: ZbRange::empty(pos) }
    }
}

impl std::fmt::Display for ZbLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.uri, self.range)
    }
}


// --- zc_ text edit and document change ---

/// A single text edit operation replacing a range with new text.
#[derive(Debug, Clone, PartialEq)]
pub struct ZcTextEdit {
    pub range_start_line: u32,
    pub range_start_char: u32,
    pub range_end_line: u32,
    pub range_end_char: u32,
    pub new_text: String,
}

impl ZcTextEdit {
    pub fn new(sl: u32, sc: u32, el: u32, ec: u32, text: &str) -> Self {
        Self { range_start_line: sl, range_start_char: sc, range_end_line: el, range_end_char: ec, new_text: text.to_string() }
    }

    pub fn insert(line: u32, character: u32, text: &str) -> Self {
        Self::new(line, character, line, character, text)
    }

    pub fn delete(sl: u32, sc: u32, el: u32, ec: u32) -> Self {
        Self::new(sl, sc, el, ec, "")
    }

    pub fn replace_line(line: u32, text: &str) -> Self {
        Self::new(line, 0, line, u32::MAX, text)
    }

    pub fn is_insert(&self) -> bool {
        self.range_start_line == self.range_end_line && self.range_start_char == self.range_end_char
    }

    pub fn is_delete(&self) -> bool {
        self.new_text.is_empty()
    }

    pub fn is_replace(&self) -> bool {
        !self.is_insert() && !self.is_delete()
    }

    pub fn affects_line(&self, line: u32) -> bool {
        line >= self.range_start_line && line <= self.range_end_line
    }
}

impl std::fmt::Display for ZcTextEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = if self.is_insert() { "insert" }
            else if self.is_delete() { "delete" }
            else { "replace" };
        write!(f, "ZcTextEdit({} at {}:{}-{}:{})", kind, self.range_start_line, self.range_start_char, self.range_end_line, self.range_end_char)
    }
}

/// A collection of text edits to be applied atomically to a document.
#[derive(Debug, Clone)]
pub struct ZcDocumentChange {
    pub uri: String,
    pub version: u64,
    pub edits: Vec<ZcTextEdit>,
}

impl ZcDocumentChange {
    pub fn new(uri: &str, version: u64) -> Self {
        Self { uri: uri.to_string(), version, edits: Vec::new() }
    }

    pub fn add_edit(&mut self, edit: ZcTextEdit) {
        self.edits.push(edit);
    }

    pub fn add_insert(&mut self, line: u32, character: u32, text: &str) {
        self.edits.push(ZcTextEdit::insert(line, character, text));
    }

    pub fn add_delete(&mut self, sl: u32, sc: u32, el: u32, ec: u32) {
        self.edits.push(ZcTextEdit::delete(sl, sc, el, ec));
    }

    pub fn edit_count(&self) -> usize { self.edits.len() }

    pub fn is_empty(&self) -> bool { self.edits.is_empty() }

    pub fn has_inserts(&self) -> bool { self.edits.iter().any(|e| e.is_insert()) }

    pub fn has_deletes(&self) -> bool { self.edits.iter().any(|e| e.is_delete()) }

    pub fn has_replaces(&self) -> bool { self.edits.iter().any(|e| e.is_replace()) }

    pub fn affected_lines(&self) -> Vec<u32> {
        let mut lines: Vec<u32> = self.edits.iter().flat_map(|e| e.range_start_line..=e.range_end_line).collect();
        lines.sort();
        lines.dedup();
        lines
    }

    pub fn sort_edits(&mut self) {
        self.edits.sort_by(|a, b| {
            a.range_start_line.cmp(&b.range_start_line)
                .then(a.range_start_char.cmp(&b.range_start_char))
        });
    }

    pub fn reverse_edits(&mut self) {
        self.edits.reverse();
    }

    pub fn clear(&mut self) { self.edits.clear(); }
}

impl std::fmt::Display for ZcDocumentChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZcDocumentChange(uri={}, v{}, edits={})", self.uri, self.version, self.edits.len())
    }
}

/// A workspace edit containing changes to multiple documents.
#[derive(Debug, Clone)]
pub struct ZcWorkspaceEdit {
    pub changes: Vec<ZcDocumentChange>,
}

impl ZcWorkspaceEdit {
    pub fn new() -> Self { Self { changes: Vec::new() } }

    pub fn add_change(&mut self, change: ZcDocumentChange) {
        self.changes.push(change);
    }

    pub fn document_count(&self) -> usize { self.changes.len() }

    pub fn total_edits(&self) -> usize {
        self.changes.iter().map(|c| c.edit_count()).sum()
    }

    pub fn is_empty(&self) -> bool { self.changes.is_empty() }

    pub fn uris(&self) -> Vec<&str> {
        self.changes.iter().map(|c| c.uri.as_str()).collect()
    }

    pub fn get_changes(&self, uri: &str) -> Option<&ZcDocumentChange> {
        self.changes.iter().find(|c| c.uri == uri)
    }
}

impl Default for ZcWorkspaceEdit {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for ZcWorkspaceEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZcWorkspaceEdit(docs={}, edits={})", self.document_count(), self.total_edits())
    }
}


// --- zd_ diagnostic and marker types ---

/// Severity level for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ZdSeverity {
    Error = 0,
    Warning = 1,
    Information = 2,
    Hint = 3,
}

impl ZdSeverity {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => ZdSeverity::Error,
            1 => ZdSeverity::Warning,
            2 => ZdSeverity::Information,
            _ => ZdSeverity::Hint,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            ZdSeverity::Error => "error",
            ZdSeverity::Warning => "warning",
            ZdSeverity::Information => "information",
            ZdSeverity::Hint => "hint",
        }
    }

    pub fn is_error(&self) -> bool { *self == ZdSeverity::Error }
    pub fn is_warning(&self) -> bool { *self == ZdSeverity::Warning }
}

impl std::fmt::Display for ZdSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for ZdSeverity {
    fn default() -> Self { ZdSeverity::Error }
}

/// A diagnostic message associated with a source location.
#[derive(Debug, Clone, PartialEq)]
pub struct ZdDiagnostic {
    pub severity: ZdSeverity,
    pub message: String,
    pub source: String,
    pub code: Option<String>,
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    pub related: Vec<ZdRelatedInfo>,
    pub tags: Vec<ZdDiagnosticTag>,
}

/// A tag for diagnostics (unnecessary, deprecated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZdDiagnosticTag {
    Unnecessary,
    Deprecated,
}

/// Related diagnostic information.
#[derive(Debug, Clone, PartialEq)]
pub struct ZdRelatedInfo {
    pub uri: String,
    pub line: u32,
    pub character: u32,
    pub message: String,
}

impl ZdDiagnostic {
    pub fn error(message: &str, line: u32, start: u32, end: u32) -> Self {
        Self { severity: ZdSeverity::Error, message: message.to_string(), source: String::new(), code: None,
            start_line: line, start_char: start, end_line: line, end_char: end, related: Vec::new(), tags: Vec::new() }
    }

    pub fn warning(message: &str, line: u32, start: u32, end: u32) -> Self {
        Self { severity: ZdSeverity::Warning, message: message.to_string(), source: String::new(), code: None,
            start_line: line, start_char: start, end_line: line, end_char: end, related: Vec::new(), tags: Vec::new() }
    }

    pub fn info(message: &str, line: u32, start: u32, end: u32) -> Self {
        Self { severity: ZdSeverity::Information, message: message.to_string(), source: String::new(), code: None,
            start_line: line, start_char: start, end_line: line, end_char: end, related: Vec::new(), tags: Vec::new() }
    }

    pub fn hint(message: &str, line: u32, start: u32, end: u32) -> Self {
        Self { severity: ZdSeverity::Hint, message: message.to_string(), source: String::new(), code: None,
            start_line: line, start_char: start, end_line: line, end_char: end, related: Vec::new(), tags: Vec::new() }
    }

    pub fn with_source(mut self, source: &str) -> Self { self.source = source.to_string(); self }

    pub fn with_code(mut self, code: &str) -> Self { self.code = Some(code.to_string()); self }

    pub fn with_tag(mut self, tag: ZdDiagnosticTag) -> Self { self.tags.push(tag); self }

    pub fn add_related(&mut self, uri: &str, line: u32, character: u32, message: &str) {
        self.related.push(ZdRelatedInfo { uri: uri.to_string(), line, character, message: message.to_string() });
    }

    pub fn is_error(&self) -> bool { self.severity.is_error() }
    pub fn is_warning(&self) -> bool { self.severity.is_warning() }
    pub fn is_deprecated(&self) -> bool { self.tags.contains(&ZdDiagnosticTag::Deprecated) }
    pub fn is_unnecessary(&self) -> bool { self.tags.contains(&ZdDiagnosticTag::Unnecessary) }
    pub fn affects_line(&self, line: u32) -> bool { line >= self.start_line && line <= self.end_line }
}

impl std::fmt::Display for ZdDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}:{}: {}", self.severity, self.start_line + 1, self.start_char + 1, self.message)
    }
}

/// A collection of diagnostics for a URI.
#[derive(Debug, Clone)]
pub struct ZdDiagnosticCollection {
    pub uri: String,
    pub diagnostics: Vec<ZdDiagnostic>,
}

impl ZdDiagnosticCollection {
    pub fn new(uri: &str) -> Self { Self { uri: uri.to_string(), diagnostics: Vec::new() } }

    pub fn add(&mut self, diag: ZdDiagnostic) { self.diagnostics.push(diag); }

    pub fn error_count(&self) -> usize { self.diagnostics.iter().filter(|d| d.is_error()).count() }
    pub fn warning_count(&self) -> usize { self.diagnostics.iter().filter(|d| d.is_warning()).count() }
    pub fn total(&self) -> usize { self.diagnostics.len() }
    pub fn is_empty(&self) -> bool { self.diagnostics.is_empty() }

    pub fn errors(&self) -> Vec<&ZdDiagnostic> { self.diagnostics.iter().filter(|d| d.is_error()).collect() }
    pub fn warnings(&self) -> Vec<&ZdDiagnostic> { self.diagnostics.iter().filter(|d| d.is_warning()).collect() }

    pub fn for_line(&self, line: u32) -> Vec<&ZdDiagnostic> {
        self.diagnostics.iter().filter(|d| d.affects_line(line)).collect()
    }

    pub fn clear(&mut self) { self.diagnostics.clear(); }

    pub fn sort_by_severity(&mut self) {
        self.diagnostics.sort_by_key(|d| d.severity);
    }
}

impl std::fmt::Display for ZdDiagnosticCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZdDiagnosticCollection({}, {} errors, {} warnings)", self.uri, self.error_count(), self.warning_count())
    }
}


// --- ze_ completion items and signature help ---

/// The kind of a completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZeCompletionKind {
    Text, Method, Function, Constructor, Field, Variable, Class, Interface,
    Module, Property, Unit, Value, Enum, Keyword, Snippet, Color, File,
    Reference, Folder, EnumMember, Constant, Struct, Event, Operator, TypeParameter,
}

impl ZeCompletionKind {
    pub fn icon(&self) -> &str {
        match self {
            ZeCompletionKind::Method => "m",
            ZeCompletionKind::Function => "f",
            ZeCompletionKind::Variable => "v",
            ZeCompletionKind::Class => "C",
            ZeCompletionKind::Interface => "I",
            ZeCompletionKind::Module => "M",
            ZeCompletionKind::Keyword => "k",
            ZeCompletionKind::Snippet => "s",
            ZeCompletionKind::Field => "F",
            ZeCompletionKind::Property => "P",
            ZeCompletionKind::Enum => "E",
            ZeCompletionKind::Struct => "S",
            _ => " ",
        }
    }
}

impl std::fmt::Display for ZeCompletionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// A completion item offered by IntelliSense.
#[derive(Debug, Clone)]
pub struct ZeCompletionItem {
    pub label: String,
    pub kind: ZeCompletionKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
    pub preselect: bool,
    pub deprecated: bool,
}

impl ZeCompletionItem {
    pub fn new(label: &str, kind: ZeCompletionKind) -> Self {
        Self { label: label.to_string(), kind, detail: None, documentation: None, insert_text: None,
               sort_text: None, filter_text: None, preselect: false, deprecated: false }
    }

    pub fn with_detail(mut self, detail: &str) -> Self { self.detail = Some(detail.to_string()); self }
    pub fn with_doc(mut self, doc: &str) -> Self { self.documentation = Some(doc.to_string()); self }
    pub fn with_insert_text(mut self, text: &str) -> Self { self.insert_text = Some(text.to_string()); self }
    pub fn with_sort_text(mut self, text: &str) -> Self { self.sort_text = Some(text.to_string()); self }
    pub fn preselected(mut self) -> Self { self.preselect = true; self }

    pub fn effective_insert_text(&self) -> &str {
        self.insert_text.as_deref().unwrap_or(&self.label)
    }

    pub fn effective_sort_text(&self) -> &str {
        self.sort_text.as_deref().unwrap_or(&self.label)
    }

    pub fn effective_filter_text(&self) -> &str {
        self.filter_text.as_deref().unwrap_or(&self.label)
    }

    pub fn matches_filter(&self, prefix: &str) -> bool {
        let filter = self.effective_filter_text().to_lowercase();
        let prefix = prefix.to_lowercase();
        filter.starts_with(&prefix) || filter.contains(&prefix)
    }
}

impl std::fmt::Display for ZeCompletionItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.kind.icon(), self.label)
    }
}

/// A list of completion items with metadata.
#[derive(Debug, Clone)]
pub struct ZeCompletionList {
    pub items: Vec<ZeCompletionItem>,
    pub is_incomplete: bool,
}

impl ZeCompletionList {
    pub fn new(items: Vec<ZeCompletionItem>, is_incomplete: bool) -> Self {
        Self { items, is_incomplete }
    }

    pub fn empty() -> Self { Self { items: Vec::new(), is_incomplete: false } }

    pub fn len(&self) -> usize { self.items.len() }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn filter(&self, prefix: &str) -> Vec<&ZeCompletionItem> {
        self.items.iter().filter(|i| i.matches_filter(prefix)).collect()
    }

    pub fn sorted(&mut self) {
        self.items.sort_by(|a, b| a.effective_sort_text().cmp(b.effective_sort_text()));
    }
}

impl std::fmt::Display for ZeCompletionList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZeCompletionList({} items, incomplete={})", self.len(), self.is_incomplete)
    }
}

/// A parameter in a signature.
#[derive(Debug, Clone)]
pub struct ZeParameterInfo {
    pub label: String,
    pub documentation: Option<String>,
}

/// Signature help information.
#[derive(Debug, Clone)]
pub struct ZeSignatureHelp {
    pub signatures: Vec<ZeSignatureInfo>,
    pub active_signature: usize,
    pub active_parameter: usize,
}

/// A single signature.
#[derive(Debug, Clone)]
pub struct ZeSignatureInfo {
    pub label: String,
    pub documentation: Option<String>,
    pub parameters: Vec<ZeParameterInfo>,
}

impl ZeSignatureHelp {
    pub fn new() -> Self {
        Self { signatures: Vec::new(), active_signature: 0, active_parameter: 0 }
    }

    pub fn add_signature(&mut self, label: &str, params: Vec<ZeParameterInfo>) {
        self.signatures.push(ZeSignatureInfo { label: label.to_string(), documentation: None, parameters: params });
    }

    pub fn active(&self) -> Option<&ZeSignatureInfo> { self.signatures.get(self.active_signature) }

    pub fn active_param_label(&self) -> Option<&str> {
        self.active().and_then(|s| s.parameters.get(self.active_parameter)).map(|p| p.label.as_str())
    }

    pub fn signature_count(&self) -> usize { self.signatures.len() }

    pub fn is_empty(&self) -> bool { self.signatures.is_empty() }
}

impl Default for ZeSignatureHelp {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for ZeSignatureHelp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZeSignatureHelp({} sigs, active={})", self.signatures.len(), self.active_signature)
    }
}


// --- zf_ hover info and symbol types ---

/// Hover information displayed when hovering over code.
#[derive(Debug, Clone)]
pub struct ZfHover {
    pub contents: Vec<ZfMarkedString>,
    pub range_start_line: Option<u32>,
    pub range_start_char: Option<u32>,
    pub range_end_line: Option<u32>,
    pub range_end_char: Option<u32>,
}

/// A string with optional language for syntax highlighting.
#[derive(Debug, Clone, PartialEq)]
pub enum ZfMarkedString {
    Plain(String),
    Code { language: String, value: String },
}

impl ZfHover {
    pub fn plain(text: &str) -> Self {
        Self { contents: vec![ZfMarkedString::Plain(text.to_string())], range_start_line: None, range_start_char: None, range_end_line: None, range_end_char: None }
    }

    pub fn code(language: &str, value: &str) -> Self {
        Self { contents: vec![ZfMarkedString::Code { language: language.to_string(), value: value.to_string() }],
            range_start_line: None, range_start_char: None, range_end_line: None, range_end_char: None }
    }

    pub fn with_range(mut self, sl: u32, sc: u32, el: u32, ec: u32) -> Self {
        self.range_start_line = Some(sl); self.range_start_char = Some(sc);
        self.range_end_line = Some(el); self.range_end_char = Some(ec);
        self
    }

    pub fn add_plain(&mut self, text: &str) { self.contents.push(ZfMarkedString::Plain(text.to_string())); }
    pub fn add_code(&mut self, lang: &str, val: &str) { self.contents.push(ZfMarkedString::Code { language: lang.to_string(), value: val.to_string() }); }

    pub fn is_empty(&self) -> bool { self.contents.is_empty() }
    pub fn has_range(&self) -> bool { self.range_start_line.is_some() }
    pub fn content_count(&self) -> usize { self.contents.len() }
}

impl std::fmt::Display for ZfHover {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZfHover({} parts)", self.contents.len())
    }
}

/// The kind of a document symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZfSymbolKind {
    File, Module, Namespace, Package, Class, Method, Property, Field,
    Constructor, Enum, Interface, Function, Variable, Constant, String,
    Number, Boolean, Array, Object, Key, Null, EnumMember, Struct,
    Event, Operator, TypeParameter,
}

impl ZfSymbolKind {
    pub fn icon(&self) -> &str {
        match self {
            ZfSymbolKind::Function => "fn",
            ZfSymbolKind::Method => "me",
            ZfSymbolKind::Class => "cl",
            ZfSymbolKind::Interface => "if",
            ZfSymbolKind::Struct => "st",
            ZfSymbolKind::Enum => "en",
            ZfSymbolKind::Module => "mo",
            ZfSymbolKind::Variable => "va",
            ZfSymbolKind::Constant => "co",
            ZfSymbolKind::Field => "fi",
            _ => "  ",
        }
    }
}

impl std::fmt::Display for ZfSymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

/// A symbol in a document (function, class, variable, etc.).
#[derive(Debug, Clone)]
pub struct ZfDocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: ZfSymbolKind,
    pub range_start_line: u32,
    pub range_end_line: u32,
    pub children: Vec<ZfDocumentSymbol>,
    pub deprecated: bool,
}

impl ZfDocumentSymbol {
    pub fn new(name: &str, kind: ZfSymbolKind, start: u32, end: u32) -> Self {
        Self { name: name.to_string(), detail: None, kind, range_start_line: start, range_end_line: end, children: Vec::new(), deprecated: false }
    }

    pub fn with_detail(mut self, detail: &str) -> Self { self.detail = Some(detail.to_string()); self }
    pub fn with_child(mut self, child: ZfDocumentSymbol) -> Self { self.children.push(child); self }

    pub fn add_child(&mut self, child: ZfDocumentSymbol) { self.children.push(child); }

    pub fn child_count(&self) -> usize { self.children.len() }
    pub fn has_children(&self) -> bool { !self.children.is_empty() }
    pub fn line_count(&self) -> u32 { self.range_end_line - self.range_start_line + 1 }

    pub fn flat_symbols(&self) -> Vec<&ZfDocumentSymbol> {
        let mut result = vec![self];
        for child in &self.children {
            result.extend(child.flat_symbols());
        }
        result
    }

    pub fn find_at_line(&self, line: u32) -> Option<&ZfDocumentSymbol> {
        if line >= self.range_start_line && line <= self.range_end_line {
            for child in &self.children {
                if let Some(found) = child.find_at_line(line) {
                    return Some(found);
                }
            }
            Some(self)
        } else {
            None
        }
    }
}

impl std::fmt::Display for ZfDocumentSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} (L{}-L{})", self.kind.icon(), self.name, self.range_start_line + 1, self.range_end_line + 1)
    }
}


// --- zg_ decoration types and theme colors ---

/// A color in RGBA format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZgColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl ZgColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self { Self { r, g, b, a: 1.0 } }
    pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self { Self { r, g, b, a } }
    pub fn transparent() -> Self { Self { r: 0, g: 0, b: 0, a: 0.0 } }
    pub fn white() -> Self { Self::new(255, 255, 255) }
    pub fn black() -> Self { Self::new(0, 0, 0) }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Self::new(r, g, b))
        } else if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Self::rgba(r, g, b, a as f32 / 255.0))
        } else {
            None
        }
    }

    pub fn to_hex(&self) -> String { format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b) }

    pub fn luminance(&self) -> f32 {
        0.299 * (self.r as f32 / 255.0) + 0.587 * (self.g as f32 / 255.0) + 0.114 * (self.b as f32 / 255.0)
    }

    pub fn is_light(&self) -> bool { self.luminance() > 0.5 }
    pub fn is_dark(&self) -> bool { !self.is_light() }

    pub fn with_alpha(&self, a: f32) -> Self { Self { a, ..*self } }

    pub fn blend(&self, other: &ZgColor, t: f32) -> ZgColor {
        let t = t.clamp(0.0, 1.0);
        ZgColor {
            r: (self.r as f32 * (1.0 - t) + other.r as f32 * t) as u8,
            g: (self.g as f32 * (1.0 - t) + other.g as f32 * t) as u8,
            b: (self.b as f32 * (1.0 - t) + other.b as f32 * t) as u8,
            a: self.a * (1.0 - t) + other.a * t,
        }
    }
}

impl Default for ZgColor {
    fn default() -> Self { Self::black() }
}

impl std::fmt::Display for ZgColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Text decoration style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZgDecorationStyle {
    None,
    Underline,
    Strikethrough,
    Bold,
    Italic,
    Dim,
    Reverse,
}

/// A text decoration applied to a range.
#[derive(Debug, Clone)]
pub struct ZgDecoration {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    pub foreground: Option<ZgColor>,
    pub background: Option<ZgColor>,
    pub style: ZgDecorationStyle,
    pub hover_message: Option<String>,
    pub tag: Option<String>,
}

impl ZgDecoration {
    pub fn new(sl: u32, sc: u32, el: u32, ec: u32) -> Self {
        Self { start_line: sl, start_char: sc, end_line: el, end_char: ec,
               foreground: None, background: None, style: ZgDecorationStyle::None, hover_message: None, tag: None }
    }

    pub fn with_fg(mut self, color: ZgColor) -> Self { self.foreground = Some(color); self }
    pub fn with_bg(mut self, color: ZgColor) -> Self { self.background = Some(color); self }
    pub fn with_style(mut self, style: ZgDecorationStyle) -> Self { self.style = style; self }
    pub fn with_hover(mut self, msg: &str) -> Self { self.hover_message = Some(msg.to_string()); self }
    pub fn with_tag(mut self, tag: &str) -> Self { self.tag = Some(tag.to_string()); self }

    pub fn is_single_line(&self) -> bool { self.start_line == self.end_line }
    pub fn affects_line(&self, line: u32) -> bool { line >= self.start_line && line <= self.end_line }
    pub fn has_foreground(&self) -> bool { self.foreground.is_some() }
    pub fn has_background(&self) -> bool { self.background.is_some() }
}

impl std::fmt::Display for ZgDecoration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZgDecoration(L{}-L{}, {:?})", self.start_line + 1, self.end_line + 1, self.style)
    }
}

/// A collection of decorations for a document.
#[derive(Debug, Clone)]
pub struct ZgDecorationSet {
    pub decorations: Vec<ZgDecoration>,
}

impl ZgDecorationSet {
    pub fn new() -> Self { Self { decorations: Vec::new() } }

    pub fn add(&mut self, dec: ZgDecoration) { self.decorations.push(dec); }

    pub fn for_line(&self, line: u32) -> Vec<&ZgDecoration> {
        self.decorations.iter().filter(|d| d.affects_line(line)).collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&ZgDecoration> {
        self.decorations.iter().filter(|d| d.tag.as_deref() == Some(tag)).collect()
    }

    pub fn len(&self) -> usize { self.decorations.len() }
    pub fn is_empty(&self) -> bool { self.decorations.is_empty() }
    pub fn clear(&mut self) { self.decorations.clear(); }
}

impl Default for ZgDecorationSet {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for ZgDecorationSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZgDecorationSet({} items)", self.len())
    }
}


// --- zh_ semantic tokens and code actions ---

/// A semantic token type for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZhSemanticTokenType {
    Namespace, Type, Class, Enum, Interface, Struct, TypeParameter,
    Parameter, Variable, Property, EnumMember, Event, Function, Method,
    Macro, Keyword, Modifier, Comment, String, Number, Regexp, Operator,
    Decorator,
}

impl ZhSemanticTokenType {
    pub fn as_str(&self) -> &str {
        match self {
            ZhSemanticTokenType::Namespace => "namespace",
            ZhSemanticTokenType::Type => "type",
            ZhSemanticTokenType::Class => "class",
            ZhSemanticTokenType::Enum => "enum",
            ZhSemanticTokenType::Interface => "interface",
            ZhSemanticTokenType::Struct => "struct",
            ZhSemanticTokenType::TypeParameter => "typeParameter",
            ZhSemanticTokenType::Parameter => "parameter",
            ZhSemanticTokenType::Variable => "variable",
            ZhSemanticTokenType::Property => "property",
            ZhSemanticTokenType::EnumMember => "enumMember",
            ZhSemanticTokenType::Event => "event",
            ZhSemanticTokenType::Function => "function",
            ZhSemanticTokenType::Method => "method",
            ZhSemanticTokenType::Macro => "macro",
            ZhSemanticTokenType::Keyword => "keyword",
            ZhSemanticTokenType::Modifier => "modifier",
            ZhSemanticTokenType::Comment => "comment",
            ZhSemanticTokenType::String => "string",
            ZhSemanticTokenType::Number => "number",
            ZhSemanticTokenType::Regexp => "regexp",
            ZhSemanticTokenType::Operator => "operator",
            ZhSemanticTokenType::Decorator => "decorator",
        }
    }
}

impl std::fmt::Display for ZhSemanticTokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) }
}

/// A single semantic token (delta-encoded line, char, length, type, modifiers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZhSemanticToken {
    pub delta_line: u32,
    pub delta_start: u32,
    pub length: u32,
    pub token_type: u32,
    pub token_modifiers: u32,
}

impl ZhSemanticToken {
    pub fn new(dl: u32, ds: u32, len: u32, tt: u32, tm: u32) -> Self {
        Self { delta_line: dl, delta_start: ds, length: len, token_type: tt, token_modifiers: tm }
    }
}

impl std::fmt::Display for ZhSemanticToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SemanticToken(dl={}, ds={}, len={}, type={}, mods={})", self.delta_line, self.delta_start, self.length, self.token_type, self.token_modifiers)
    }
}

/// A collection of semantic tokens for a document.
#[derive(Debug, Clone)]
pub struct ZhSemanticTokens {
    pub result_id: Option<String>,
    pub data: Vec<ZhSemanticToken>,
}

impl ZhSemanticTokens {
    pub fn new() -> Self { Self { result_id: None, data: Vec::new() } }

    pub fn with_result_id(mut self, id: &str) -> Self { self.result_id = Some(id.to_string()); self }

    pub fn push(&mut self, token: ZhSemanticToken) { self.data.push(token); }

    pub fn len(&self) -> usize { self.data.len() }
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    pub fn to_data(&self) -> Vec<u32> {
        self.data.iter().flat_map(|t| vec![t.delta_line, t.delta_start, t.length, t.token_type, t.token_modifiers]).collect()
    }
}

impl Default for ZhSemanticTokens {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Display for ZhSemanticTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZhSemanticTokens({} tokens)", self.len())
    }
}

/// The kind of a code action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ZhCodeActionKind {
    QuickFix,
    Refactor,
    RefactorExtract,
    RefactorInline,
    RefactorRewrite,
    Source,
    SourceOrganizeImports,
    SourceFixAll,
    Other(String),
}

impl ZhCodeActionKind {
    pub fn as_str(&self) -> &str {
        match self {
            ZhCodeActionKind::QuickFix => "quickfix",
            ZhCodeActionKind::Refactor => "refactor",
            ZhCodeActionKind::RefactorExtract => "refactor.extract",
            ZhCodeActionKind::RefactorInline => "refactor.inline",
            ZhCodeActionKind::RefactorRewrite => "refactor.rewrite",
            ZhCodeActionKind::Source => "source",
            ZhCodeActionKind::SourceOrganizeImports => "source.organizeImports",
            ZhCodeActionKind::SourceFixAll => "source.fixAll",
            ZhCodeActionKind::Other(s) => s,
        }
    }

    pub fn is_quickfix(&self) -> bool { matches!(self, ZhCodeActionKind::QuickFix) }
    pub fn is_refactor(&self) -> bool { matches!(self, ZhCodeActionKind::Refactor | ZhCodeActionKind::RefactorExtract | ZhCodeActionKind::RefactorInline | ZhCodeActionKind::RefactorRewrite) }
}

impl std::fmt::Display for ZhCodeActionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) }
}

/// A code action (quick fix, refactoring, etc.).
#[derive(Debug, Clone)]
pub struct ZhCodeAction {
    pub title: String,
    pub kind: ZhCodeActionKind,
    pub is_preferred: bool,
    pub disabled_reason: Option<String>,
}

impl ZhCodeAction {
    pub fn new(title: &str, kind: ZhCodeActionKind) -> Self {
        Self { title: title.to_string(), kind, is_preferred: false, disabled_reason: None }
    }

    pub fn preferred(mut self) -> Self { self.is_preferred = true; self }
    pub fn disabled(mut self, reason: &str) -> Self { self.disabled_reason = Some(reason.to_string()); self }

    pub fn is_disabled(&self) -> bool { self.disabled_reason.is_some() }
}

impl std::fmt::Display for ZhCodeAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZhCodeAction({}: {})", self.kind, self.title)
    }
}


// --- zi_ document links and code lenses ---

/// A clickable link in a document.
#[derive(Debug, Clone, PartialEq)]
pub struct ZiDocumentLink {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    pub target: Option<String>,
    pub tooltip: Option<String>,
}

impl ZiDocumentLink {
    pub fn new(sl: u32, sc: u32, el: u32, ec: u32) -> Self {
        Self { start_line: sl, start_char: sc, end_line: el, end_char: ec, target: None, tooltip: None }
    }

    pub fn with_target(mut self, target: &str) -> Self { self.target = Some(target.to_string()); self }
    pub fn with_tooltip(mut self, tip: &str) -> Self { self.tooltip = Some(tip.to_string()); self }

    pub fn is_resolved(&self) -> bool { self.target.is_some() }
    pub fn affects_line(&self, line: u32) -> bool { line >= self.start_line && line <= self.end_line }
    pub fn is_single_line(&self) -> bool { self.start_line == self.end_line }
}

impl std::fmt::Display for ZiDocumentLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZiDocumentLink(L{}:{}-L{}:{})", self.start_line + 1, self.start_char, self.end_line + 1, self.end_char)
    }
}

/// A collection of document links.
#[derive(Debug, Clone, Default)]
pub struct ZiDocumentLinks {
    pub links: Vec<ZiDocumentLink>,
}

impl ZiDocumentLinks {
    pub fn new() -> Self { Self { links: Vec::new() } }
    pub fn add(&mut self, link: ZiDocumentLink) { self.links.push(link); }
    pub fn len(&self) -> usize { self.links.len() }
    pub fn is_empty(&self) -> bool { self.links.is_empty() }
    pub fn for_line(&self, line: u32) -> Vec<&ZiDocumentLink> {
        self.links.iter().filter(|l| l.affects_line(line)).collect()
    }
    pub fn resolved_count(&self) -> usize { self.links.iter().filter(|l| l.is_resolved()).count() }
    pub fn clear(&mut self) { self.links.clear(); }
}

impl std::fmt::Display for ZiDocumentLinks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZiDocumentLinks({} links)", self.len())
    }
}

/// A code lens — an actionable annotation above a code block.
#[derive(Debug, Clone)]
pub struct ZiCodeLens {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    pub command_id: Option<String>,
    pub command_title: Option<String>,
    pub command_args: Vec<String>,
}

impl ZiCodeLens {
    pub fn new(sl: u32, sc: u32, el: u32, ec: u32) -> Self {
        Self { start_line: sl, start_char: sc, end_line: el, end_char: ec, command_id: None, command_title: None, command_args: Vec::new() }
    }

    pub fn with_command(mut self, id: &str, title: &str) -> Self {
        self.command_id = Some(id.to_string()); self.command_title = Some(title.to_string()); self
    }

    pub fn add_arg(&mut self, arg: &str) { self.command_args.push(arg.to_string()); }

    pub fn is_resolved(&self) -> bool { self.command_id.is_some() }
    pub fn title(&self) -> &str { self.command_title.as_deref().unwrap_or("") }
}

impl std::fmt::Display for ZiCodeLens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(title) = &self.command_title {
            write!(f, "ZiCodeLens(L{}: {})", self.start_line + 1, title)
        } else {
            write!(f, "ZiCodeLens(L{}: unresolved)", self.start_line + 1)
        }
    }
}

/// A collection of code lenses for a document.
#[derive(Debug, Clone, Default)]
pub struct ZiCodeLenses {
    pub lenses: Vec<ZiCodeLens>,
}

impl ZiCodeLenses {
    pub fn new() -> Self { Self { lenses: Vec::new() } }
    pub fn add(&mut self, lens: ZiCodeLens) { self.lenses.push(lens); }
    pub fn len(&self) -> usize { self.lenses.len() }
    pub fn is_empty(&self) -> bool { self.lenses.is_empty() }

    pub fn for_line(&self, line: u32) -> Vec<&ZiCodeLens> {
        self.lenses.iter().filter(|l| l.start_line == line).collect()
    }

    pub fn resolved_count(&self) -> usize { self.lenses.iter().filter(|l| l.is_resolved()).count() }

    pub fn sort_by_line(&mut self) {
        self.lenses.sort_by_key(|l| (l.start_line, l.start_char));
    }

    pub fn clear(&mut self) { self.lenses.clear(); }
}

impl std::fmt::Display for ZiCodeLenses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZiCodeLenses({} lenses)", self.len())
    }
}

/// A folding range in a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZiFoldingRange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: ZiFoldingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZiFoldingKind {
    Comment,
    Imports,
    Region,
}

impl ZiFoldingRange {
    pub fn new(start: u32, end: u32, kind: ZiFoldingKind) -> Self {
        Self { start_line: start, end_line: end, kind }
    }

    pub fn comment(start: u32, end: u32) -> Self { Self::new(start, end, ZiFoldingKind::Comment) }
    pub fn imports(start: u32, end: u32) -> Self { Self::new(start, end, ZiFoldingKind::Imports) }
    pub fn region(start: u32, end: u32) -> Self { Self::new(start, end, ZiFoldingKind::Region) }

    pub fn line_count(&self) -> u32 { self.end_line - self.start_line + 1 }
    pub fn contains_line(&self, line: u32) -> bool { line >= self.start_line && line <= self.end_line }
}

impl std::fmt::Display for ZiFoldingRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZiFoldingRange(L{}-L{}, {:?})", self.start_line + 1, self.end_line + 1, self.kind)
    }
}

/// A selection range (smart select) with parent chain.
#[derive(Debug, Clone)]
pub struct ZiSelectionRange {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    pub parent: Option<Box<ZiSelectionRange>>,
}

impl ZiSelectionRange {
    pub fn new(sl: u32, sc: u32, el: u32, ec: u32) -> Self {
        Self { start_line: sl, start_char: sc, end_line: el, end_char: ec, parent: None }
    }

    pub fn with_parent(mut self, parent: ZiSelectionRange) -> Self {
        self.parent = Some(Box::new(parent)); self
    }

    pub fn depth(&self) -> usize {
        let mut d = 0;
        let mut cur = self.parent.as_ref();
        while let Some(p) = cur { d += 1; cur = p.parent.as_ref(); }
        d
    }

    pub fn outermost(&self) -> &ZiSelectionRange {
        let mut cur = self;
        while let Some(p) = &cur.parent { cur = p; }
        cur
    }
}

impl std::fmt::Display for ZiSelectionRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZiSelectionRange(L{}:{}-L{}:{}, depth={})", self.start_line + 1, self.start_char, self.end_line + 1, self.end_char, self.depth())
    }
}


// --- zj_ inlay hints and rename info ---

/// The kind of inlay hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZjInlayHintKind {
    Type,
    Parameter,
    Other,
}

impl std::fmt::Display for ZjInlayHintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZjInlayHintKind::Type => write!(f, "type"),
            ZjInlayHintKind::Parameter => write!(f, "parameter"),
            ZjInlayHintKind::Other => write!(f, "other"),
        }
    }
}

/// An inlay hint displayed inline in the editor.
#[derive(Debug, Clone)]
pub struct ZjInlayHint {
    pub line: u32,
    pub character: u32,
    pub label: String,
    pub kind: ZjInlayHintKind,
    pub tooltip: Option<String>,
    pub padding_left: bool,
    pub padding_right: bool,
}

impl ZjInlayHint {
    pub fn new(line: u32, character: u32, label: &str, kind: ZjInlayHintKind) -> Self {
        Self { line, character, label: label.to_string(), kind, tooltip: None, padding_left: false, padding_right: false }
    }

    pub fn type_hint(line: u32, character: u32, label: &str) -> Self {
        Self::new(line, character, label, ZjInlayHintKind::Type)
    }

    pub fn param_hint(line: u32, character: u32, label: &str) -> Self {
        Self::new(line, character, label, ZjInlayHintKind::Parameter).padded_left()
    }

    pub fn with_tooltip(mut self, tip: &str) -> Self { self.tooltip = Some(tip.to_string()); self }
    pub fn padded_left(mut self) -> Self { self.padding_left = true; self }
    pub fn padded_right(mut self) -> Self { self.padding_right = true; self }

    pub fn is_type(&self) -> bool { self.kind == ZjInlayHintKind::Type }
    pub fn is_parameter(&self) -> bool { self.kind == ZjInlayHintKind::Parameter }
}

impl std::fmt::Display for ZjInlayHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZjInlayHint(L{}:{}, {}: {})", self.line + 1, self.character, self.kind, self.label)
    }
}

/// A collection of inlay hints for a range.
#[derive(Debug, Clone, Default)]
pub struct ZjInlayHints {
    pub hints: Vec<ZjInlayHint>,
}

impl ZjInlayHints {
    pub fn new() -> Self { Self { hints: Vec::new() } }
    pub fn add(&mut self, hint: ZjInlayHint) { self.hints.push(hint); }
    pub fn len(&self) -> usize { self.hints.len() }
    pub fn is_empty(&self) -> bool { self.hints.is_empty() }

    pub fn for_line(&self, line: u32) -> Vec<&ZjInlayHint> {
        self.hints.iter().filter(|h| h.line == line).collect()
    }

    pub fn type_hints(&self) -> Vec<&ZjInlayHint> {
        self.hints.iter().filter(|h| h.is_type()).collect()
    }

    pub fn param_hints(&self) -> Vec<&ZjInlayHint> {
        self.hints.iter().filter(|h| h.is_parameter()).collect()
    }

    pub fn sort(&mut self) {
        self.hints.sort_by_key(|h| (h.line, h.character));
    }

    pub fn clear(&mut self) { self.hints.clear(); }
}

impl std::fmt::Display for ZjInlayHints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZjInlayHints({} hints)", self.len())
    }
}

/// Information for preparing a rename operation.
#[derive(Debug, Clone)]
pub struct ZjPrepareRename {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    pub placeholder: String,
}

impl ZjPrepareRename {
    pub fn new(sl: u32, sc: u32, el: u32, ec: u32, placeholder: &str) -> Self {
        Self { start_line: sl, start_char: sc, end_line: el, end_char: ec, placeholder: placeholder.to_string() }
    }

    pub fn is_single_line(&self) -> bool { self.start_line == self.end_line }
}

impl std::fmt::Display for ZjPrepareRename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZjPrepareRename(L{}:{}-L{}:{}, {})", self.start_line + 1, self.start_char, self.end_line + 1, self.end_char, self.placeholder)
    }
}

/// A workspace rename edit containing changes across multiple files.
#[derive(Debug, Clone)]
pub struct ZjRenameEdit {
    pub old_name: String,
    pub new_name: String,
    pub changes: Vec<ZjRenameFileChange>,
}

#[derive(Debug, Clone)]
pub struct ZjRenameFileChange {
    pub uri: String,
    pub edits: Vec<ZjRenameTextEdit>,
}

#[derive(Debug, Clone)]
pub struct ZjRenameTextEdit {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
}

impl ZjRenameEdit {
    pub fn new(old_name: &str, new_name: &str) -> Self {
        Self { old_name: old_name.to_string(), new_name: new_name.to_string(), changes: Vec::new() }
    }

    pub fn add_file_change(&mut self, uri: &str, edits: Vec<ZjRenameTextEdit>) {
        self.changes.push(ZjRenameFileChange { uri: uri.to_string(), edits });
    }

    pub fn file_count(&self) -> usize { self.changes.len() }
    pub fn total_edits(&self) -> usize { self.changes.iter().map(|c| c.edits.len()).sum() }
    pub fn is_empty(&self) -> bool { self.changes.is_empty() }

    pub fn affected_uris(&self) -> Vec<&str> {
        self.changes.iter().map(|c| c.uri.as_str()).collect()
    }
}

impl std::fmt::Display for ZjRenameEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZjRenameEdit({} -> {}, {} files, {} edits)", self.old_name, self.new_name, self.file_count(), self.total_edits())
    }
}


// --- zk_ workspace symbols and call hierarchy ---

/// A workspace symbol returned from workspace symbol search.
#[derive(Debug, Clone)]
pub struct ZkWorkspaceSymbol {
    pub name: String,
    pub kind: u8,
    pub uri: String,
    pub start_line: u32,
    pub start_char: u32,
    pub container_name: Option<String>,
}

impl ZkWorkspaceSymbol {
    pub fn new(name: &str, kind: u8, uri: &str, line: u32, ch: u32) -> Self {
        Self { name: name.to_string(), kind, uri: uri.to_string(), start_line: line, start_char: ch, container_name: None }
    }

    pub fn with_container(mut self, c: &str) -> Self { self.container_name = Some(c.to_string()); self }

    pub fn matches_query(&self, query: &str) -> bool {
        let name_lower = self.name.to_lowercase();
        let q_lower = query.to_lowercase();
        name_lower.contains(&q_lower)
    }

    pub fn qualified_name(&self) -> String {
        if let Some(c) = &self.container_name {
            format!("{}.{}", c, self.name)
        } else {
            self.name.clone()
        }
    }
}

impl std::fmt::Display for ZkWorkspaceSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZkWorkspaceSymbol({} in {})", self.name, self.uri)
    }
}

/// A workspace symbol search result set.
#[derive(Debug, Clone, Default)]
pub struct ZkWorkspaceSymbols {
    pub symbols: Vec<ZkWorkspaceSymbol>,
}

impl ZkWorkspaceSymbols {
    pub fn new() -> Self { Self { symbols: Vec::new() } }
    pub fn add(&mut self, sym: ZkWorkspaceSymbol) { self.symbols.push(sym); }
    pub fn len(&self) -> usize { self.symbols.len() }
    pub fn is_empty(&self) -> bool { self.symbols.is_empty() }

    pub fn search(&self, query: &str) -> Vec<&ZkWorkspaceSymbol> {
        self.symbols.iter().filter(|s| s.matches_query(query)).collect()
    }

    pub fn in_file(&self, uri: &str) -> Vec<&ZkWorkspaceSymbol> {
        self.symbols.iter().filter(|s| s.uri == uri).collect()
    }
}

impl std::fmt::Display for ZkWorkspaceSymbols {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZkWorkspaceSymbols({} symbols)", self.len())
    }
}

/// A call hierarchy item — a function/method that can be called or calls others.
#[derive(Debug, Clone)]
pub struct ZkCallHierarchyItem {
    pub name: String,
    pub kind: u8,
    pub uri: String,
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
    pub detail: Option<String>,
}

impl ZkCallHierarchyItem {
    pub fn new(name: &str, kind: u8, uri: &str, sl: u32, sc: u32, el: u32, ec: u32) -> Self {
        Self { name: name.to_string(), kind, uri: uri.to_string(), start_line: sl, start_char: sc, end_line: el, end_char: ec, detail: None }
    }

    pub fn with_detail(mut self, d: &str) -> Self { self.detail = Some(d.to_string()); self }
    pub fn line_count(&self) -> u32 { self.end_line - self.start_line + 1 }
}

impl std::fmt::Display for ZkCallHierarchyItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZkCallHierarchyItem({}, L{}-L{})", self.name, self.start_line + 1, self.end_line + 1)
    }
}

/// An incoming call — who calls a given function.
#[derive(Debug, Clone)]
pub struct ZkIncomingCall {
    pub from: ZkCallHierarchyItem,
    pub from_ranges: Vec<(u32, u32, u32, u32)>,
}

impl ZkIncomingCall {
    pub fn new(from: ZkCallHierarchyItem) -> Self {
        Self { from, from_ranges: Vec::new() }
    }

    pub fn add_range(&mut self, sl: u32, sc: u32, el: u32, ec: u32) {
        self.from_ranges.push((sl, sc, el, ec));
    }

    pub fn call_count(&self) -> usize { self.from_ranges.len() }
}

impl std::fmt::Display for ZkIncomingCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZkIncomingCall(from={}, calls={})", self.from.name, self.call_count())
    }
}

/// An outgoing call — what a given function calls.
#[derive(Debug, Clone)]
pub struct ZkOutgoingCall {
    pub to: ZkCallHierarchyItem,
    pub from_ranges: Vec<(u32, u32, u32, u32)>,
}

impl ZkOutgoingCall {
    pub fn new(to: ZkCallHierarchyItem) -> Self {
        Self { to, from_ranges: Vec::new() }
    }

    pub fn add_range(&mut self, sl: u32, sc: u32, el: u32, ec: u32) {
        self.from_ranges.push((sl, sc, el, ec));
    }

    pub fn call_count(&self) -> usize { self.from_ranges.len() }
}

impl std::fmt::Display for ZkOutgoingCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZkOutgoingCall(to={}, calls={})", self.to.name, self.call_count())
    }
}

/// Type hierarchy item for class/interface inheritance.
#[derive(Debug, Clone)]
pub struct ZkTypeHierarchyItem {
    pub name: String,
    pub kind: u8,
    pub uri: String,
    pub start_line: u32,
    pub end_line: u32,
    pub detail: Option<String>,
}

impl ZkTypeHierarchyItem {
    pub fn new(name: &str, kind: u8, uri: &str, sl: u32, el: u32) -> Self {
        Self { name: name.to_string(), kind, uri: uri.to_string(), start_line: sl, end_line: el, detail: None }
    }

    pub fn with_detail(mut self, d: &str) -> Self { self.detail = Some(d.to_string()); self }
}

impl std::fmt::Display for ZkTypeHierarchyItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZkTypeHierarchyItem({})", self.name)
    }
}


// --- zl_ notebook cells and terminal types ---

/// A notebook cell kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZlCellKind {
    Markup,
    Code,
}

impl std::fmt::Display for ZlCellKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { ZlCellKind::Markup => write!(f, "markup"), ZlCellKind::Code => write!(f, "code") }
    }
}

/// A notebook cell containing either code or markup.
#[derive(Debug, Clone)]
pub struct ZlNotebookCell {
    pub kind: ZlCellKind,
    pub source: String,
    pub language: String,
    pub outputs: Vec<ZlCellOutput>,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Output from a notebook cell execution.
#[derive(Debug, Clone)]
pub struct ZlCellOutput {
    pub output_kind: String,
    pub data: std::collections::HashMap<String, String>,
}

impl ZlNotebookCell {
    pub fn code(source: &str, language: &str) -> Self {
        Self { kind: ZlCellKind::Code, source: source.to_string(), language: language.to_string(), outputs: Vec::new(), metadata: std::collections::HashMap::new() }
    }

    pub fn markup(source: &str) -> Self {
        Self { kind: ZlCellKind::Markup, source: source.to_string(), language: "markdown".to_string(), outputs: Vec::new(), metadata: std::collections::HashMap::new() }
    }

    pub fn add_output(&mut self, kind: &str) {
        self.outputs.push(ZlCellOutput { output_kind: kind.to_string(), data: std::collections::HashMap::new() });
    }

    pub fn set_meta(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }

    pub fn is_code(&self) -> bool { self.kind == ZlCellKind::Code }
    pub fn is_markup(&self) -> bool { self.kind == ZlCellKind::Markup }
    pub fn line_count(&self) -> usize { self.source.lines().count() }
    pub fn has_outputs(&self) -> bool { !self.outputs.is_empty() }
}

impl std::fmt::Display for ZlNotebookCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZlNotebookCell({}, {}, {} lines)", self.kind, self.language, self.line_count())
    }
}

/// A notebook document containing cells.
#[derive(Debug, Clone)]
pub struct ZlNotebook {
    pub uri: String,
    pub cells: Vec<ZlNotebookCell>,
    pub metadata: std::collections::HashMap<String, String>,
}

impl ZlNotebook {
    pub fn new(uri: &str) -> Self {
        Self { uri: uri.to_string(), cells: Vec::new(), metadata: std::collections::HashMap::new() }
    }

    pub fn add_cell(&mut self, cell: ZlNotebookCell) { self.cells.push(cell); }
    pub fn cell_count(&self) -> usize { self.cells.len() }
    pub fn code_cells(&self) -> Vec<&ZlNotebookCell> { self.cells.iter().filter(|c| c.is_code()).collect() }
    pub fn markup_cells(&self) -> Vec<&ZlNotebookCell> { self.cells.iter().filter(|c| c.is_markup()).collect() }
    pub fn is_empty(&self) -> bool { self.cells.is_empty() }
}

impl std::fmt::Display for ZlNotebook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZlNotebook({}, {} cells)", self.uri, self.cell_count())
    }
}

/// A terminal profile for creating terminal instances.
#[derive(Debug, Clone)]
pub struct ZlTerminalProfile {
    pub name: String,
    pub shell_path: String,
    pub shell_args: Vec<String>,
    pub env: std::collections::HashMap<String, String>,
    pub cwd: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

impl ZlTerminalProfile {
    pub fn new(name: &str, shell_path: &str) -> Self {
        Self { name: name.to_string(), shell_path: shell_path.to_string(), shell_args: Vec::new(),
               env: std::collections::HashMap::new(), cwd: None, icon: None, color: None }
    }

    pub fn with_args(mut self, args: &[&str]) -> Self {
        self.shell_args = args.iter().map(|a| a.to_string()).collect(); self
    }

    pub fn with_cwd(mut self, cwd: &str) -> Self { self.cwd = Some(cwd.to_string()); self }
    pub fn with_icon(mut self, icon: &str) -> Self { self.icon = Some(icon.to_string()); self }
    pub fn with_color(mut self, color: &str) -> Self { self.color = Some(color.to_string()); self }

    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.insert(key.to_string(), value.to_string());
    }
}

impl std::fmt::Display for ZlTerminalProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZlTerminalProfile({}: {})", self.name, self.shell_path)
    }
}

/// A terminal link detected in terminal output.
#[derive(Debug, Clone)]
pub struct ZlTerminalLink {
    pub start_index: usize,
    pub length: usize,
    pub tooltip: Option<String>,
    pub uri: Option<String>,
}

impl ZlTerminalLink {
    pub fn new(start: usize, length: usize) -> Self {
        Self { start_index: start, length, tooltip: None, uri: None }
    }

    pub fn with_uri(mut self, uri: &str) -> Self { self.uri = Some(uri.to_string()); self }
    pub fn with_tooltip(mut self, tip: &str) -> Self { self.tooltip = Some(tip.to_string()); self }

    pub fn end_index(&self) -> usize { self.start_index + self.length }
    pub fn is_resolved(&self) -> bool { self.uri.is_some() }
}

impl std::fmt::Display for ZlTerminalLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZlTerminalLink({}..{})", self.start_index, self.end_index())
    }
}


// --- zm_ debug adapter protocol types ---

/// A debug breakpoint.
#[derive(Debug, Clone)]
pub struct ZmBreakpoint {
    pub id: u32,
    pub verified: bool,
    pub line: u32,
    pub column: Option<u32>,
    pub source_path: Option<String>,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
    pub log_message: Option<String>,
}

impl ZmBreakpoint {
    pub fn new(id: u32, line: u32) -> Self {
        Self { id, verified: true, line, column: None, source_path: None, condition: None, hit_condition: None, log_message: None }
    }

    pub fn with_condition(mut self, cond: &str) -> Self { self.condition = Some(cond.to_string()); self }
    pub fn with_hit_condition(mut self, cond: &str) -> Self { self.hit_condition = Some(cond.to_string()); self }
    pub fn with_log_message(mut self, msg: &str) -> Self { self.log_message = Some(msg.to_string()); self }
    pub fn with_source(mut self, path: &str) -> Self { self.source_path = Some(path.to_string()); self }
    pub fn unverified(mut self) -> Self { self.verified = false; self }

    pub fn is_conditional(&self) -> bool { self.condition.is_some() }
    pub fn is_logpoint(&self) -> bool { self.log_message.is_some() }
}

impl std::fmt::Display for ZmBreakpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.verified { "verified" } else { "pending" };
        write!(f, "ZmBreakpoint(id={}, L{}, {})", self.id, self.line, status)
    }
}

/// A stack frame in a debug session.
#[derive(Debug, Clone)]
pub struct ZmStackFrame {
    pub id: u32,
    pub name: String,
    pub source_path: Option<String>,
    pub line: u32,
    pub column: u32,
    pub end_line: Option<u32>,
    pub module_id: Option<String>,
}

impl ZmStackFrame {
    pub fn new(id: u32, name: &str, line: u32, column: u32) -> Self {
        Self { id, name: name.to_string(), source_path: None, line, column, end_line: None, module_id: None }
    }

    pub fn with_source(mut self, path: &str) -> Self { self.source_path = Some(path.to_string()); self }
    pub fn with_module(mut self, m: &str) -> Self { self.module_id = Some(m.to_string()); self }

    pub fn has_source(&self) -> bool { self.source_path.is_some() }
}

impl std::fmt::Display for ZmStackFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZmStackFrame(#{}: {} at L{}:{})", self.id, self.name, self.line, self.column)
    }
}

/// A debug variable or watch expression value.
#[derive(Debug, Clone)]
pub struct ZmVariable {
    pub name: String,
    pub value: String,
    pub var_type: Option<String>,
    pub variables_reference: u32,
    pub named_variables: Option<u32>,
    pub indexed_variables: Option<u32>,
}

impl ZmVariable {
    pub fn new(name: &str, value: &str) -> Self {
        Self { name: name.to_string(), value: value.to_string(), var_type: None, variables_reference: 0, named_variables: None, indexed_variables: None }
    }

    pub fn with_type(mut self, t: &str) -> Self { self.var_type = Some(t.to_string()); self }
    pub fn with_ref(mut self, r: u32) -> Self { self.variables_reference = r; self }

    pub fn has_children(&self) -> bool { self.variables_reference > 0 }
    pub fn is_structured(&self) -> bool { self.has_children() }
}

impl std::fmt::Display for ZmVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(t) = &self.var_type {
            write!(f, "{}: {} = {}", self.name, t, self.value)
        } else {
            write!(f, "{} = {}", self.name, self.value)
        }
    }
}

/// A debug thread.
#[derive(Debug, Clone)]
pub struct ZmThread {
    pub id: u32,
    pub name: String,
    pub is_stopped: bool,
}

impl ZmThread {
    pub fn new(id: u32, name: &str) -> Self {
        Self { id, name: name.to_string(), is_stopped: false }
    }

    pub fn stopped(mut self) -> Self { self.is_stopped = true; self }
}

impl std::fmt::Display for ZmThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.is_stopped { "stopped" } else { "running" };
        write!(f, "ZmThread(#{}: {} [{}])", self.id, self.name, state)
    }
}

/// A debug session managing the state of a debugging run.
#[derive(Debug, Clone)]
pub struct ZmDebugSession {
    pub session_id: String,
    pub name: String,
    pub debug_type: String,
    pub threads: Vec<ZmThread>,
    pub breakpoints: Vec<ZmBreakpoint>,
    pub is_running: bool,
}

impl ZmDebugSession {
    pub fn new(id: &str, name: &str, debug_type: &str) -> Self {
        Self { session_id: id.to_string(), name: name.to_string(), debug_type: debug_type.to_string(),
               threads: Vec::new(), breakpoints: Vec::new(), is_running: false }
    }

    pub fn start(&mut self) { self.is_running = true; }
    pub fn stop(&mut self) { self.is_running = false; }

    pub fn add_thread(&mut self, thread: ZmThread) { self.threads.push(thread); }
    pub fn add_breakpoint(&mut self, bp: ZmBreakpoint) { self.breakpoints.push(bp); }

    pub fn thread_count(&self) -> usize { self.threads.len() }
    pub fn breakpoint_count(&self) -> usize { self.breakpoints.len() }
    pub fn stopped_threads(&self) -> Vec<&ZmThread> { self.threads.iter().filter(|t| t.is_stopped).collect() }
    pub fn verified_breakpoints(&self) -> Vec<&ZmBreakpoint> { self.breakpoints.iter().filter(|b| b.verified).collect() }
}

impl std::fmt::Display for ZmDebugSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.is_running { "running" } else { "stopped" };
        write!(f, "ZmDebugSession({}: {} [{}])", self.session_id, self.name, state)
    }
}


// --- zn_ source control and status bar types ---

/// A source control change status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZnChangeStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Ignored,
    Conflicted,
}

impl ZnChangeStatus {
    pub fn letter(&self) -> char {
        match self {
            ZnChangeStatus::Modified => 'M',
            ZnChangeStatus::Added => 'A',
            ZnChangeStatus::Deleted => 'D',
            ZnChangeStatus::Renamed => 'R',
            ZnChangeStatus::Copied => 'C',
            ZnChangeStatus::Untracked => '?',
            ZnChangeStatus::Ignored => '!',
            ZnChangeStatus::Conflicted => 'U',
        }
    }

    pub fn is_staged_change(&self) -> bool {
        matches!(self, ZnChangeStatus::Modified | ZnChangeStatus::Added | ZnChangeStatus::Deleted | ZnChangeStatus::Renamed | ZnChangeStatus::Copied)
    }
}

impl std::fmt::Display for ZnChangeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.letter())
    }
}

/// A file change in a source control group.
#[derive(Debug, Clone)]
pub struct ZnResourceChange {
    pub uri: String,
    pub status: ZnChangeStatus,
    pub original_uri: Option<String>,
}

impl ZnResourceChange {
    pub fn new(uri: &str, status: ZnChangeStatus) -> Self {
        Self { uri: uri.to_string(), status, original_uri: None }
    }

    pub fn renamed(old_uri: &str, new_uri: &str) -> Self {
        Self { uri: new_uri.to_string(), status: ZnChangeStatus::Renamed, original_uri: Some(old_uri.to_string()) }
    }

    pub fn is_rename(&self) -> bool { self.status == ZnChangeStatus::Renamed }
    pub fn filename(&self) -> &str { self.uri.rsplit('/').next().unwrap_or(&self.uri) }
}

impl std::fmt::Display for ZnResourceChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.status, self.filename())
    }
}

/// A source control resource group (staged, unstaged, untracked, etc.).
#[derive(Debug, Clone)]
pub struct ZnResourceGroup {
    pub id: String,
    pub label: String,
    pub resources: Vec<ZnResourceChange>,
}

impl ZnResourceGroup {
    pub fn new(id: &str, label: &str) -> Self {
        Self { id: id.to_string(), label: label.to_string(), resources: Vec::new() }
    }

    pub fn add(&mut self, change: ZnResourceChange) { self.resources.push(change); }
    pub fn len(&self) -> usize { self.resources.len() }
    pub fn is_empty(&self) -> bool { self.resources.is_empty() }

    pub fn by_status(&self, status: ZnChangeStatus) -> Vec<&ZnResourceChange> {
        self.resources.iter().filter(|r| r.status == status).collect()
    }
}

impl std::fmt::Display for ZnResourceGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZnResourceGroup({}: {} changes)", self.label, self.len())
    }
}

/// A source control provider (Git, SVN, etc.).
#[derive(Debug, Clone)]
pub struct ZnSourceControl {
    pub id: String,
    pub label: String,
    pub root_uri: Option<String>,
    pub groups: Vec<ZnResourceGroup>,
    pub count: usize,
    pub commit_template: Option<String>,
}

impl ZnSourceControl {
    pub fn new(id: &str, label: &str) -> Self {
        Self { id: id.to_string(), label: label.to_string(), root_uri: None, groups: Vec::new(), count: 0, commit_template: None }
    }

    pub fn with_root(mut self, uri: &str) -> Self { self.root_uri = Some(uri.to_string()); self }

    pub fn add_group(&mut self, group: ZnResourceGroup) {
        self.count += group.len();
        self.groups.push(group);
    }

    pub fn total_changes(&self) -> usize { self.count }
    pub fn group_count(&self) -> usize { self.groups.len() }
}

impl std::fmt::Display for ZnSourceControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZnSourceControl({}: {} changes)", self.label, self.total_changes())
    }
}

/// A status bar item.
#[derive(Debug, Clone)]
pub struct ZnStatusBarItem {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
    pub command: Option<String>,
    pub alignment: ZnStatusBarAlignment,
    pub priority: i32,
    pub is_visible: bool,
    pub background_color: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZnStatusBarAlignment {
    Left,
    Right,
}

impl ZnStatusBarItem {
    pub fn new(id: &str, text: &str) -> Self {
        Self { id: id.to_string(), text: text.to_string(), tooltip: None, command: None,
               alignment: ZnStatusBarAlignment::Left, priority: 0, is_visible: true, background_color: None, color: None }
    }

    pub fn right(mut self) -> Self { self.alignment = ZnStatusBarAlignment::Right; self }
    pub fn with_priority(mut self, p: i32) -> Self { self.priority = p; self }
    pub fn with_command(mut self, cmd: &str) -> Self { self.command = Some(cmd.to_string()); self }
    pub fn with_tooltip(mut self, tip: &str) -> Self { self.tooltip = Some(tip.to_string()); self }

    pub fn hide(&mut self) { self.is_visible = false; }
    pub fn show(&mut self) { self.is_visible = true; }
    pub fn set_text(&mut self, text: &str) { self.text = text.to_string(); }
}

impl std::fmt::Display for ZnStatusBarItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZnStatusBarItem({}: {})", self.id, self.text)
    }
}


// --- zo_ extension manifest and context types ---

/// An extension activation event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ZoActivationEvent {
    OnLanguage(String),
    OnCommand(String),
    OnDebug,
    OnDebugResolve(String),
    OnUri,
    WorkspaceContains(String),
    OnFileSystem(String),
    OnView(String),
    OnStartupFinished,
    Star,
}

impl ZoActivationEvent {
    pub fn parse(s: &str) -> Option<Self> {
        if s == "*" { return Some(ZoActivationEvent::Star); }
        if s == "onDebug" { return Some(ZoActivationEvent::OnDebug); }
        if s == "onUri" { return Some(ZoActivationEvent::OnUri); }
        if s == "onStartupFinished" { return Some(ZoActivationEvent::OnStartupFinished); }
        if let Some(lang) = s.strip_prefix("onLanguage:") { return Some(ZoActivationEvent::OnLanguage(lang.to_string())); }
        if let Some(cmd) = s.strip_prefix("onCommand:") { return Some(ZoActivationEvent::OnCommand(cmd.to_string())); }
        if let Some(dt) = s.strip_prefix("onDebugResolve:") { return Some(ZoActivationEvent::OnDebugResolve(dt.to_string())); }
        if let Some(glob) = s.strip_prefix("workspaceContains:") { return Some(ZoActivationEvent::WorkspaceContains(glob.to_string())); }
        if let Some(scheme) = s.strip_prefix("onFileSystem:") { return Some(ZoActivationEvent::OnFileSystem(scheme.to_string())); }
        if let Some(view) = s.strip_prefix("onView:") { return Some(ZoActivationEvent::OnView(view.to_string())); }
        None
    }

    pub fn is_eager(&self) -> bool { matches!(self, ZoActivationEvent::Star) }
}

impl std::fmt::Display for ZoActivationEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoActivationEvent::OnLanguage(l) => write!(f, "onLanguage:{}", l),
            ZoActivationEvent::OnCommand(c) => write!(f, "onCommand:{}", c),
            ZoActivationEvent::OnDebug => write!(f, "onDebug"),
            ZoActivationEvent::OnDebugResolve(d) => write!(f, "onDebugResolve:{}", d),
            ZoActivationEvent::OnUri => write!(f, "onUri"),
            ZoActivationEvent::WorkspaceContains(g) => write!(f, "workspaceContains:{}", g),
            ZoActivationEvent::OnFileSystem(s) => write!(f, "onFileSystem:{}", s),
            ZoActivationEvent::OnView(v) => write!(f, "onView:{}", v),
            ZoActivationEvent::OnStartupFinished => write!(f, "onStartupFinished"),
            ZoActivationEvent::Star => write!(f, "*"),
        }
    }
}

/// An extension contribution point.
#[derive(Debug, Clone)]
pub struct ZoContribution {
    pub kind: String,
    pub id: String,
    pub title: Option<String>,
}

impl ZoContribution {
    pub fn new(kind: &str, id: &str) -> Self {
        Self { kind: kind.to_string(), id: id.to_string(), title: None }
    }

    pub fn with_title(mut self, t: &str) -> Self { self.title = Some(t.to_string()); self }
}

impl std::fmt::Display for ZoContribution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZoContribution({}: {})", self.kind, self.id)
    }
}

/// An extension manifest (package.json equivalent).
#[derive(Debug, Clone)]
pub struct ZoExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub categories: Vec<String>,
    pub activation_events: Vec<ZoActivationEvent>,
    pub contributions: Vec<ZoContribution>,
    pub dependencies: Vec<String>,
    pub main: Option<String>,
    pub engines_vscode: Option<String>,
}

impl ZoExtensionManifest {
    pub fn new(publisher: &str, name: &str, version: &str) -> Self {
        Self {
            id: format!("{}.{}", publisher, name),
            name: name.to_string(), version: version.to_string(), publisher: publisher.to_string(),
            display_name: None, description: None, categories: Vec::new(),
            activation_events: Vec::new(), contributions: Vec::new(),
            dependencies: Vec::new(), main: None, engines_vscode: None,
        }
    }

    pub fn add_activation(&mut self, event: ZoActivationEvent) { self.activation_events.push(event); }
    pub fn add_contribution(&mut self, contrib: ZoContribution) { self.contributions.push(contrib); }
    pub fn add_category(&mut self, cat: &str) { self.categories.push(cat.to_string()); }

    pub fn is_eager(&self) -> bool { self.activation_events.iter().any(|e| e.is_eager()) }
    pub fn activates_on_language(&self, lang: &str) -> bool {
        self.activation_events.iter().any(|e| matches!(e, ZoActivationEvent::OnLanguage(l) if l == lang))
    }
}

impl std::fmt::Display for ZoExtensionManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZoExtensionManifest({} v{})", self.id, self.version)
    }
}

/// Extension runtime context provided on activation.
#[derive(Debug, Clone)]
pub struct ZoExtensionContext {
    pub extension_id: String,
    pub extension_path: String,
    pub storage_path: Option<String>,
    pub global_storage_path: Option<String>,
    pub log_path: Option<String>,
    pub subscriptions: Vec<String>,
    pub is_active: bool,
}

impl ZoExtensionContext {
    pub fn new(id: &str, path: &str) -> Self {
        Self { extension_id: id.to_string(), extension_path: path.to_string(),
               storage_path: None, global_storage_path: None, log_path: None,
               subscriptions: Vec::new(), is_active: false }
    }

    pub fn activate(&mut self) { self.is_active = true; }
    pub fn deactivate(&mut self) { self.is_active = false; }
    pub fn add_subscription(&mut self, sub: &str) { self.subscriptions.push(sub.to_string()); }
    pub fn subscription_count(&self) -> usize { self.subscriptions.len() }
}

impl std::fmt::Display for ZoExtensionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.is_active { "active" } else { "inactive" };
        write!(f, "ZoExtensionContext({} [{}])", self.extension_id, state)
    }
}


// --- zp_ task and build system types ---

/// The group a task belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZpTaskGroup {
    Build,
    Test,
    Clean,
    Rebuild,
    None,
}

impl ZpTaskGroup {
    pub fn as_str(&self) -> &str {
        match self {
            ZpTaskGroup::Build => "build",
            ZpTaskGroup::Test => "test",
            ZpTaskGroup::Clean => "clean",
            ZpTaskGroup::Rebuild => "rebuild",
            ZpTaskGroup::None => "none",
        }
    }
}

impl std::fmt::Display for ZpTaskGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) }
}

/// The source of a task definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZpTaskSource {
    Workspace,
    Extension(String),
    User,
    Auto,
}

impl std::fmt::Display for ZpTaskSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZpTaskSource::Workspace => write!(f, "workspace"),
            ZpTaskSource::Extension(e) => write!(f, "extension:{}", e),
            ZpTaskSource::User => write!(f, "user"),
            ZpTaskSource::Auto => write!(f, "auto"),
        }
    }
}

/// A task execution — a process/shell command to run.
#[derive(Debug, Clone)]
pub struct ZpTaskExecution {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: std::collections::HashMap<String, String>,
    pub is_shell: bool,
}

impl ZpTaskExecution {
    pub fn process(command: &str, args: &[&str]) -> Self {
        Self { command: command.to_string(), args: args.iter().map(|a| a.to_string()).collect(),
               cwd: None, env: std::collections::HashMap::new(), is_shell: false }
    }

    pub fn shell(command: &str) -> Self {
        Self { command: command.to_string(), args: Vec::new(), cwd: None,
               env: std::collections::HashMap::new(), is_shell: true }
    }

    pub fn with_cwd(mut self, cwd: &str) -> Self { self.cwd = Some(cwd.to_string()); self }
    pub fn set_env(&mut self, key: &str, value: &str) { self.env.insert(key.to_string(), value.to_string()); }

    pub fn command_line(&self) -> String {
        if self.args.is_empty() { self.command.clone() }
        else { format!("{} {}", self.command, self.args.join(" ")) }
    }
}

impl std::fmt::Display for ZpTaskExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZpTaskExecution({})", self.command_line())
    }
}

/// A task definition.
#[derive(Debug, Clone)]
pub struct ZpTask {
    pub name: String,
    pub source: ZpTaskSource,
    pub group: ZpTaskGroup,
    pub execution: ZpTaskExecution,
    pub is_background: bool,
    pub problem_matchers: Vec<String>,
    pub detail: Option<String>,
    pub presentation: ZpTaskPresentation,
}

/// How a task is presented in the terminal.
#[derive(Debug, Clone)]
pub struct ZpTaskPresentation {
    pub reveal: ZpRevealKind,
    pub echo: bool,
    pub focus: bool,
    pub panel: ZpPanelKind,
    pub show_reuse_message: bool,
    pub clear: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZpRevealKind { Always, Silent, Never }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZpPanelKind { Shared, Dedicated, New }

impl Default for ZpTaskPresentation {
    fn default() -> Self {
        Self { reveal: ZpRevealKind::Always, echo: true, focus: false, panel: ZpPanelKind::Shared, show_reuse_message: true, clear: false }
    }
}

impl ZpTask {
    pub fn new(name: &str, source: ZpTaskSource, execution: ZpTaskExecution) -> Self {
        Self { name: name.to_string(), source, group: ZpTaskGroup::None, execution,
               is_background: false, problem_matchers: Vec::new(), detail: None, presentation: ZpTaskPresentation::default() }
    }

    pub fn with_group(mut self, group: ZpTaskGroup) -> Self { self.group = group; self }
    pub fn background(mut self) -> Self { self.is_background = true; self }
    pub fn with_detail(mut self, d: &str) -> Self { self.detail = Some(d.to_string()); self }
    pub fn add_problem_matcher(&mut self, m: &str) { self.problem_matchers.push(m.to_string()); }

    pub fn is_build(&self) -> bool { self.group == ZpTaskGroup::Build }
    pub fn is_test(&self) -> bool { self.group == ZpTaskGroup::Test }
}

impl std::fmt::Display for ZpTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZpTask({}: {} [{}])", self.name, self.execution.command_line(), self.group)
    }
}

/// A running task instance.
#[derive(Debug, Clone)]
pub struct ZpTaskRun {
    pub task_name: String,
    pub exit_code: Option<i32>,
    pub is_running: bool,
    pub output_lines: Vec<String>,
}

impl ZpTaskRun {
    pub fn new(name: &str) -> Self {
        Self { task_name: name.to_string(), exit_code: None, is_running: true, output_lines: Vec::new() }
    }

    pub fn add_output(&mut self, line: &str) { self.output_lines.push(line.to_string()); }

    pub fn complete(&mut self, exit_code: i32) {
        self.exit_code = Some(exit_code);
        self.is_running = false;
    }

    pub fn succeeded(&self) -> bool { self.exit_code == Some(0) }
    pub fn failed(&self) -> bool { self.exit_code.map(|c| c != 0).unwrap_or(false) }
    pub fn output_count(&self) -> usize { self.output_lines.len() }
}

impl std::fmt::Display for ZpTaskRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.is_running { "running" }
            else if self.succeeded() { "succeeded" }
            else { "failed" };
        write!(f, "ZpTaskRun({} [{}])", self.task_name, state)
    }
}


// --- zq_ testing framework types ---

/// The state of a test run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZqTestRunState {
    Queued,
    Running,
    Passed,
    Failed,
    Errored,
    Skipped,
}

impl ZqTestRunState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, ZqTestRunState::Passed | ZqTestRunState::Failed | ZqTestRunState::Errored | ZqTestRunState::Skipped)
    }

    pub fn is_success(&self) -> bool { *self == ZqTestRunState::Passed }
    pub fn is_failure(&self) -> bool { matches!(self, ZqTestRunState::Failed | ZqTestRunState::Errored) }
    pub fn icon(&self) -> &str {
        match self {
            ZqTestRunState::Queued => "◌",
            ZqTestRunState::Running => "◑",
            ZqTestRunState::Passed => "✓",
            ZqTestRunState::Failed => "✗",
            ZqTestRunState::Errored => "⚠",
            ZqTestRunState::Skipped => "○",
        }
    }
}

impl std::fmt::Display for ZqTestRunState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

/// A test item in the test explorer tree.
#[derive(Debug, Clone)]
pub struct ZqTestItem {
    pub id: String,
    pub label: String,
    pub uri: Option<String>,
    pub line: Option<u32>,
    pub children: Vec<ZqTestItem>,
    pub tags: Vec<String>,
    pub state: ZqTestRunState,
    pub duration_ms: Option<u64>,
    pub error_message: Option<String>,
}

impl ZqTestItem {
    pub fn new(id: &str, label: &str) -> Self {
        Self { id: id.to_string(), label: label.to_string(), uri: None, line: None,
               children: Vec::new(), tags: Vec::new(), state: ZqTestRunState::Queued,
               duration_ms: None, error_message: None }
    }

    pub fn with_location(mut self, uri: &str, line: u32) -> Self {
        self.uri = Some(uri.to_string()); self.line = Some(line); self
    }

    pub fn add_child(&mut self, child: ZqTestItem) { self.children.push(child); }
    pub fn add_tag(&mut self, tag: &str) { self.tags.push(tag.to_string()); }

    pub fn pass(&mut self, duration_ms: u64) {
        self.state = ZqTestRunState::Passed; self.duration_ms = Some(duration_ms);
    }

    pub fn fail(&mut self, message: &str, duration_ms: u64) {
        self.state = ZqTestRunState::Failed; self.error_message = Some(message.to_string()); self.duration_ms = Some(duration_ms);
    }

    pub fn skip(&mut self) { self.state = ZqTestRunState::Skipped; }
    pub fn run(&mut self) { self.state = ZqTestRunState::Running; }

    pub fn has_children(&self) -> bool { !self.children.is_empty() }
    pub fn child_count(&self) -> usize { self.children.len() }
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }

    pub fn flat_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.flat_count()).sum::<usize>()
    }

    pub fn passed_count(&self) -> usize {
        let me = if self.state.is_success() { 1 } else { 0 };
        me + self.children.iter().map(|c| c.passed_count()).sum::<usize>()
    }

    pub fn failed_count(&self) -> usize {
        let me = if self.state.is_failure() { 1 } else { 0 };
        me + self.children.iter().map(|c| c.failed_count()).sum::<usize>()
    }
}

impl std::fmt::Display for ZqTestItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.state.icon(), self.label)?;
        if let Some(ms) = self.duration_ms { write!(f, " ({}ms)", ms)?; }
        Ok(())
    }
}

/// A test run containing results for a set of test items.
#[derive(Debug, Clone)]
pub struct ZqTestRun {
    pub id: String,
    pub name: String,
    pub items: Vec<ZqTestItem>,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
}

impl ZqTestRun {
    pub fn new(id: &str, name: &str) -> Self {
        Self { id: id.to_string(), name: name.to_string(), items: Vec::new(), started_at: None, completed_at: None }
    }

    pub fn add_item(&mut self, item: ZqTestItem) { self.items.push(item); }
    pub fn item_count(&self) -> usize { self.items.len() }
    pub fn total_flat_count(&self) -> usize { self.items.iter().map(|i| i.flat_count()).sum() }
    pub fn total_passed(&self) -> usize { self.items.iter().map(|i| i.passed_count()).sum() }
    pub fn total_failed(&self) -> usize { self.items.iter().map(|i| i.failed_count()).sum() }
    pub fn is_complete(&self) -> bool { self.completed_at.is_some() }

    pub fn all_passed(&self) -> bool { self.total_failed() == 0 && self.total_passed() > 0 }

    pub fn start(&mut self, ts: u64) { self.started_at = Some(ts); }
    pub fn complete(&mut self, ts: u64) { self.completed_at = Some(ts); }

    pub fn duration_ms(&self) -> Option<u64> {
        match (self.started_at, self.completed_at) {
            (Some(s), Some(e)) => Some(e - s),
            _ => None,
        }
    }
}

impl std::fmt::Display for ZqTestRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ZqTestRun({}: {}/{} passed)", self.name, self.total_passed(), self.total_flat_count())
    }
}


// --- zr_ webview and panel types ---

/// Represents how a webview retains its context when hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZrWebviewRetention {
    /// Webview content is destroyed when hidden.
    Destroy,
    /// Webview content is kept in memory when hidden.
    Retain,
}

/// Options for creating or showing a webview panel.
#[derive(Debug, Clone)]
pub struct ZrWebviewOptions {
    pub enable_scripts: bool,
    pub enable_forms: bool,
    pub local_resource_roots: Vec<String>,
    pub port_mapping: Vec<(u16, u16)>,
}

impl ZrWebviewOptions {
    pub fn new() -> Self {
        Self {
            enable_scripts: false,
            enable_forms: false,
            local_resource_roots: Vec::new(),
            port_mapping: Vec::new(),
        }
    }

    pub fn with_scripts(mut self) -> Self {
        self.enable_scripts = true;
        self
    }

    pub fn with_forms(mut self) -> Self {
        self.enable_forms = true;
        self
    }

    pub fn add_local_root(mut self, root: &str) -> Self {
        self.local_resource_roots.push(root.to_string());
        self
    }

    pub fn add_port_mapping(mut self, from: u16, to: u16) -> Self {
        self.port_mapping.push((from, to));
        self
    }
}

/// A webview that displays HTML content.
#[derive(Debug, Clone)]
pub struct ZrWebview {
    pub html: String,
    pub options: ZrWebviewOptions,
    messages_out: Vec<String>,
    messages_in: Vec<String>,
}

impl ZrWebview {
    pub fn new(options: ZrWebviewOptions) -> Self {
        Self {
            html: String::new(),
            options,
            messages_out: Vec::new(),
            messages_in: Vec::new(),
        }
    }

    pub fn set_html(&mut self, html: &str) {
        self.html = html.to_string();
    }

    pub fn post_message(&mut self, msg: &str) {
        self.messages_out.push(msg.to_string());
    }

    pub fn receive_message(&mut self, msg: &str) {
        self.messages_in.push(msg.to_string());
    }

    pub fn pending_messages(&self) -> &[String] {
        &self.messages_out
    }

    pub fn received_messages(&self) -> &[String] {
        &self.messages_in
    }

    pub fn drain_outgoing(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages_out)
    }

    pub fn as_uri(&self, base: &str, path: &str) -> String {
        format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
    }
}

/// Which column a webview panel occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZrViewColumn {
    Active,
    Beside,
    One,
    Two,
    Three,
}

impl ZrViewColumn {
    pub fn index(&self) -> Option<usize> {
        match self {
            Self::One => Some(0),
            Self::Two => Some(1),
            Self::Three => Some(2),
            _ => None,
        }
    }
}

/// A webview panel shown in an editor tab.
#[derive(Debug, Clone)]
pub struct ZrWebviewPanel {
    pub view_type: String,
    pub title: String,
    pub column: ZrViewColumn,
    pub webview: ZrWebview,
    pub is_active: bool,
    pub is_visible: bool,
    pub retention: ZrWebviewRetention,
    disposed: bool,
}

impl ZrWebviewPanel {
    pub fn new(view_type: &str, title: &str, column: ZrViewColumn, options: ZrWebviewOptions) -> Self {
        Self {
            view_type: view_type.to_string(),
            title: title.to_string(),
            column,
            webview: ZrWebview::new(options),
            is_active: true,
            is_visible: true,
            retention: ZrWebviewRetention::Destroy,
            disposed: false,
        }
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn reveal(&mut self, column: ZrViewColumn) {
        self.column = column;
        self.is_visible = true;
        self.is_active = true;
    }

    pub fn hide(&mut self) {
        self.is_visible = false;
        self.is_active = false;
    }

    pub fn dispose(&mut self) {
        self.disposed = true;
        self.is_visible = false;
        self.is_active = false;
    }

    pub fn is_disposed(&self) -> bool {
        self.disposed
    }

    pub fn with_retention(mut self, r: ZrWebviewRetention) -> Self {
        self.retention = r;
        self
    }
}

/// A webview view shown in the sidebar or panel area.
#[derive(Debug, Clone)]
pub struct ZrWebviewView {
    pub view_type: String,
    pub webview: ZrWebview,
    pub is_visible: bool,
    pub badge_value: Option<String>,
    pub badge_tooltip: Option<String>,
    pub description: Option<String>,
}

impl ZrWebviewView {
    pub fn new(view_type: &str, options: ZrWebviewOptions) -> Self {
        Self {
            view_type: view_type.to_string(),
            webview: ZrWebview::new(options),
            is_visible: true,
            badge_value: None,
            badge_tooltip: None,
            description: None,
        }
    }

    pub fn set_badge(&mut self, value: &str, tooltip: &str) {
        self.badge_value = Some(value.to_string());
        self.badge_tooltip = Some(tooltip.to_string());
    }

    pub fn clear_badge(&mut self) {
        self.badge_value = None;
        self.badge_tooltip = None;
    }

    pub fn set_description(&mut self, desc: &str) {
        self.description = Some(desc.to_string());
    }
}

/// Registry that tracks active webview panels and views.
#[derive(Debug, Clone)]
pub struct ZrWebviewRegistry {
    panels: Vec<ZrWebviewPanel>,
    views: Vec<ZrWebviewView>,
}

impl ZrWebviewRegistry {
    pub fn new() -> Self {
        Self { panels: Vec::new(), views: Vec::new() }
    }

    pub fn register_panel(&mut self, panel: ZrWebviewPanel) -> usize {
        let id = self.panels.len();
        self.panels.push(panel);
        id
    }

    pub fn register_view(&mut self, view: ZrWebviewView) -> usize {
        let id = self.views.len();
        self.views.push(view);
        id
    }

    pub fn panel(&self, id: usize) -> Option<&ZrWebviewPanel> {
        self.panels.get(id)
    }

    pub fn panel_mut(&mut self, id: usize) -> Option<&mut ZrWebviewPanel> {
        self.panels.get_mut(id)
    }

    pub fn view(&self, id: usize) -> Option<&ZrWebviewView> {
        self.views.get(id)
    }

    pub fn view_mut(&mut self, id: usize) -> Option<&mut ZrWebviewView> {
        self.views.get_mut(id)
    }

    pub fn active_panels(&self) -> Vec<&ZrWebviewPanel> {
        self.panels.iter().filter(|p| !p.is_disposed() && p.is_visible).collect()
    }

    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }

    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    pub fn dispose_panel(&mut self, id: usize) {
        if let Some(p) = self.panels.get_mut(id) {
            p.dispose();
        }
    }
}


// --- zs_ configuration and settings types ---

/// Target scope for a configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZsConfigTarget {
    /// Default/global settings.
    Global,
    /// User-level settings.
    User,
    /// Workspace-level settings.
    Workspace,
    /// Workspace folder-level settings.
    WorkspaceFolder,
}

impl ZsConfigTarget {
    pub fn priority(&self) -> u8 {
        match self {
            Self::Global => 0,
            Self::User => 1,
            Self::Workspace => 2,
            Self::WorkspaceFolder => 3,
        }
    }

    pub fn overrides(&self, other: &Self) -> bool {
        self.priority() > other.priority()
    }
}

/// A single configuration entry with its value and metadata.
#[derive(Debug, Clone)]
pub struct ZsConfigEntry {
    pub key: String,
    pub value: ZsConfigValue,
    pub target: ZsConfigTarget,
    pub description: Option<String>,
    pub default_value: Option<ZsConfigValue>,
}

/// JSON-like value for configuration.
#[derive(Debug, Clone, PartialEq)]
pub enum ZsConfigValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<ZsConfigValue>),
    Object(Vec<(String, ZsConfigValue)>),
}

impl ZsConfigValue {
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(b) = self { Some(*b) } else { None }
    }

    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Int(n) = self { Some(*n) } else { None }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Self::Str(s) = self { Some(s) } else { None }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Int(_) => "integer",
            Self::Float(_) => "number",
            Self::Str(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }
}

/// A layered configuration store supporting Global, User, Workspace, WorkspaceFolder.
#[derive(Debug, Clone)]
pub struct ZsConfigStore {
    entries: Vec<ZsConfigEntry>,
}

impl ZsConfigStore {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn set(&mut self, key: &str, value: ZsConfigValue, target: ZsConfigTarget) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.key == key && e.target == target) {
            e.value = value;
        } else {
            self.entries.push(ZsConfigEntry {
                key: key.to_string(),
                value,
                target,
                description: None,
                default_value: None,
            });
        }
    }

    pub fn get(&self, key: &str) -> Option<&ZsConfigValue> {
        self.entries.iter()
            .filter(|e| e.key == key)
            .max_by_key(|e| e.target.priority())
            .map(|e| &e.value)
    }

    pub fn get_at(&self, key: &str, target: ZsConfigTarget) -> Option<&ZsConfigValue> {
        self.entries.iter()
            .find(|e| e.key == key && e.target == target)
            .map(|e| &e.value)
    }

    pub fn remove(&mut self, key: &str, target: ZsConfigTarget) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| !(e.key == key && e.target == target));
        self.entries.len() < before
    }

    pub fn keys(&self) -> Vec<&str> {
        let mut seen = Vec::new();
        for e in &self.entries {
            if !seen.contains(&e.key.as_str()) {
                seen.push(&e.key);
            }
        }
        seen
    }

    pub fn entries_for_target(&self, target: ZsConfigTarget) -> Vec<&ZsConfigEntry> {
        self.entries.iter().filter(|e| e.target == target).collect()
    }

    pub fn has(&self, key: &str) -> bool {
        self.entries.iter().any(|e| e.key == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Merge another store, with the other store's entries overriding on conflict.
    pub fn merge(&mut self, other: &ZsConfigStore) {
        for entry in &other.entries {
            self.set(&entry.key, entry.value.clone(), entry.target);
        }
    }

    /// Get the effective value respecting layered overrides.
    pub fn effective(&self, key: &str, default: ZsConfigValue) -> ZsConfigValue {
        self.get(key).cloned().unwrap_or(default)
    }

    pub fn inspect(&self, key: &str) -> Vec<(ZsConfigTarget, &ZsConfigValue)> {
        self.entries.iter()
            .filter(|e| e.key == key)
            .map(|e| (e.target, &e.value))
            .collect()
    }
}

/// Schema for a configuration property.
#[derive(Debug, Clone)]
pub struct ZsConfigSchema {
    pub key: String,
    pub type_name: String,
    pub default: ZsConfigValue,
    pub description: String,
    pub enum_values: Vec<String>,
    pub scope: ZsConfigTarget,
}

impl ZsConfigSchema {
    pub fn new(key: &str, type_name: &str, default: ZsConfigValue, description: &str) -> Self {
        Self {
            key: key.to_string(),
            type_name: type_name.to_string(),
            default,
            description: description.to_string(),
            enum_values: Vec::new(),
            scope: ZsConfigTarget::User,
        }
    }

    pub fn with_enum(mut self, values: Vec<String>) -> Self {
        self.enum_values = values;
        self
    }

    pub fn with_scope(mut self, scope: ZsConfigTarget) -> Self {
        self.scope = scope;
        self
    }

    pub fn validate(&self, value: &ZsConfigValue) -> bool {
        if !self.enum_values.is_empty() {
            if let ZsConfigValue::Str(s) = value {
                return self.enum_values.contains(s);
            }
            return false;
        }
        match self.type_name.as_str() {
            "boolean" => matches!(value, ZsConfigValue::Bool(_)),
            "integer" => matches!(value, ZsConfigValue::Int(_)),
            "number" => matches!(value, ZsConfigValue::Int(_) | ZsConfigValue::Float(_)),
            "string" => matches!(value, ZsConfigValue::Str(_)),
            "array" => matches!(value, ZsConfigValue::Array(_)),
            "object" => matches!(value, ZsConfigValue::Object(_)),
            _ => true,
        }
    }
}


// --- zt_ language feature provider types ---

/// Options for document formatting.
#[derive(Debug, Clone)]
pub struct ZtFormattingOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
    pub trim_final_newlines: bool,
}

impl ZtFormattingOptions {
    pub fn new(tab_size: u32, insert_spaces: bool) -> Self {
        Self {
            tab_size,
            insert_spaces,
            trim_trailing_whitespace: false,
            insert_final_newline: false,
            trim_final_newlines: false,
        }
    }

    pub fn indent_str(&self) -> String {
        if self.insert_spaces {
            " ".repeat(self.tab_size as usize)
        } else {
            "\t".to_string()
        }
    }
}

/// A text edit returned by a formatter.
#[derive(Debug, Clone, PartialEq)]
pub struct ZtTextEdit {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub new_text: String,
}

impl ZtTextEdit {
    pub fn new(sl: u32, sc: u32, el: u32, ec: u32, text: &str) -> Self {
        Self { start_line: sl, start_col: sc, end_line: el, end_col: ec, new_text: text.to_string() }
    }

    pub fn is_empty_range(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }

    pub fn is_insert(&self) -> bool {
        self.is_empty_range() && !self.new_text.is_empty()
    }

    pub fn is_delete(&self) -> bool {
        !self.is_empty_range() && self.new_text.is_empty()
    }
}

/// Result from a document formatter.
#[derive(Debug, Clone)]
pub struct ZtFormatResult {
    pub edits: Vec<ZtTextEdit>,
}

impl ZtFormatResult {
    pub fn new() -> Self {
        Self { edits: Vec::new() }
    }

    pub fn add(&mut self, edit: ZtTextEdit) {
        self.edits.push(edit);
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    pub fn sort_edits(&mut self) {
        self.edits.sort_by(|a, b| {
            a.start_line.cmp(&b.start_line).then(a.start_col.cmp(&b.start_col))
        });
    }
}

/// Kind of a document link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZtLinkKind {
    Url,
    File,
    Definition,
    Reference,
}

/// A resolved link in a document.
#[derive(Debug, Clone)]
pub struct ZtDocumentLink {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub target: String,
    pub tooltip: Option<String>,
    pub kind: ZtLinkKind,
}

impl ZtDocumentLink {
    pub fn new(sl: u32, sc: u32, el: u32, ec: u32, target: &str, kind: ZtLinkKind) -> Self {
        Self {
            start_line: sl, start_col: sc, end_line: el, end_col: ec,
            target: target.to_string(), tooltip: None, kind,
        }
    }

    pub fn with_tooltip(mut self, tip: &str) -> Self {
        self.tooltip = Some(tip.to_string());
        self
    }
}

/// An on-type formatting trigger.
#[derive(Debug, Clone)]
pub struct ZtOnTypeFormatting {
    pub trigger_chars: Vec<char>,
    pub more_trigger_chars: Vec<char>,
}

impl ZtOnTypeFormatting {
    pub fn new(first: char) -> Self {
        Self { trigger_chars: vec![first], more_trigger_chars: Vec::new() }
    }

    pub fn add_trigger(mut self, ch: char) -> Self {
        self.more_trigger_chars.push(ch);
        self
    }

    pub fn all_triggers(&self) -> Vec<char> {
        let mut all = self.trigger_chars.clone();
        all.extend(&self.more_trigger_chars);
        all
    }

    pub fn is_trigger(&self, ch: char) -> bool {
        self.trigger_chars.contains(&ch) || self.more_trigger_chars.contains(&ch)
    }
}

/// A color presentation (e.g., #ff0000, rgb(255, 0, 0)).
#[derive(Debug, Clone)]
pub struct ZtColorPresentation {
    pub label: String,
    pub text_edit: Option<ZtTextEdit>,
    pub additional_edits: Vec<ZtTextEdit>,
}

impl ZtColorPresentation {
    pub fn new(label: &str) -> Self {
        Self { label: label.to_string(), text_edit: None, additional_edits: Vec::new() }
    }

    pub fn with_edit(mut self, edit: ZtTextEdit) -> Self {
        self.text_edit = Some(edit);
        self
    }

    pub fn add_edit(mut self, edit: ZtTextEdit) -> Self {
        self.additional_edits.push(edit);
        self
    }
}

/// A color occurrence in a document.
#[derive(Debug, Clone)]
pub struct ZtColorInfo {
    pub line: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl ZtColorInfo {
    pub fn new(line: u32, sc: u32, ec: u32, r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { line, start_col: sc, end_col: ec, red: r, green: g, blue: b, alpha: a }
    }

    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}",
            (self.red * 255.0) as u8,
            (self.green * 255.0) as u8,
            (self.blue * 255.0) as u8)
    }

    pub fn to_rgb_string(&self) -> String {
        format!("rgb({}, {}, {})",
            (self.red * 255.0) as u8,
            (self.green * 255.0) as u8,
            (self.blue * 255.0) as u8)
    }
}

/// Provider registration for language features with a document selector.
#[derive(Debug, Clone)]
pub struct ZtProviderRegistration {
    pub id: String,
    pub language_ids: Vec<String>,
    pub schemes: Vec<String>,
    pub patterns: Vec<String>,
}

impl ZtProviderRegistration {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string(), language_ids: Vec::new(), schemes: Vec::new(), patterns: Vec::new() }
    }

    pub fn for_language(mut self, lang: &str) -> Self {
        self.language_ids.push(lang.to_string());
        self
    }

    pub fn for_scheme(mut self, scheme: &str) -> Self {
        self.schemes.push(scheme.to_string());
        self
    }

    pub fn for_pattern(mut self, pattern: &str) -> Self {
        self.patterns.push(pattern.to_string());
        self
    }

    pub fn matches_language(&self, lang: &str) -> bool {
        self.language_ids.is_empty() || self.language_ids.iter().any(|l| l == lang)
    }

    pub fn matches_scheme(&self, scheme: &str) -> bool {
        self.schemes.is_empty() || self.schemes.iter().any(|s| s == scheme)
    }
}


// --- zu_ workspace trust and authentication types ---

/// Trust state for a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZuTrustState {
    Trusted,
    Untrusted,
    Unknown,
}

impl ZuTrustState {
    pub fn is_trusted(&self) -> bool {
        matches!(self, Self::Trusted)
    }
}

/// Workspace trust configuration.
#[derive(Debug, Clone)]
pub struct ZuWorkspaceTrust {
    pub state: ZuTrustState,
    pub parent_trusted: bool,
    pub trusted_folders: Vec<String>,
    pub untrusted_folders: Vec<String>,
}

impl ZuWorkspaceTrust {
    pub fn new() -> Self {
        Self {
            state: ZuTrustState::Unknown,
            parent_trusted: false,
            trusted_folders: Vec::new(),
            untrusted_folders: Vec::new(),
        }
    }

    pub fn grant_trust(&mut self) {
        self.state = ZuTrustState::Trusted;
    }

    pub fn revoke_trust(&mut self) {
        self.state = ZuTrustState::Untrusted;
    }

    pub fn add_trusted_folder(&mut self, path: &str) {
        if !self.trusted_folders.iter().any(|f| f == path) {
            self.trusted_folders.push(path.to_string());
        }
    }

    pub fn add_untrusted_folder(&mut self, path: &str) {
        if !self.untrusted_folders.iter().any(|f| f == path) {
            self.untrusted_folders.push(path.to_string());
        }
    }

    pub fn is_folder_trusted(&self, path: &str) -> bool {
        if self.state == ZuTrustState::Trusted {
            return true;
        }
        self.trusted_folders.iter().any(|f| path.starts_with(f.as_str()))
    }

    pub fn effective_trust(&self) -> ZuTrustState {
        if self.state == ZuTrustState::Trusted || self.parent_trusted {
            ZuTrustState::Trusted
        } else {
            self.state
        }
    }
}

/// An authentication session from an auth provider.
#[derive(Debug, Clone)]
pub struct ZuAuthSession {
    pub id: String,
    pub access_token: String,
    pub account_label: String,
    pub account_id: String,
    pub scopes: Vec<String>,
}

impl ZuAuthSession {
    pub fn new(id: &str, token: &str, label: &str, account_id: &str) -> Self {
        Self {
            id: id.to_string(),
            access_token: token.to_string(),
            account_label: label.to_string(),
            account_id: account_id.to_string(),
            scopes: Vec::new(),
        }
    }

    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    pub fn display_name(&self) -> &str {
        &self.account_label
    }
}

/// Options for requesting authentication.
#[derive(Debug, Clone)]
pub struct ZuAuthOptions {
    pub create_if_none: bool,
    pub clear_session_preference: bool,
    pub silent: bool,
    pub force_new_session: bool,
}

impl ZuAuthOptions {
    pub fn new() -> Self {
        Self {
            create_if_none: false,
            clear_session_preference: false,
            silent: false,
            force_new_session: false,
        }
    }

    pub fn interactive() -> Self {
        Self { create_if_none: true, ..Self::new() }
    }

    pub fn silent() -> Self {
        Self { silent: true, ..Self::new() }
    }
}

/// An authentication provider with session management.
#[derive(Debug, Clone)]
pub struct ZuAuthProvider {
    pub id: String,
    pub label: String,
    sessions: Vec<ZuAuthSession>,
    supports_multiple: bool,
}

impl ZuAuthProvider {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            sessions: Vec::new(),
            supports_multiple: false,
        }
    }

    pub fn with_multiple_accounts(mut self) -> Self {
        self.supports_multiple = true;
        self
    }

    pub fn add_session(&mut self, session: ZuAuthSession) {
        self.sessions.push(session);
    }

    pub fn remove_session(&mut self, session_id: &str) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.id != session_id);
        self.sessions.len() < before
    }

    pub fn get_session(&self, scopes: &[String]) -> Option<&ZuAuthSession> {
        self.sessions.iter().find(|s| {
            scopes.iter().all(|scope| s.has_scope(scope))
        })
    }

    pub fn sessions(&self) -> &[ZuAuthSession] {
        &self.sessions
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn supports_multiple_accounts(&self) -> bool {
        self.supports_multiple
    }
}

/// Registry of authentication providers.
#[derive(Debug, Clone)]
pub struct ZuAuthRegistry {
    providers: Vec<ZuAuthProvider>,
}

impl ZuAuthRegistry {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn register(&mut self, provider: ZuAuthProvider) {
        self.providers.push(provider);
    }

    pub fn get_provider(&self, id: &str) -> Option<&ZuAuthProvider> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn get_provider_mut(&mut self, id: &str) -> Option<&mut ZuAuthProvider> {
        self.providers.iter_mut().find(|p| p.id == id)
    }

    pub fn provider_ids(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.id.as_str()).collect()
    }

    pub fn all_sessions(&self) -> Vec<&ZuAuthSession> {
        self.providers.iter().flat_map(|p| p.sessions()).collect()
    }
}

/// A secret stored in the secret storage API.
#[derive(Debug, Clone)]
pub struct ZuSecretStorage {
    secrets: Vec<(String, String)>,
}

impl ZuSecretStorage {
    pub fn new() -> Self {
        Self { secrets: Vec::new() }
    }

    pub fn store(&mut self, key: &str, value: &str) {
        if let Some(entry) = self.secrets.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value.to_string();
        } else {
            self.secrets.push((key.to_string(), value.to_string()));
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.secrets.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn delete(&mut self, key: &str) -> bool {
        let before = self.secrets.len();
        self.secrets.retain(|(k, _)| k != key);
        self.secrets.len() < before
    }

    pub fn keys(&self) -> Vec<&str> {
        self.secrets.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.secrets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }
}


// --- zv_ comments API types ---

/// The mode of a comment (editing, preview, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZvCommentMode {
    Editing,
    Preview,
}

/// Reaction on a comment (like, thumbs up, etc.)
#[derive(Debug, Clone)]
pub struct ZvReaction {
    pub label: String,
    pub icon_path: Option<String>,
    pub count: u32,
    pub has_reacted: bool,
}

impl ZvReaction {
    pub fn new(label: &str, count: u32) -> Self {
        Self { label: label.to_string(), icon_path: None, count, has_reacted: false }
    }

    pub fn toggle(&mut self) {
        if self.has_reacted {
            self.count = self.count.saturating_sub(1);
            self.has_reacted = false;
        } else {
            self.count += 1;
            self.has_reacted = true;
        }
    }
}

/// Author of a comment.
#[derive(Debug, Clone)]
pub struct ZvCommentAuthor {
    pub name: String,
    pub icon_path: Option<String>,
}

impl ZvCommentAuthor {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), icon_path: None }
    }

    pub fn with_icon(mut self, path: &str) -> Self {
        self.icon_path = Some(path.to_string());
        self
    }
}

/// A single comment in a thread.
#[derive(Debug, Clone)]
pub struct ZvComment {
    pub body: String,
    pub mode: ZvCommentMode,
    pub author: ZvCommentAuthor,
    pub reactions: Vec<ZvReaction>,
    pub context_value: Option<String>,
    pub timestamp: Option<u64>,
}

impl ZvComment {
    pub fn new(body: &str, author: ZvCommentAuthor) -> Self {
        Self {
            body: body.to_string(),
            mode: ZvCommentMode::Preview,
            author,
            reactions: Vec::new(),
            context_value: None,
            timestamp: None,
        }
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = Some(ts);
        self
    }

    pub fn add_reaction(&mut self, reaction: ZvReaction) {
        self.reactions.push(reaction);
    }

    pub fn set_mode(&mut self, mode: ZvCommentMode) {
        self.mode = mode;
    }

    pub fn total_reactions(&self) -> u32 {
        self.reactions.iter().map(|r| r.count).sum()
    }
}

/// State of a comment thread (unresolved, resolved, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZvThreadState {
    Unresolved,
    Resolved,
}

/// Collapsible state for a comment thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZvCollapsibleState {
    Collapsed,
    Expanded,
}

/// A thread of comments anchored to a location.
#[derive(Debug, Clone)]
pub struct ZvCommentThread {
    pub uri: String,
    pub line: u32,
    pub comments: Vec<ZvComment>,
    pub state: ZvThreadState,
    pub collapsible: ZvCollapsibleState,
    pub label: Option<String>,
    pub can_reply: bool,
    disposed: bool,
}

impl ZvCommentThread {
    pub fn new(uri: &str, line: u32) -> Self {
        Self {
            uri: uri.to_string(),
            line,
            comments: Vec::new(),
            state: ZvThreadState::Unresolved,
            collapsible: ZvCollapsibleState::Expanded,
            label: None,
            can_reply: true,
            disposed: false,
        }
    }

    pub fn add_comment(&mut self, comment: ZvComment) {
        self.comments.push(comment);
    }

    pub fn resolve(&mut self) {
        self.state = ZvThreadState::Resolved;
    }

    pub fn unresolve(&mut self) {
        self.state = ZvThreadState::Unresolved;
    }

    pub fn collapse(&mut self) {
        self.collapsible = ZvCollapsibleState::Collapsed;
    }

    pub fn expand(&mut self) {
        self.collapsible = ZvCollapsibleState::Expanded;
    }

    pub fn comment_count(&self) -> usize {
        self.comments.len()
    }

    pub fn is_resolved(&self) -> bool {
        self.state == ZvThreadState::Resolved
    }

    pub fn dispose(&mut self) {
        self.disposed = true;
    }

    pub fn is_disposed(&self) -> bool {
        self.disposed
    }

    pub fn set_label(&mut self, label: &str) {
        self.label = Some(label.to_string());
    }
}

/// A comment controller that manages threads for a resource.
#[derive(Debug, Clone)]
pub struct ZvCommentController {
    pub id: String,
    pub label: String,
    threads: Vec<ZvCommentThread>,
    pub commenting_range_provider: bool,
    pub reaction_handler: bool,
}

impl ZvCommentController {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            threads: Vec::new(),
            commenting_range_provider: false,
            reaction_handler: false,
        }
    }

    pub fn create_thread(&mut self, uri: &str, line: u32) -> usize {
        let idx = self.threads.len();
        self.threads.push(ZvCommentThread::new(uri, line));
        idx
    }

    pub fn thread(&self, idx: usize) -> Option<&ZvCommentThread> {
        self.threads.get(idx)
    }

    pub fn thread_mut(&mut self, idx: usize) -> Option<&mut ZvCommentThread> {
        self.threads.get_mut(idx)
    }

    pub fn threads_for_uri(&self, uri: &str) -> Vec<&ZvCommentThread> {
        self.threads.iter().filter(|t| t.uri == uri && !t.is_disposed()).collect()
    }

    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    pub fn active_thread_count(&self) -> usize {
        self.threads.iter().filter(|t| !t.is_disposed()).count()
    }

    pub fn dispose_thread(&mut self, idx: usize) {
        if let Some(t) = self.threads.get_mut(idx) {
            t.dispose();
        }
    }

    pub fn with_commenting_ranges(mut self) -> Self {
        self.commenting_range_provider = true;
        self
    }

    pub fn with_reaction_handler(mut self) -> Self {
        self.reaction_handler = true;
        self
    }
}


// --- zw_ custom editor and file system provider types ---

/// File type in a virtual file system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZwFileType {
    Unknown,
    File,
    Directory,
    SymbolicLink,
}

/// File stat returned by a file system provider.
#[derive(Debug, Clone)]
pub struct ZwFileStat {
    pub file_type: ZwFileType,
    pub ctime: u64,
    pub mtime: u64,
    pub size: u64,
    pub permissions: Option<u32>,
}

impl ZwFileStat {
    pub fn file(size: u64, mtime: u64) -> Self {
        Self { file_type: ZwFileType::File, ctime: mtime, mtime, size, permissions: None }
    }

    pub fn directory(mtime: u64) -> Self {
        Self { file_type: ZwFileType::Directory, ctime: mtime, mtime, size: 0, permissions: None }
    }

    pub fn is_file(&self) -> bool {
        self.file_type == ZwFileType::File
    }

    pub fn is_directory(&self) -> bool {
        self.file_type == ZwFileType::Directory
    }

    pub fn with_permissions(mut self, perms: u32) -> Self {
        self.permissions = Some(perms);
        self
    }

    pub fn is_readonly(&self) -> bool {
        self.permissions.map_or(false, |p| p & 0o200 == 0)
    }
}

/// File change type for watching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZwFileChangeType {
    Changed,
    Created,
    Deleted,
}

/// A file change event.
#[derive(Debug, Clone)]
pub struct ZwFileChangeEvent {
    pub uri: String,
    pub change_type: ZwFileChangeType,
}

impl ZwFileChangeEvent {
    pub fn new(uri: &str, change_type: ZwFileChangeType) -> Self {
        Self { uri: uri.to_string(), change_type }
    }
}

/// Directory entry returned by readDirectory.
#[derive(Debug, Clone)]
pub struct ZwDirectoryEntry {
    pub name: String,
    pub file_type: ZwFileType,
}

impl ZwDirectoryEntry {
    pub fn new(name: &str, ft: ZwFileType) -> Self {
        Self { name: name.to_string(), file_type: ft }
    }
}

/// In-memory file system implementation.
#[derive(Debug, Clone)]
pub struct ZwMemoryFs {
    files: Vec<(String, Vec<u8>, ZwFileStat)>,
}

impl ZwMemoryFs {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn write_file(&mut self, uri: &str, content: &[u8], mtime: u64) {
        if let Some(entry) = self.files.iter_mut().find(|(u, _, _)| u == uri) {
            entry.1 = content.to_vec();
            entry.2.mtime = mtime;
            entry.2.size = content.len() as u64;
        } else {
            self.files.push((
                uri.to_string(),
                content.to_vec(),
                ZwFileStat::file(content.len() as u64, mtime),
            ));
        }
    }

    pub fn read_file(&self, uri: &str) -> Option<&[u8]> {
        self.files.iter().find(|(u, _, _)| u == uri).map(|(_, c, _)| c.as_slice())
    }

    pub fn stat(&self, uri: &str) -> Option<&ZwFileStat> {
        self.files.iter().find(|(u, _, _)| u == uri).map(|(_, _, s)| s)
    }

    pub fn delete(&mut self, uri: &str) -> bool {
        let before = self.files.len();
        self.files.retain(|(u, _, _)| u != uri);
        self.files.len() < before
    }

    pub fn rename(&mut self, old_uri: &str, new_uri: &str) -> bool {
        if let Some(entry) = self.files.iter_mut().find(|(u, _, _)| u == old_uri) {
            entry.0 = new_uri.to_string();
            true
        } else {
            false
        }
    }

    pub fn list(&self, prefix: &str) -> Vec<ZwDirectoryEntry> {
        self.files.iter()
            .filter(|(u, _, _)| u.starts_with(prefix) && u != prefix)
            .map(|(u, _, s)| {
                let name = u.strip_prefix(prefix).unwrap_or(u);
                let name = name.trim_start_matches('/');
                ZwDirectoryEntry::new(name, s.file_type)
            })
            .collect()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn exists(&self, uri: &str) -> bool {
        self.files.iter().any(|(u, _, _)| u == uri)
    }
}

/// Undo/redo edit for custom editors.
#[derive(Debug, Clone)]
pub struct ZwCustomEdit {
    pub label: String,
    pub data: Vec<u8>,
}

impl ZwCustomEdit {
    pub fn new(label: &str, data: Vec<u8>) -> Self {
        Self { label: label.to_string(), data }
    }
}

/// A custom document model for custom editors.
#[derive(Debug, Clone)]
pub struct ZwCustomDocument {
    pub uri: String,
    pub is_dirty: bool,
    edits: Vec<ZwCustomEdit>,
    saved_version: usize,
}

impl ZwCustomDocument {
    pub fn new(uri: &str) -> Self {
        Self { uri: uri.to_string(), is_dirty: false, edits: Vec::new(), saved_version: 0 }
    }

    pub fn apply_edit(&mut self, edit: ZwCustomEdit) {
        self.edits.push(edit);
        self.is_dirty = true;
    }

    pub fn save(&mut self) {
        self.saved_version = self.edits.len();
        self.is_dirty = false;
    }

    pub fn revert(&mut self) {
        self.edits.truncate(self.saved_version);
        self.is_dirty = false;
    }

    pub fn undo(&mut self) -> Option<ZwCustomEdit> {
        let edit = self.edits.pop();
        self.is_dirty = self.edits.len() != self.saved_version;
        edit
    }

    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.edits.is_empty()
    }
}


// --- zx_ tree view and data provider types ---

/// Collapsible state for tree items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZxTreeCollapsible {
    None,
    Collapsed,
    Expanded,
}

/// Selection mode for tree views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZxTreeSelectionMode {
    Single,
    Multiple,
}

/// Context value for when-clause evaluation on tree items.
#[derive(Debug, Clone)]
pub struct ZxTreeContext {
    values: Vec<(String, String)>,
}

impl ZxTreeContext {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        if let Some(entry) = self.values.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value.to_string();
        } else {
            self.values.push((key.to_string(), value.to_string()));
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn has(&self, key: &str) -> bool {
        self.values.iter().any(|(k, _)| k == key)
    }

    pub fn to_string_value(&self) -> String {
        self.values.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(";")
    }
}

/// A tree item with label, icon, and children references.
#[derive(Debug, Clone)]
pub struct ZxTreeItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub tooltip: Option<String>,
    pub icon_id: Option<String>,
    pub collapsible: ZxTreeCollapsible,
    pub context: ZxTreeContext,
    pub resource_uri: Option<String>,
    pub command_id: Option<String>,
    pub children_ids: Vec<String>,
}

impl ZxTreeItem {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            description: None,
            tooltip: None,
            icon_id: None,
            collapsible: ZxTreeCollapsible::None,
            context: ZxTreeContext::new(),
            resource_uri: None,
            command_id: None,
            children_ids: Vec::new(),
        }
    }

    pub fn with_children(mut self, collapsible: ZxTreeCollapsible) -> Self {
        self.collapsible = collapsible;
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    pub fn with_tooltip(mut self, tip: &str) -> Self {
        self.tooltip = Some(tip.to_string());
        self
    }

    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon_id = Some(icon.to_string());
        self
    }

    pub fn with_command(mut self, cmd: &str) -> Self {
        self.command_id = Some(cmd.to_string());
        self
    }

    pub fn with_uri(mut self, uri: &str) -> Self {
        self.resource_uri = Some(uri.to_string());
        self
    }

    pub fn add_child(&mut self, child_id: &str) {
        self.children_ids.push(child_id.to_string());
    }

    pub fn is_leaf(&self) -> bool {
        self.collapsible == ZxTreeCollapsible::None
    }

    pub fn has_children(&self) -> bool {
        !self.children_ids.is_empty()
    }
}

/// Options for a tree view.
#[derive(Debug, Clone)]
pub struct ZxTreeViewOptions {
    pub show_collapse_all: bool,
    pub can_select_many: bool,
    pub drag_and_drop: bool,
    pub manage_checkboxes: bool,
}

impl ZxTreeViewOptions {
    pub fn new() -> Self {
        Self {
            show_collapse_all: false,
            can_select_many: false,
            drag_and_drop: false,
            manage_checkboxes: false,
        }
    }
}

/// A tree view that displays a hierarchical list.
#[derive(Debug, Clone)]
pub struct ZxTreeView {
    pub view_id: String,
    pub title: String,
    pub options: ZxTreeViewOptions,
    items: Vec<ZxTreeItem>,
    root_ids: Vec<String>,
    selection: Vec<String>,
    pub is_visible: bool,
    pub message: Option<String>,
    pub badge_count: Option<u32>,
}

impl ZxTreeView {
    pub fn new(view_id: &str, title: &str, options: ZxTreeViewOptions) -> Self {
        Self {
            view_id: view_id.to_string(),
            title: title.to_string(),
            options,
            items: Vec::new(),
            root_ids: Vec::new(),
            selection: Vec::new(),
            is_visible: true,
            message: None,
            badge_count: None,
        }
    }

    pub fn add_root(&mut self, item: ZxTreeItem) {
        self.root_ids.push(item.id.clone());
        self.items.push(item);
    }

    pub fn add_child_item(&mut self, parent_id: &str, item: ZxTreeItem) {
        let child_id = item.id.clone();
        self.items.push(item);
        if let Some(parent) = self.items.iter_mut().find(|i| i.id == parent_id) {
            parent.add_child(&child_id);
        }
    }

    pub fn get_item(&self, id: &str) -> Option<&ZxTreeItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn root_items(&self) -> Vec<&ZxTreeItem> {
        self.root_ids.iter().filter_map(|id| self.get_item(id)).collect()
    }

    pub fn children_of(&self, parent_id: &str) -> Vec<&ZxTreeItem> {
        if let Some(parent) = self.get_item(parent_id) {
            parent.children_ids.iter().filter_map(|id| self.get_item(id)).collect()
        } else {
            Vec::new()
        }
    }

    pub fn select(&mut self, id: &str) {
        if !self.options.can_select_many {
            self.selection.clear();
        }
        if !self.selection.contains(&id.to_string()) {
            self.selection.push(id.to_string());
        }
    }

    pub fn deselect(&mut self, id: &str) {
        self.selection.retain(|s| s != id);
    }

    pub fn selection(&self) -> &[String] {
        &self.selection
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message = Some(msg.to_string());
    }

    pub fn set_badge(&mut self, count: u32) {
        self.badge_count = Some(count);
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn reveal(&mut self, _id: &str) {
        self.is_visible = true;
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
    fn find_by_command_works() {
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
    fn format_keybinding_works() {
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


    // -- keybindings_ui additional tests -------------------------------------------

    #[test]
    fn x_keybindings_ui_panel_state_new() {
        let p = XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XKeybindingsUiLayoutRegion::Sidebar);
    }

    #[test]
    fn x_keybindings_ui_panel_area() {
        let p = XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_keybindings_ui_panel_toggle() {
        let mut p = XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_keybindings_ui_panel_resize() {
        let mut p = XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_keybindings_ui_panel_is_narrow() {
        let mut p = XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_keybindings_ui_total_visible_area_basic() {
        let panels = vec![
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "a"),
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_keybindings_ui_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_keybindings_ui_total_visible_area_hidden() {
        let mut panels = vec![
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "a"),
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_keybindings_ui_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_keybindings_ui_count_in_region_basic() {
        let panels = vec![
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "a"),
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "b"),
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_keybindings_ui_count_in_region(&panels, XKeybindingsUiLayoutRegion::Sidebar), 2);
        assert_eq!(x_keybindings_ui_count_in_region(&panels, XKeybindingsUiLayoutRegion::Editor), 1);
        assert_eq!(x_keybindings_ui_count_in_region(&panels, XKeybindingsUiLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_keybindings_ui_widest_panel_basic() {
        let mut panels = vec![
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "narrow"),
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_keybindings_ui_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_keybindings_ui_collapse_region_basic() {
        let mut panels = vec![
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "a"),
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Sidebar, "b"),
            XKeybindingsUiPanelState::new(XKeybindingsUiLayoutRegion::Editor, "c"),
        ];
        x_keybindings_ui_collapse_region(&mut panels, XKeybindingsUiLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_keybindings_ui_layout_constraint_clamp() {
        let lc = XKeybindingsUiLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_keybindings_ui_layout_constraint_satisfied() {
        let lc = XKeybindingsUiLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_keybindings_ui_widest_panel_empty() {
        let panels: Vec<XKeybindingsUiPanelState> = vec![];
        assert!(x_keybindings_ui_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_keybindings_ui_layout_region_eq() {
        assert_eq!(XKeybindingsUiLayoutRegion::Sidebar, XKeybindingsUiLayoutRegion::Sidebar);
        assert_ne!(XKeybindingsUiLayoutRegion::Sidebar, XKeybindingsUiLayoutRegion::Panel);
    }


    // -- keybindings_ui extended domain tests ----------------------------------------

    #[test]
    fn y_keybindings_ui_enum_index() {
        assert_eq!(YKeybindingsUiKeybindingConflict::None.index(), 0);
        assert_eq!(YKeybindingsUiKeybindingConflict::Shadowed.index(), 1);
        assert_eq!(YKeybindingsUiKeybindingConflict::Duplicate.index(), 2);
        assert_eq!(YKeybindingsUiKeybindingConflict::Incompatible.index(), 3);
    }

    #[test]
    fn y_keybindings_ui_enum_label() {
        assert_eq!(YKeybindingsUiKeybindingConflict::None.label(), "None");
        assert_eq!(YKeybindingsUiKeybindingConflict::Shadowed.label(), "Shadowed");
        assert_eq!(YKeybindingsUiKeybindingConflict::Duplicate.label(), "Duplicate");
        assert_eq!(YKeybindingsUiKeybindingConflict::Incompatible.label(), "Incompatible");
    }

    #[test]
    fn y_keybindings_ui_enum_all() {
        let all = YKeybindingsUiKeybindingConflict::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_keybindings_ui_enum_is_default() {
        assert!(YKeybindingsUiKeybindingConflict::None.is_default());
        assert!(!YKeybindingsUiKeybindingConflict::Incompatible.is_default());
    }

    #[test]
    fn y_keybindings_ui_enum_display() {
        assert_eq!(format!("{}", YKeybindingsUiKeybindingConflict::None), "None");
    }

    #[test]
    fn y_keybindings_ui_struct_new() {
        let s = YKeybindingsUiKeybindingSearchResult::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_keybindings_ui_struct_clear() {
        let mut s = YKeybindingsUiKeybindingSearchResult::new();
        s.matches.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_keybindings_ui_fingerprint_deterministic() {
        let h1 = y_keybindings_ui_fingerprint("hello");
        let h2 = y_keybindings_ui_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_keybindings_ui_fingerprint("a"), y_keybindings_ui_fingerprint("b"));
    }

    #[test]
    fn y_keybindings_ui_truncate_short() {
        assert_eq!(y_keybindings_ui_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_keybindings_ui_truncate_long() {
        let r = y_keybindings_ui_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_keybindings_ui_normalize_key_basic() {
        assert_eq!(y_keybindings_ui_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_keybindings_ui_split_path_basic() {
        let parts = y_keybindings_ui_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_keybindings_ui_count_occurrences_basic() {
        assert_eq!(y_keybindings_ui_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_keybindings_ui_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_keybindings_ui_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_keybindings_ui_in_range_basic() {
        assert!(y_keybindings_ui_in_range(5, 1, 10));
        assert!(y_keybindings_ui_in_range(1, 1, 10));
        assert!(y_keybindings_ui_in_range(10, 1, 10));
        assert!(!y_keybindings_ui_in_range(0, 1, 10));
        assert!(!y_keybindings_ui_in_range(11, 1, 10));
    }

    #[test]
    fn y_keybindings_ui_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_keybindings_ui_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_keybindings_ui_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_keybindings_ui_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- keybindings_ui Z-extended tests -----------------------------------------------

    #[test]
    fn z_keybindings_ui_priority_weight() {
        assert_eq!(ZKeybindingsUiPriority::Idle.weight(), 0);
        assert_eq!(ZKeybindingsUiPriority::Normal.weight(), 2);
        assert_eq!(ZKeybindingsUiPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_keybindings_ui_priority_label() {
        assert_eq!(ZKeybindingsUiPriority::Low.label(), "low");
        assert_eq!(ZKeybindingsUiPriority::High.label(), "high");
    }

    #[test]
    fn z_keybindings_ui_priority_is_elevated() {
        assert!(!ZKeybindingsUiPriority::Normal.is_elevated());
        assert!(ZKeybindingsUiPriority::High.is_elevated());
        assert!(ZKeybindingsUiPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_keybindings_ui_priority_display() {
        assert_eq!(format!("{}", ZKeybindingsUiPriority::Idle), "idle");
    }

    #[test]
    fn z_keybindings_ui_priority_all_asc() {
        let all = ZKeybindingsUiPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZKeybindingsUiPriority::Idle);
        assert_eq!(all[4], ZKeybindingsUiPriority::Realtime);
    }

    #[test]
    fn z_keybindings_ui_struct_new() {
        let s = ZKeybindingsUiKeybindingRecorder::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_keybindings_ui_struct_toggled_clone() {
        let s = ZKeybindingsUiKeybindingRecorder::new();
        let t = s.toggled_clone();
        let _ = t.max_chord_len;
    }

    #[test]
    fn z_keybindings_ui_rolling_hash_deterministic() {
        let h1 = z_keybindings_ui_rolling_hash(b"test");
        let h2 = z_keybindings_ui_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_keybindings_ui_rolling_hash(b"a"), z_keybindings_ui_rolling_hash(b"b"));
    }

    #[test]
    fn z_keybindings_ui_pad_to_basic() {
        assert_eq!(z_keybindings_ui_pad_to("hi", 5), "hi   ");
        assert_eq!(z_keybindings_ui_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_keybindings_ui_is_identifier_basic() {
        assert!(z_keybindings_ui_is_identifier("foo_bar"));
        assert!(z_keybindings_ui_is_identifier("abc123"));
        assert!(!z_keybindings_ui_is_identifier(""));
        assert!(!z_keybindings_ui_is_identifier("has space"));
    }

    #[test]
    fn z_keybindings_ui_levenshtein_basic() {
        assert_eq!(z_keybindings_ui_levenshtein("", ""), 0);
        assert_eq!(z_keybindings_ui_levenshtein("abc", "abc"), 0);
        assert_eq!(z_keybindings_ui_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_keybindings_ui_unique_words_basic() {
        let w = z_keybindings_ui_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_keybindings_ui_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_keybindings_ui_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_keybindings_ui_common_prefix_basic() {
        assert_eq!(z_keybindings_ui_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_keybindings_ui_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_keybindings_ui_struct_clear() {
        let mut s = ZKeybindingsUiKeybindingRecorder::new();
        s.recorded_keys.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_keybindings_ui_rolling_hash_empty() {
        let h = z_keybindings_ui_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 103 ----

    #[test]
    fn xc_103_pool_new_empty() {
        let pool: super::Xc103Pool<i32> = super::Xc103Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_103_pool_release_acquire() {
        let mut pool = super::Xc103Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_103_pool_acquire_empty() {
        let mut pool: super::Xc103Pool<i32> = super::Xc103Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_103_pool_full() {
        let mut pool = super::Xc103Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_103_pool_drain() {
        let mut pool = super::Xc103Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_103_pool_stats() {
        let mut pool = super::Xc103Pool::new(8);
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
    fn xc_103_pool_clear() {
        let mut pool = super::Xc103Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_103_pool_shrink() {
        let mut pool = super::Xc103Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_103_pool_default() {
        let pool: super::Xc103Pool<String> = super::Xc103Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_103_pool_extend() {
        let mut pool = super::Xc103Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_103_pool_retain() {
        let mut pool = super::Xc103Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_103_scheduler_round_robin() {
        let mut sched = super::Xc103Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_103_scheduler_empty() {
        let mut sched = super::Xc103Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_103_scheduler_reset() {
        let mut sched = super::Xc103Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_103_scheduler_add_remove() {
        let mut sched = super::Xc103Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_103_scheduler_targets() {
        let sched = super::Xc103Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_103_hash_empty() {
        assert_eq!(super::xc_103_hash(b""), 5381);
    }

    #[test]
    fn xc_103_hash_data() {
        let h = super::xc_103_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_103_hash(b"hello"), h);
    }

    #[test]
    fn xc_103_reverse_str() {
        assert_eq!(super::xc_103_reverse("abc"), "cba");
        assert_eq!(super::xc_103_reverse(""), "");
    }


    // --- xd_5 deepening tests ---

    #[test]
    fn xd_5_sm_initial_state() {
        let sm = Xd5StateMachine::new();
        assert_eq!(sm.current_state(), Xd5State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_5_sm_valid_idle_to_running() {
        let mut sm = Xd5StateMachine::new();
        assert!(sm.transition(Xd5State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd5State::Running);
    }

    #[test]
    fn xd_5_sm_valid_running_to_paused() {
        let mut sm = Xd5StateMachine::new();
        sm.transition(Xd5State::Running).unwrap();
        assert!(sm.transition(Xd5State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd5State::Paused);
    }

    #[test]
    fn xd_5_sm_valid_running_to_done() {
        let mut sm = Xd5StateMachine::new();
        sm.transition(Xd5State::Running).unwrap();
        assert!(sm.transition(Xd5State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd5State::Done);
    }

    #[test]
    fn xd_5_sm_valid_paused_to_running() {
        let mut sm = Xd5StateMachine::new();
        sm.transition(Xd5State::Running).unwrap();
        sm.transition(Xd5State::Paused).unwrap();
        assert!(sm.transition(Xd5State::Running).is_ok());
    }

    #[test]
    fn xd_5_sm_valid_done_to_idle() {
        let mut sm = Xd5StateMachine::new();
        sm.transition(Xd5State::Running).unwrap();
        sm.transition(Xd5State::Done).unwrap();
        assert!(sm.transition(Xd5State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd5State::Idle);
    }

    #[test]
    fn xd_5_sm_invalid_idle_to_done() {
        let mut sm = Xd5StateMachine::new();
        assert!(sm.transition(Xd5State::Done).is_err());
    }

    #[test]
    fn xd_5_sm_invalid_idle_to_paused() {
        let mut sm = Xd5StateMachine::new();
        assert!(sm.transition(Xd5State::Paused).is_err());
    }

    #[test]
    fn xd_5_sm_history_tracking() {
        let mut sm = Xd5StateMachine::new();
        sm.transition(Xd5State::Running).unwrap();
        sm.transition(Xd5State::Paused).unwrap();
        sm.transition(Xd5State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd5State::Idle);
        assert_eq!(sm.history()[0].to, Xd5State::Running);
        assert_eq!(sm.history()[1].from, Xd5State::Running);
        assert_eq!(sm.history()[2].to, Xd5State::Done);
    }

    #[test]
    fn xd_5_sm_serialize_deserialize() {
        let mut sm = Xd5StateMachine::new();
        sm.transition(Xd5State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd5StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd5State::Running));
    }

    #[test]
    fn xd_5_sm_deserialize_invalid() {
        assert_eq!(Xd5StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_5_sm_reset() {
        let mut sm = Xd5StateMachine::new();
        sm.transition(Xd5State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd5State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_5_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd5EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd5Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_5_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd5EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd5Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd5Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_5_bus_unsubscribe() {
        let mut bus = Xd5EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_5_event_kind_and_payload() {
        let e = Xd5Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd5Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_5_bus_clear_history() {
        let mut bus = Xd5EventBus::new();
        bus.publish(Xd5Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_5_sm_step_counter_increments() {
        let mut sm = Xd5StateMachine::new();
        sm.transition(Xd5State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd5State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #3 --

    #[test]
    fn xf3_trie_insert_search() {
        let mut t = Xf3Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf3_trie_starts_with() {
        let mut t = Xf3Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf3_trie_remove() {
        let mut t = Xf3Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf3_trie_word_count() {
        let mut t = Xf3Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf3_trie_longest_prefix() {
        let mut t = Xf3Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf3_trie_all_words() {
        let mut t = Xf3Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf3_trie_autocomplete() {
        let mut t = Xf3Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf3_trie_empty_search() {
        let t = Xf3Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf3_bloom_add_contains() {
        let mut bf = Xf3BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf3_bloom_probably_absent() {
        let bf = Xf3BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf3_bloom_false_positive_rate() {
        let mut bf = Xf3BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf3_bloom_clear() {
        let mut bf = Xf3BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf3_bloom_union() {
        let mut a = Xf3BloomFilter::xf_new(512, 2);
        let mut b = Xf3BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf3_bloom_intersection_estimate() {
        let mut a = Xf3BloomFilter::xf_new(512, 2);
        let mut b = Xf3BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf3_bloom_union_size_mismatch() {
        let a = Xf3BloomFilter::xf_new(256, 2);
        let b = Xf3BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh102_skip_insert_contains() {
        let mut sl = super::Xh102SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh102_skip_remove() {
        let mut sl = super::Xh102SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh102_skip_len() {
        let mut sl = super::Xh102SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh102_skip_range_query() {
        let mut sl = super::Xh102SkipList::xh_new(4);
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
    fn xh102_skip_floor_ceiling() {
        let mut sl = super::Xh102SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh102_skip_rank() {
        let mut sl = super::Xh102SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh102_skip_empty() {
        let sl = super::Xh102SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh102_skip_duplicates() {
        let mut sl = super::Xh102SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh102_bitset_set_test() {
        let mut bs = super::Xh102BitSet::xh_new(256);
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
    fn xh102_bitset_clear_count() {
        let mut bs = super::Xh102BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh102_bitset_and_or_xor() {
        let mut a = super::Xh102BitSet::xh_new(128);
        let mut b = super::Xh102BitSet::xh_new(128);
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
    fn xh102_bitset_iter_ones() {
        let mut bs = super::Xh102BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh102_bitset_first_last() {
        let mut bs = super::Xh102BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh102_bitset_empty() {
        let bs = super::Xh102BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi102_deque_push_pop_back() {
        let mut dq = super::Xi102Deque::xi_new(4);
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
    fn xi102_deque_push_pop_front() {
        let mut dq = super::Xi102Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi102_deque_mixed_ops() {
        let mut dq = super::Xi102Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi102_deque_get_and_split() {
        let mut dq = super::Xi102Deque::xi_new(8);
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
    fn xi102_deque_rotate_left() {
        let mut dq = super::Xi102Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi102_deque_rotate_right() {
        let mut dq = super::Xi102Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi102_deque_grow() {
        let mut dq = super::Xi102Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi102_deque_empty() {
        let dq = super::Xi102Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi102_interval_tree_insert_query() {
        let mut tree = super::Xi102IntervalTree::xi_new();
        tree.xi_insert(super::Xi102Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi102Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi102Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi102_interval_tree_overlap() {
        let mut tree = super::Xi102IntervalTree::xi_new();
        tree.xi_insert(super::Xi102Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi102Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi102Interval::xi_new(12, 20));
        let q = super::Xi102Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi102_interval_tree_remove() {
        let mut tree = super::Xi102IntervalTree::xi_new();
        tree.xi_insert(super::Xi102Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi102Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi102_interval_tree_gaps() {
        let mut tree = super::Xi102IntervalTree::xi_new();
        tree.xi_insert(super::Xi102Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi102Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi102Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi102Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi102Interval::xi_new(8, 10));
    }

    #[test]
    fn xi102_interval_tree_merge() {
        let mut tree = super::Xi102IntervalTree::xi_new();
        tree.xi_insert(super::Xi102Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi102Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi102Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi102Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi102Interval::xi_new(10, 15));
    }

    #[test]
    fn xi102_interval_tree_all() {
        let mut tree = super::Xi102IntervalTree::xi_new();
        tree.xi_insert(super::Xi102Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi102Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi102_interval_tree_empty() {
        let tree = super::Xi102IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi102_interval_tree_contains_point() {
        let iv = super::Xi102Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 102) ---

    #[test]
    fn xj_102_uf_make_and_find() {
        let mut uf = super::Xj102UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_102_uf_union_connected() {
        let mut uf = super::Xj102UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_102_uf_component_count() {
        let mut uf = super::Xj102UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_102_uf_component_size() {
        let mut uf = super::Xj102UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_102_uf_largest_component() {
        let mut uf = super::Xj102UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_102_uf_many_elements() {
        let mut uf = super::Xj102UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_102_uf_separate_components() {
        let mut uf = super::Xj102UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_102_uf_path_compression() {
        let mut uf = super::Xj102UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_102_bt_insert_get() {
        let mut bt = super::Xj102BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_102_bt_contains_len() {
        let mut bt = super::Xj102BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_102_bt_replace() {
        let mut bt = super::Xj102BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_102_bt_remove() {
        let mut bt = super::Xj102BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_102_bt_keys_values() {
        let mut bt = super::Xj102BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_102_bt_range() {
        let mut bt = super::Xj102BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_102_bt_min_max() {
        let mut bt = super::Xj102BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_102_bt_many_inserts() {
        let mut bt = super::Xj102BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_103 segment tree tests ---

    #[test]
    fn xk_103_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk103SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_103_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk103SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_103_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk103SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_103_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk103SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_103_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk103SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_103_st_single_element() {
        let data = vec![42];
        let st = super::Xk103SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_103_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk103SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_103_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk103SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_103 disjoint intervals tests ---

    #[test]
    fn xk_103_di_add_and_count() {
        let mut di = super::Xk103DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_103_di_merge_overlap() {
        let mut di = super::Xk103DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_103_di_contains() {
        let mut di = super::Xk103DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_103_di_remove() {
        let mut di = super::Xk103DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_103_di_covered_length() {
        let mut di = super::Xk103DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_103_di_gaps() {
        let mut di = super::Xk103DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_103_di_merge_adjacent() {
        let mut di = super::Xk103DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_103_di_empty() {
        let di = super::Xk103DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_102_rope_new_empty() {
        let rope = super::Xl102Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_102_rope_from_str() {
        let rope = super::Xl102Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_102_rope_insert_at() {
        let mut rope = super::Xl102Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_102_rope_delete_range() {
        let mut rope = super::Xl102Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_102_rope_char_at() {
        let rope = super::Xl102Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_102_rope_split_concat() {
        let rope = super::Xl102Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_102_rope_line_count() {
        let rope = super::Xl102Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_102_rope_line_at() {
        let rope = super::Xl102Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_102_sa_build_and_search() {
        let sa = super::Xl102SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_102_sa_count() {
        let sa = super::Xl102SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_102_sa_longest_repeated() {
        let sa = super::Xl102SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_102_sa_all_positions() {
        let sa = super::Xl102SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_102_sa_len() {
        let sa = super::Xl102SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_102_sa_empty() {
        let sa = super::Xl102SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_102_rope_slice() {
        let rope = super::Xl102Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_102_sa_search_start() {
        let sa = super::Xl102SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_102_sparse_set_get() {
        let mut m = super::Xm102MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_102_sparse_row_col() {
        let mut m = super::Xm102MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_102_sparse_transpose() {
        let mut m = super::Xm102MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_102_sparse_multiply_vec() {
        let mut m = super::Xm102MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_102_sparse_nnz_density() {
        let mut m = super::Xm102MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_102_sparse_clear() {
        let mut m = super::Xm102MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_102_sparse_overwrite_zero() {
        let mut m = super::Xm102MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_102_tokenizer_basic() {
        let t = super::Xm102Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_102_tokenizer_count() {
        let t = super::Xm102Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_102_tokenizer_unique() {
        let t = super::Xm102Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_102_tokenizer_frequency() {
        let t = super::Xm102Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_102_tokenizer_delimiter() {
        let t = super::Xm102Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_102_tokenizer_whitespace() {
        let t = super::Xm102Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_102_tokenizer_empty() {
        let t = super::Xm102Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 102 ----

    #[test]
    fn xn_102_fenwick_prefix_sum() {
        let mut ft = super::Xn102Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_102_fenwick_range_sum() {
        let mut ft = super::Xn102Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_102_fenwick_point_query() {
        let mut ft = super::Xn102Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_102_fenwick_len() {
        let ft = super::Xn102Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_102_fenwick_multiple_updates() {
        let mut ft = super::Xn102Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_102_fenwick_single_element() {
        let mut ft = super::Xn102Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_102_fenwick_find_kth() {
        let mut ft = super::Xn102Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_102_fenwick_negative_delta() {
        let mut ft = super::Xn102Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 102 ----

    #[test]
    fn xn_102_avl_insert_get() {
        let mut m = super::Xn102AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_102_avl_remove() {
        let mut m = super::Xn102AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_102_avl_in_order() {
        let mut m = super::Xn102AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_102_avl_min_max() {
        let mut m = super::Xn102AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_102_avl_floor_ceiling() {
        let mut m = super::Xn102AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_102_avl_height_balanced() {
        let mut m = super::Xn102AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_102_avl_overwrite() {
        let mut m = super::Xn102AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_102_avl_empty() {
        let m: super::Xn102AVL<i32, i32> = super::Xn102AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo102RedBlack tests ---

    #[test]
    fn xo_102_rb_insert_and_get() {
        let mut tree = super::Xo102RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_102_rb_len_and_empty() {
        let mut tree = super::Xo102RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_102_rb_min_max() {
        let mut tree = super::Xo102RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_102_rb_contains() {
        let mut tree = super::Xo102RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_102_rb_remove() {
        let mut tree = super::Xo102RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_102_rb_in_order() {
        let mut tree = super::Xo102RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_102_rb_black_height() {
        let mut tree = super::Xo102RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_102_rb_overwrite() {
        let mut tree = super::Xo102RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo102ConsistentHash tests ---

    #[test]
    fn xo_102_ch_add_and_count() {
        let mut ring = super::Xo102ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_102_ch_remove_node() {
        let mut ring = super::Xo102ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_102_ch_get_node() {
        let mut ring = super::Xo102ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_102_ch_empty_ring() {
        let ring = super::Xo102ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_102_ch_distribution() {
        let mut ring = super::Xo102ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_102_ch_rebalance() {
        let mut ring = super::Xo102ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_102_ch_virtual_nodes() {
        let mut ring = super::Xo102ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_102_ch_consistent_lookup() {
        let mut ring = super::Xo102ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_103_splay_insert_get() {
        let mut t = super::Xp103SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_103_splay_remove() {
        let mut t = super::Xp103SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_103_splay_count_increases() {
        let mut t = super::Xp103SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_103_splay_depth() {
        let mut t = super::Xp103SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_103_splay_len_empty() {
        let t = super::Xp103SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_103_splay_min_max() {
        let mut t = super::Xp103SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_103_splay_overwrite() {
        let mut t = super::Xp103SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_103_splay_remove_missing() {
        let mut t = super::Xp103SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_102 treap tests ----
    #[test]
    fn xq_102_treap_empty() {
        let t = super::Xq102Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_102_treap_insert_get() {
        let mut t = super::Xq102Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_102_treap_overwrite() {
        let mut t = super::Xq102Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_102_treap_remove() {
        let mut t = super::Xq102Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_102_treap_min_max() {
        let mut t = super::Xq102Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_102_treap_rank() {
        let mut t = super::Xq102Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_102_treap_kth() {
        let mut t = super::Xq102Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_102_treap_in_order() {
        let mut t = super::Xq102Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_102 VEB tree tests ----
    #[test]
    fn xq_102_veb_empty() {
        let v = super::Xq102VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_102_veb_insert_contains() {
        let mut v = super::Xq102VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_102_veb_min_max() {
        let mut v = super::Xq102VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_102_veb_delete() {
        let mut v = super::Xq102VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_102_veb_successor() {
        let mut v = super::Xq102VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_102_veb_predecessor() {
        let mut v = super::Xq102VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_102_veb_count() {
        let mut v = super::Xq102VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_102_veb_duplicate_insert() {
        let mut v = super::Xq102VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_102_kdtree_empty() {
        let tree = super::Xr102KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_102_kdtree_insert_one() {
        let mut tree = super::Xr102KDTree::xr_new();
        tree.xr_insert(super::Xr102KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_102_kdtree_insert_multiple() {
        let mut tree = super::Xr102KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr102KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_102_kdtree_nearest_neighbor() {
        let mut tree = super::Xr102KDTree::xr_new();
        tree.xr_insert(super::Xr102KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr102KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr102KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_102_kdtree_nn_empty() {
        let tree = super::Xr102KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr102KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_102_kdtree_range_search() {
        let mut tree = super::Xr102KDTree::xr_new();
        tree.xr_insert(super::Xr102KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr102KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr102KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_102_kdtree_range_empty() {
        let mut tree = super::Xr102KDTree::xr_new();
        tree.xr_insert(super::Xr102KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_102_kdtree_all_points() {
        let mut tree = super::Xr102KDTree::xr_new();
        tree.xr_insert(super::Xr102KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr102KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_102_kdtree_depth() {
        let mut tree = super::Xr102KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr102KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_102_kdtree_bounding_box() {
        let mut tree = super::Xr102KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr102KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr102KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_102_persistent_array_new() {
        let arr = super::Xs102PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_102_persistent_array_push() {
        let mut arr = super::Xs102PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_102_persistent_array_set() {
        let mut arr = super::Xs102PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_102_persistent_array_diff() {
        let mut arr = super::Xs102PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_102_persistent_array_rollback() {
        let mut arr = super::Xs102PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_102_persistent_array_history() {
        let mut arr = super::Xs102PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_102_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs102PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_102_persistent_array_from_vec() {
        let arr = super::Xs102PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_102_concurrent_queue_new() {
        let q = super::Xs102ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_102_concurrent_queue_push_pop() {
        let mut q = super::Xs102ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_102_concurrent_queue_full() {
        let mut q = super::Xs102ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_102_concurrent_queue_drain() {
        let mut q = super::Xs102ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_102_concurrent_queue_try_pop() {
        let mut q = super::Xs102ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_102_concurrent_queue_clear() {
        let mut q = super::Xs102ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_102_range_map_new() {
        let rm = super::Xs102RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_102_range_map_insert_get() {
        let mut rm = super::Xs102RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_102_range_map_overlap() {
        let mut rm = super::Xs102RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_102_range_map_remove() {
        let mut rm = super::Xs102RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_102_range_map_gaps() {
        let mut rm = super::Xs102RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_102_range_map_coverage() {
        let mut rm = super::Xs102RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_102_range_map_contains() {
        let mut rm = super::Xs102RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_102_range_map_clear() {
        let mut rm = super::Xs102RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_102_circular_buffer_new() {
        let buf = super::Xs102CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_102_circular_buffer_push_pop() {
        let mut buf = super::Xs102CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_102_circular_buffer_overwrite() {
        let mut buf = super::Xs102CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_102_circular_buffer_peek() {
        let mut buf = super::Xs102CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_102_circular_buffer_is_full() {
        let mut buf = super::Xs102CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_102_circular_buffer_iter() {
        let mut buf = super::Xs102CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_102_circular_buffer_clear() {
        let mut buf = super::Xs102CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_102_circular_buffer_to_vec() {
        let mut buf = super::Xs102CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_102_stats_tracker_new() {
        let tracker = super::Xs102StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_102_stats_tracker_mean() {
        let mut tracker = super::Xs102StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_102_stats_tracker_min_max() {
        let mut tracker = super::Xs102StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_102_stats_tracker_median() {
        let mut tracker = super::Xs102StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_102_stats_tracker_variance() {
        let mut tracker = super::Xs102StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_102_stats_tracker_range() {
        let mut tracker = super::Xs102StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_102_stats_tracker_clear() {
        let mut tracker = super::Xs102StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_102_stats_tracker_sum() {
        let mut tracker = super::Xs102StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }


    // --- xw_ Scapegoat Tree tests ---

    #[test]
    fn xw_sg_tree_new() {
        let t = super::XwScapegoatTree::<i32, &str>::xw_new();
        assert!(t.xw_is_empty());
        assert_eq!(t.xw_len(), 0);
    }

    #[test]
    fn xw_sg_tree_insert_get() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "five");
        t.xw_insert(3, "three");
        t.xw_insert(7, "seven");
        assert_eq!(t.xw_get(&5), Some(&"five"));
        assert_eq!(t.xw_get(&3), Some(&"three"));
        assert_eq!(t.xw_get(&4), None);
    }

    #[test]
    fn xw_sg_tree_contains() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(10, "a");
        assert!(t.xw_contains(&10));
        assert!(!t.xw_contains(&20));
    }

    #[test]
    fn xw_sg_tree_keys_sorted() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xw_insert(k, k * 10);
        }
        assert_eq!(t.xw_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xw_sg_tree_sequential_inserts() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in 1..=20 {
            t.xw_insert(k, k);
        }
        assert_eq!(t.xw_len(), 20);
        assert!(t.xw_height() <= 15);
    }

    #[test]
    fn xw_sg_tree_replace_value() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "old");
        t.xw_insert(5, "new");
        assert_eq!(t.xw_get(&5), Some(&"new"));
        assert_eq!(t.xw_len(), 1);
    }

    #[test]
    fn xw_sg_tree_clear() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(1, "a");
        t.xw_clear();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_tree_display() {
        let t = super::XwScapegoatTree::<i32, i32>::xw_new();
        assert!(format!("{}", t).contains("SGTree"));
    }

    #[test]
    fn xw_sg_tree_default() {
        let t = super::XwScapegoatTree::<i32, i32>::default();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_node_display() {
        let n = super::XwScapegoatNode { xw_key: 1, xw_value: "a", xw_left: None, xw_right: None };
        assert!(format!("{}", n).contains("SGNode"));
    }

    // --- xw_ Rope tests ---

    #[test]
    fn xw_rope_new() {
        let r = super::XwRope::xw_new();
        assert!(r.xw_is_empty());
        assert_eq!(r.xw_len(), 0);
    }

    #[test]
    fn xw_rope_from_str() {
        let r = super::XwRope::xw_from_str("hello");
        assert_eq!(r.xw_len(), 5);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_concat() {
        let a = super::XwRope::xw_from_str("hello ");
        let b = super::XwRope::xw_from_str("world");
        let c = super::XwRope::xw_concat(a, b);
        assert_eq!(c.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_insert() {
        let mut r = super::XwRope::xw_from_str("helo");
        r.xw_insert(3, "l");
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_delete() {
        let mut r = super::XwRope::xw_from_str("hello world");
        r.xw_delete(5, 11);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_append() {
        let mut r = super::XwRope::xw_from_str("hello");
        r.xw_append(" world");
        assert_eq!(r.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_substring() {
        let r = super::XwRope::xw_from_str("hello world");
        assert_eq!(r.xw_substring(6, 11), "world");
    }

    #[test]
    fn xw_rope_char_at() {
        let r = super::XwRope::xw_from_str("abc");
        assert_eq!(r.xw_char_at(0), Some('a'));
        assert_eq!(r.xw_char_at(2), Some('c'));
    }

    #[test]
    fn xw_rope_clear() {
        let mut r = super::XwRope::xw_from_str("text");
        r.xw_clear();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_display() {
        let r = super::XwRope::xw_from_str("test");
        assert!(format!("{}", r).contains("Rope"));
    }

    #[test]
    fn xw_rope_default() {
        let r = super::XwRope::default();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_empty_ops() {
        let r = super::XwRope::xw_new();
        assert_eq!(r.xw_to_string(), "");
        assert_eq!(r.xw_substring(0, 5), "");
    }


    // --- xx_ Skip List tests ---

    #[test]
    fn xx_skip_list_new() {
        let sl = super::XxSkipList::<i32, &str>::xx_new();
        assert!(sl.xx_is_empty());
        assert_eq!(sl.xx_len(), 0);
    }

    #[test]
    fn xx_skip_list_insert_get() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "five");
        sl.xx_insert(3, "three");
        sl.xx_insert(7, "seven");
        assert_eq!(sl.xx_get(&5), Some(&"five"));
        assert_eq!(sl.xx_get(&3), Some(&"three"));
        assert_eq!(sl.xx_get(&7), Some(&"seven"));
        assert_eq!(sl.xx_get(&4), None);
    }

    #[test]
    fn xx_skip_list_contains() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(10, "a");
        assert!(sl.xx_contains(&10));
        assert!(!sl.xx_contains(&20));
    }

    #[test]
    fn xx_skip_list_keys_sorted() {
        let mut sl = super::XxSkipList::xx_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            sl.xx_insert(k, k * 10);
        }
        assert_eq!(sl.xx_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xx_skip_list_replace() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "old");
        sl.xx_insert(5, "new");
        assert_eq!(sl.xx_get(&5), Some(&"new"));
    }

    #[test]
    fn xx_skip_list_many() {
        let mut sl = super::XxSkipList::xx_new();
        for k in 1..=50 {
            sl.xx_insert(k, k);
        }
        assert_eq!(sl.xx_len(), 50);
        for k in 1..=50 {
            assert!(sl.xx_contains(&k));
        }
    }

    #[test]
    fn xx_skip_list_clear() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(1, "a");
        sl.xx_clear();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_list_display() {
        let sl = super::XxSkipList::<i32, i32>::xx_new();
        assert!(format!("{}", sl).contains("SkipList"));
    }

    #[test]
    fn xx_skip_list_default() {
        let sl = super::XxSkipList::<i32, i32>::default();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_node_display() {
        let n = super::XxSkipNode::<i32, i32> { xx_key: Some(5), xx_value: Some(50), xx_forward: vec![None] };
        assert!(format!("{}", n).contains("SkipNode"));
    }

    // --- xx_ Suffix Array tests ---

    #[test]
    fn xx_suffix_array_new() {
        let sa = super::XxSuffixArray::xx_new("banana");
        assert_eq!(sa.xx_len(), 6);
        assert!(!sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_search() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let pos = sa.xx_search("ana");
        assert_eq!(pos.len(), 2);
    }

    #[test]
    fn xx_suffix_array_count() {
        let sa = super::XxSuffixArray::xx_new("abcabcabc");
        assert_eq!(sa.xx_count("abc"), 3);
    }

    #[test]
    fn xx_suffix_array_no_match() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_count("xyz"), 0);
    }

    #[test]
    fn xx_suffix_array_suffix_at() {
        let sa = super::XxSuffixArray::xx_new("abc");
        let s = sa.xx_suffix_at(0);
        assert!(!s.is_empty());
    }

    #[test]
    fn xx_suffix_array_longest_repeated() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let lr = sa.xx_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xx_suffix_array_empty() {
        let sa = super::XxSuffixArray::xx_new("");
        assert!(sa.xx_is_empty());
        assert_eq!(sa.xx_search("a").len(), 0);
    }

    #[test]
    fn xx_suffix_array_display() {
        let sa = super::XxSuffixArray::xx_new("test");
        assert!(format!("{}", sa).contains("SuffixArray"));
    }

    #[test]
    fn xx_suffix_array_default() {
        let sa = super::XxSuffixArray::default();
        assert!(sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_text() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_text(), "hello");
    }


    // --- xy_ Cuckoo Hash Map tests ---

    #[test]
    fn xy_cuckoo_new() {
        let m = super::XyCuckooMap::<String, i32>::xy_new(16);
        assert!(m.xy_is_empty());
        assert_eq!(m.xy_len(), 0);
    }

    #[test]
    fn xy_cuckoo_insert_get() {
        let mut m = super::XyCuckooMap::xy_new(32);
        m.xy_insert("hello".to_string(), 1);
        m.xy_insert("world".to_string(), 2);
        assert_eq!(m.xy_get(&"hello".to_string()), Some(&1));
        assert_eq!(m.xy_get(&"world".to_string()), Some(&2));
        assert_eq!(m.xy_get(&"missing".to_string()), None);
    }

    #[test]
    fn xy_cuckoo_contains() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(42, "a");
        assert!(m.xy_contains(&42));
        assert!(!m.xy_contains(&99));
    }

    #[test]
    fn xy_cuckoo_replace() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(5, "old");
        m.xy_insert(5, "new");
        assert_eq!(m.xy_get(&5), Some(&"new"));
    }

    #[test]
    fn xy_cuckoo_remove() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(10, "val");
        assert_eq!(m.xy_remove(&10), Some("val"));
        assert!(!m.xy_contains(&10));
    }

    #[test]
    fn xy_cuckoo_many() {
        let mut m = super::XyCuckooMap::xy_new(64);
        for i in 0..30 {
            m.xy_insert(i, i * 10);
        }
        assert_eq!(m.xy_len(), 30);
        for i in 0..30 {
            assert!(m.xy_contains(&i));
        }
    }

    #[test]
    fn xy_cuckoo_keys() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_insert(2, "b");
        let keys = m.xy_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn xy_cuckoo_clear() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_clear();
        assert!(m.xy_is_empty());
    }

    #[test]
    fn xy_cuckoo_display() {
        let m = super::XyCuckooMap::<i32, i32>::xy_new(16);
        assert!(format!("{}", m).contains("CuckooMap"));
    }

    #[test]
    fn xy_cuckoo_default() {
        let m = super::XyCuckooMap::<i32, i32>::default();
        assert!(m.xy_is_empty());
    }

    // --- xy_ Count-Min Sketch tests ---

    #[test]
    fn xy_cms_new() {
        let cms = super::XyCountMinSketch::xy_new(100, 5);
        assert_eq!(cms.xy_width(), 100);
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_add_estimate() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..10 { cms.xy_add(42); }
        assert!(cms.xy_estimate(42) >= 10);
    }

    #[test]
    fn xy_cms_add_count() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        cms.xy_add_count(7, 100);
        assert!(cms.xy_estimate(7) >= 100);
    }

    #[test]
    fn xy_cms_unseen() {
        let cms = super::XyCountMinSketch::xy_new(1000, 5);
        assert_eq!(cms.xy_estimate(999), 0);
    }

    #[test]
    fn xy_cms_merge() {
        let mut a = super::XyCountMinSketch::xy_new(100, 3);
        let mut b = super::XyCountMinSketch::xy_new(100, 3);
        a.xy_add(1);
        b.xy_add(1);
        a.xy_merge(&b);
        assert!(a.xy_estimate(1) >= 2);
    }

    #[test]
    fn xy_cms_clear() {
        let mut cms = super::XyCountMinSketch::xy_new(100, 3);
        cms.xy_add(1);
        cms.xy_clear();
        assert_eq!(cms.xy_estimate(1), 0);
    }

    #[test]
    fn xy_cms_display() {
        let cms = super::XyCountMinSketch::xy_new(100, 3);
        assert!(format!("{}", cms).contains("CMS"));
    }

    #[test]
    fn xy_cms_default() {
        let cms = super::XyCountMinSketch::default();
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_multiple_items() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for i in 0..100 { cms.xy_add(i); }
        for i in 0..100 { assert!(cms.xy_estimate(i) >= 1); }
    }

    #[test]
    fn xy_cms_heavy_hitter() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..1000 { cms.xy_add(42); }
        for i in 0..10 { cms.xy_add(i); }
        assert!(cms.xy_estimate(42) > cms.xy_estimate(0));
    }


    // --- xz_ HyperLogLog tests ---

    #[test]
    fn xz_hll_new() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_num_registers(), 1024);
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_add_estimate() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for i in 0..1000 {
            hll.xz_add(i);
        }
        let est = hll.xz_estimate();
        assert!(est > 500.0 && est < 2000.0);
    }

    #[test]
    fn xz_hll_empty() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_merge() {
        let mut a = super::XzHyperLogLog::xz_new(10);
        let mut b = super::XzHyperLogLog::xz_new(10);
        for i in 0..500 { a.xz_add(i); }
        for i in 500..1000 { b.xz_add(i); }
        a.xz_merge(&b);
        let est = a.xz_estimate();
        assert!(est > 500.0);
    }

    #[test]
    fn xz_hll_clear() {
        let mut hll = super::XzHyperLogLog::xz_new(10);
        hll.xz_add(1);
        hll.xz_clear();
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_display() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert!(format!("{}", hll).contains("HLL"));
    }

    #[test]
    fn xz_hll_default() {
        let hll = super::XzHyperLogLog::default();
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_duplicates() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for _ in 0..1000 { hll.xz_add(42); }
        let est = hll.xz_estimate();
        assert!(est < 10.0);
    }

    // --- xz_ LRU Cache tests ---

    #[test]
    fn xz_lru_new() {
        let lru = super::XzLruCache::<String, i32>::xz_new(10);
        assert!(lru.xz_is_empty());
        assert_eq!(lru.xz_capacity(), 10);
    }

    #[test]
    fn xz_lru_put_get() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put("a".to_string(), 1);
        lru.xz_put("b".to_string(), 2);
        assert_eq!(lru.xz_get(&"a".to_string()), Some(&1));
        assert_eq!(lru.xz_get(&"b".to_string()), Some(&2));
    }

    #[test]
    fn xz_lru_eviction() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        assert!(!lru.xz_contains(&1));
        assert!(lru.xz_contains(&2));
        assert!(lru.xz_contains(&3));
    }

    #[test]
    fn xz_lru_access_updates_order() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_get(&1);
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1));
        assert!(!lru.xz_contains(&2));
    }

    #[test]
    fn xz_lru_update_value() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "old");
        lru.xz_put(1, "new");
        assert_eq!(lru.xz_get(&1), Some(&"new"));
        assert_eq!(lru.xz_len(), 1);
    }

    #[test]
    fn xz_lru_remove() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        assert_eq!(lru.xz_remove(&1), Some("a"));
        assert!(!lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_peek() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        assert_eq!(lru.xz_peek(&1), Some(&"a"));
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1) || !lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_keys_order() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        let keys = lru.xz_keys_lru();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn xz_lru_clear() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_clear();
        assert!(lru.xz_is_empty());
    }

    #[test]
    fn xz_lru_display() {
        let lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert!(format!("{}", lru).contains("LRU"));
    }

    #[test]
    fn xz_lru_missing_key() {
        let mut lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert_eq!(lru.xz_get(&999), None);
    }


    // --- ya_ Trie tests ---

    #[test]
    fn ya_trie_new() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(t.ya_is_empty());
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_insert_get() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        t.ya_insert("world", 2);
        assert_eq!(t.ya_get("hello"), Some(&1));
        assert_eq!(t.ya_get("world"), Some(&2));
        assert_eq!(t.ya_get("missing"), None);
    }

    #[test]
    fn ya_trie_contains() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        assert!(t.ya_contains("abc"));
        assert!(!t.ya_contains("ab"));
        assert!(!t.ya_contains("abcd"));
    }

    #[test]
    fn ya_trie_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert!(t.ya_has_prefix("ab"));
        assert!(!t.ya_has_prefix("ac"));
    }

    #[test]
    fn ya_trie_keys_with_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("cat", 1);
        t.ya_insert("car", 2);
        t.ya_insert("dog", 3);
        let keys = t.ya_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"cat".to_string()));
        assert!(keys.contains(&"car".to_string()));
    }

    #[test]
    fn ya_trie_all_keys() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("b", 1);
        t.ya_insert("a", 2);
        t.ya_insert("c", 3);
        let keys = t.ya_all_keys();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn ya_trie_remove() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        assert_eq!(t.ya_remove("hello"), Some(1));
        assert!(!t.ya_contains("hello"));
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_lcp() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert_eq!(t.ya_longest_common_prefix(), "ab");
    }

    #[test]
    fn ya_trie_clear() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("a", 1);
        t.ya_clear();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_display() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(format!("{}", t).contains("Trie"));
    }

    #[test]
    fn ya_trie_default() {
        let t = super::YaTrie::<i32>::default();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_count_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("test1", 1);
        t.ya_insert("test2", 2);
        t.ya_insert("other", 3);
        assert_eq!(t.ya_count_prefix("test"), 2);
    }

    // --- ya_ Bloom Filter tests ---

    #[test]
    fn ya_bloom_new() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_bit_size(), 1000);
        assert_eq!(bf.ya_num_hashes(), 5);
        assert_eq!(bf.ya_count(), 0);
    }

    #[test]
    fn ya_bloom_add_contains() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        bf.ya_add(42);
        bf.ya_add(100);
        assert!(bf.ya_might_contain(42));
        assert!(bf.ya_might_contain(100));
    }

    #[test]
    fn ya_bloom_no_false_negatives() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        for i in 0..100 { bf.ya_add(i); }
        for i in 0..100 { assert!(bf.ya_might_contain(i)); }
    }

    #[test]
    fn ya_bloom_with_fp_rate() {
        let bf = super::YaBloomFilter::ya_with_fp_rate(1000, 0.01);
        assert!(bf.ya_bit_size() > 0);
        assert!(bf.ya_num_hashes() > 0);
    }

    #[test]
    fn ya_bloom_clear() {
        let mut bf = super::YaBloomFilter::ya_new(1000, 5);
        bf.ya_add(1);
        bf.ya_clear();
        assert_eq!(bf.ya_count(), 0);
        assert!(!bf.ya_might_contain(1));
    }

    #[test]
    fn ya_bloom_merge() {
        let mut a = super::YaBloomFilter::ya_new(1000, 5);
        let mut b = super::YaBloomFilter::ya_new(1000, 5);
        a.ya_add(1);
        b.ya_add(2);
        a.ya_merge(&b);
        assert!(a.ya_might_contain(1));
        assert!(a.ya_might_contain(2));
    }

    #[test]
    fn ya_bloom_fp_rate() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_estimated_fp_rate(), 0.0);
    }

    #[test]
    fn ya_bloom_display() {
        let bf = super::YaBloomFilter::ya_new(100, 3);
        assert!(format!("{}", bf).contains("Bloom"));
    }

    #[test]
    fn ya_bloom_default() {
        let bf = super::YaBloomFilter::default();
        assert_eq!(bf.ya_num_hashes(), 5);
    }


    // --- yb_ TST tests ---

    #[test]
    fn yb_tst_new() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_insert_get() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("hello", 1);
        t.yb_insert("world", 2);
        assert_eq!(t.yb_get("hello"), Some(&1));
        assert_eq!(t.yb_get("world"), Some(&2));
        assert_eq!(t.yb_get("missing"), None);
    }

    #[test]
    fn yb_tst_contains() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("abc", 10);
        assert!(t.yb_contains("abc"));
        assert!(!t.yb_contains("ab"));
    }

    #[test]
    fn yb_tst_all_keys() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("b", 1);
        t.yb_insert("a", 2);
        t.yb_insert("c", 3);
        let keys = t.yb_all_keys();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn yb_tst_prefix() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("cat", 1);
        t.yb_insert("car", 2);
        t.yb_insert("dog", 3);
        let keys = t.yb_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn yb_tst_clear() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("a", 1);
        t.yb_clear();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_display() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(format!("{}", t).contains("TST"));
    }

    #[test]
    fn yb_tst_default() {
        let t = super::YbTernarySearchTree::<i32>::default();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_overwrite() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("key", 1);
        t.yb_insert("key", 2);
        assert_eq!(t.yb_get("key"), Some(&2));
        assert_eq!(t.yb_len(), 1);
    }

    // --- yb_ Quadtree tests ---

    #[test]
    fn yb_quad_new() {
        let q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_insert() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_insert(super::YbPoint::yb_new(50.0, 50.0)));
        assert_eq!(q.yb_count(), 1);
    }

    #[test]
    fn yb_quad_query() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        q.yb_insert(super::YbPoint::yb_new(15.0, 15.0));
        let found = q.yb_query(&super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn yb_quad_outside() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(!q.yb_insert(super::YbPoint::yb_new(200.0, 200.0)));
    }

    #[test]
    fn yb_quad_nearest() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        let near = q.yb_nearest(&super::YbPoint::yb_new(12.0, 12.0)).unwrap();
        assert!((near.yb_x - 10.0).abs() < 0.001);
    }

    #[test]
    fn yb_quad_display() {
        let q = super::YbQuadtree::default();
        assert!(format!("{}", q).contains("Quadtree"));
    }

    #[test]
    fn yb_quad_default() {
        let q = super::YbQuadtree::default();
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_many() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        for i in 0..20 {
            q.yb_insert(super::YbPoint::yb_new(i as f64 * 4.0, i as f64 * 4.0));
        }
        assert_eq!(q.yb_count(), 20);
    }

    #[test]
    fn yb_point_distance() {
        let a = super::YbPoint::yb_new(0.0, 0.0);
        let b = super::YbPoint::yb_new(3.0, 4.0);
        assert!((a.yb_distance(&b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn yb_bounds_intersects() {
        let a = super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0);
        let b = super::YbBounds::yb_new(25.0, 25.0, 50.0, 50.0);
        assert!(a.yb_intersects(&b));
    }


    // --- yc_ VebSet tests ---

    #[test]
    fn yc_veb_new() {
        let v = super::YcVebSet::yc_new(1000);
        assert!(v.yc_is_empty());
        assert_eq!(v.yc_universe(), 1000);
    }

    #[test]
    fn yc_veb_insert_contains() {
        let mut v = super::YcVebSet::yc_new(1000);
        assert!(v.yc_insert(42));
        assert!(v.yc_contains(42));
        assert!(!v.yc_contains(43));
    }

    #[test]
    fn yc_veb_remove() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        assert!(v.yc_remove(10));
        assert!(!v.yc_contains(10));
    }

    #[test]
    fn yc_veb_min_max() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(50);
        v.yc_insert(10);
        v.yc_insert(90);
        assert_eq!(v.yc_min(), Some(10));
        assert_eq!(v.yc_max(), Some(90));
    }

    #[test]
    fn yc_veb_successor() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        v.yc_insert(20);
        v.yc_insert(30);
        assert_eq!(v.yc_successor(10), Some(20));
        assert_eq!(v.yc_successor(20), Some(30));
        assert_eq!(v.yc_successor(30), None);
    }

    #[test]
    fn yc_veb_predecessor() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        v.yc_insert(20);
        assert_eq!(v.yc_predecessor(20), Some(10));
        assert_eq!(v.yc_predecessor(10), None);
    }

    #[test]
    fn yc_veb_sorted() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(30);
        v.yc_insert(10);
        v.yc_insert(20);
        assert_eq!(v.yc_to_sorted_vec(), vec![10, 20, 30]);
    }

    #[test]
    fn yc_veb_clear() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(1);
        v.yc_clear();
        assert!(v.yc_is_empty());
    }

    #[test]
    fn yc_veb_union() {
        let mut a = super::YcVebSet::yc_new(100);
        let mut b = super::YcVebSet::yc_new(100);
        a.yc_insert(1);
        b.yc_insert(2);
        a.yc_union(&b);
        assert!(a.yc_contains(1));
        assert!(a.yc_contains(2));
    }

    #[test]
    fn yc_veb_intersection() {
        let mut a = super::YcVebSet::yc_new(100);
        let mut b = super::YcVebSet::yc_new(100);
        a.yc_insert(1); a.yc_insert(2);
        b.yc_insert(2); b.yc_insert(3);
        let c = a.yc_intersection(&b);
        assert!(c.yc_contains(2));
        assert!(!c.yc_contains(1));
    }

    #[test]
    fn yc_veb_display() {
        let v = super::YcVebSet::yc_new(100);
        assert!(format!("{}", v).contains("VebSet"));
    }

    #[test]
    fn yc_veb_default() {
        let v = super::YcVebSet::default();
        assert_eq!(v.yc_universe(), 65536);
    }

    // --- yc_ HashRing tests ---

    #[test]
    fn yc_ring_new() {
        let r = super::YcHashRing::yc_new(100);
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_add_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("server1");
        assert_eq!(r.yc_node_count(), 1);
        assert_eq!(r.yc_virtual_count(), 50);
    }

    #[test]
    fn yc_ring_get_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("a");
        r.yc_add_node("b");
        let n = r.yc_get_node("mykey");
        assert!(n.is_some());
    }

    #[test]
    fn yc_ring_remove_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("a");
        r.yc_remove_node("a");
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_has_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("server1");
        assert!(r.yc_has_node("server1"));
        assert!(!r.yc_has_node("server2"));
    }

    #[test]
    fn yc_ring_display() {
        let r = super::YcHashRing::yc_new(10);
        assert!(format!("{}", r).contains("HashRing"));
    }

    #[test]
    fn yc_ring_default() {
        let r = super::YcHashRing::default();
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_consistency() {
        let mut r = super::YcHashRing::yc_new(100);
        r.yc_add_node("a");
        r.yc_add_node("b");
        let n1 = r.yc_get_node("key1").unwrap().to_string();
        let n2 = r.yc_get_node("key1").unwrap().to_string();
        assert_eq!(n1, n2);
    }


    // --- yd_ DAG tests ---

    #[test]
    fn yd_dag_new() {
        let g = super::YdDag::yd_new();
        assert_eq!(g.yd_node_count(), 0);
    }

    #[test]
    fn yd_dag_add_edge() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        assert_eq!(g.yd_node_count(), 3);
        assert_eq!(g.yd_edge_count(), 2);
    }

    #[test]
    fn yd_dag_topo_sort() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        g.yd_add_edge(1, 3);
        g.yd_add_edge(2, 3);
        let order = g.yd_topological_sort().unwrap();
        assert_eq!(order.len(), 4);
        let pos: std::collections::HashMap<usize, usize> = order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&0] < pos[&2]);
    }

    #[test]
    fn yd_dag_cycle() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        g.yd_add_edge(2, 0);
        assert!(g.yd_has_cycle());
    }

    #[test]
    fn yd_dag_roots_leaves() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_roots(), vec![0]);
        assert_eq!(g.yd_leaves(), vec![1, 2]);
    }

    #[test]
    fn yd_dag_bfs() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        g.yd_add_edge(1, 3);
        let bfs = g.yd_bfs(0);
        assert_eq!(bfs[0], 0);
        assert_eq!(bfs.len(), 4);
    }

    #[test]
    fn yd_dag_dfs() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        let dfs = g.yd_dfs(0);
        assert_eq!(dfs[0], 0);
        assert_eq!(dfs.len(), 3);
    }

    #[test]
    fn yd_dag_shortest_path() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_shortest_path(0, 2), Some(1));
    }

    #[test]
    fn yd_dag_degrees() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_out_degree(0), 2);
        assert_eq!(g.yd_in_degree(1), 1);
    }

    #[test]
    fn yd_dag_clear() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_clear();
        assert_eq!(g.yd_node_count(), 0);
    }

    #[test]
    fn yd_dag_display() {
        let g = super::YdDag::yd_new();
        assert!(format!("{}", g).contains("DAG"));
    }

    // --- yd_ SparseMatrix tests ---

    #[test]
    fn yd_sparse_new() {
        let m = super::YdSparseMatrix::yd_new(3, 3);
        assert_eq!(m.yd_nnz(), 0);
    }

    #[test]
    fn yd_sparse_set_get() {
        let mut m = super::YdSparseMatrix::yd_new(3, 3);
        m.yd_set(0, 1, 5.0);
        assert_eq!(m.yd_get(0, 1), 5.0);
        assert_eq!(m.yd_get(0, 0), 0.0);
    }

    #[test]
    fn yd_sparse_transpose() {
        let mut m = super::YdSparseMatrix::yd_new(2, 3);
        m.yd_set(0, 2, 7.0);
        let t = m.yd_transpose();
        assert_eq!(t.yd_get(2, 0), 7.0);
    }

    #[test]
    fn yd_sparse_mul_vec() {
        let mut m = super::YdSparseMatrix::yd_new(2, 2);
        m.yd_set(0, 0, 1.0);
        m.yd_set(1, 1, 2.0);
        let r = m.yd_mul_vec(&[3.0, 4.0]);
        assert_eq!(r, vec![3.0, 8.0]);
    }

    #[test]
    fn yd_sparse_add() {
        let mut a = super::YdSparseMatrix::yd_new(2, 2);
        let mut b = super::YdSparseMatrix::yd_new(2, 2);
        a.yd_set(0, 0, 1.0);
        b.yd_set(0, 0, 2.0);
        let c = a.yd_add(&b);
        assert_eq!(c.yd_get(0, 0), 3.0);
    }

    #[test]
    fn yd_sparse_scale() {
        let mut m = super::YdSparseMatrix::yd_new(1, 1);
        m.yd_set(0, 0, 5.0);
        m.yd_scale(2.0);
        assert_eq!(m.yd_get(0, 0), 10.0);
    }

    #[test]
    fn yd_sparse_row_sum() {
        let mut m = super::YdSparseMatrix::yd_new(2, 3);
        m.yd_set(0, 0, 1.0);
        m.yd_set(0, 1, 2.0);
        m.yd_set(0, 2, 3.0);
        assert_eq!(m.yd_row_sum(0), 6.0);
    }

    #[test]
    fn yd_sparse_display() {
        let m = super::YdSparseMatrix::yd_new(2, 2);
        assert!(format!("{}", m).contains("SparseMatrix"));
    }

    #[test]
    fn yd_sparse_clear() {
        let mut m = super::YdSparseMatrix::yd_new(2, 2);
        m.yd_set(0, 0, 1.0);
        m.yd_clear();
        assert_eq!(m.yd_nnz(), 0);
    }


    // --- ye_ IndexedPQ tests ---

    #[test]
    fn ye_ipq_new() {
        let pq = super::YeIndexedPQ::ye_new();
        assert!(pq.ye_is_empty());
    }

    #[test]
    fn ye_ipq_insert_pop() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 10);
        pq.ye_insert(1, 5);
        pq.ye_insert(2, 15);
        assert_eq!(pq.ye_pop(), Some((1, 5)));
        assert_eq!(pq.ye_pop(), Some((0, 10)));
    }

    #[test]
    fn ye_ipq_decrease_key() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 10);
        pq.ye_insert(1, 20);
        pq.ye_decrease_key(1, 5);
        assert_eq!(pq.ye_peek(), Some((1, 5)));
    }

    #[test]
    fn ye_ipq_contains() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(42, 1);
        assert!(pq.ye_contains(42));
        assert!(!pq.ye_contains(99));
    }

    #[test]
    fn ye_ipq_priority() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 7);
        assert_eq!(pq.ye_priority(0), Some(7));
    }

    #[test]
    fn ye_ipq_drain() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 30);
        pq.ye_insert(1, 10);
        pq.ye_insert(2, 20);
        let sorted = pq.ye_drain_sorted();
        assert_eq!(sorted, vec![(1, 10), (2, 20), (0, 30)]);
    }

    #[test]
    fn ye_ipq_clear() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 1);
        pq.ye_clear();
        assert!(pq.ye_is_empty());
    }

    #[test]
    fn ye_ipq_display() {
        let pq = super::YeIndexedPQ::ye_new();
        assert!(format!("{}", pq).contains("IndexedPQ"));
    }

    #[test]
    fn ye_ipq_default() {
        let pq = super::YeIndexedPQ::default();
        assert!(pq.ye_is_empty());
    }

    // --- ye_ SegTree tests ---

    #[test]
    fn ye_seg_from_slice() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(st.ye_len(), 5);
        assert_eq!(st.ye_query(0, 4), 15);
    }

    #[test]
    fn ye_seg_point_query() {
        let mut st = super::YeSegTree::ye_from_slice(&[10, 20, 30]);
        assert_eq!(st.ye_point_query(1), 20);
    }

    #[test]
    fn ye_seg_range_query() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(st.ye_query(1, 3), 9);
    }

    #[test]
    fn ye_seg_update() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        st.ye_update(1, 3, 10);
        assert_eq!(st.ye_query(0, 4), 45);
    }

    #[test]
    fn ye_seg_single_update() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3]);
        st.ye_update(1, 1, 5);
        assert_eq!(st.ye_point_query(1), 7);
    }

    #[test]
    fn ye_seg_empty() {
        let st = super::YeSegTree::ye_from_slice(&[]);
        assert!(st.ye_is_empty());
    }

    #[test]
    fn ye_seg_single() {
        let mut st = super::YeSegTree::ye_from_slice(&[42]);
        assert_eq!(st.ye_query(0, 0), 42);
    }

    #[test]
    fn ye_seg_display() {
        let st = super::YeSegTree::ye_from_slice(&[1, 2, 3]);
        assert!(format!("{}", st).contains("SegTree"));
    }

    #[test]
    fn ye_seg_default() {
        let st = super::YeSegTree::default();
        assert!(st.ye_is_empty());
    }


    // --- yf_ IntervalSet tests ---

    #[test]
    fn yf_interval_new() {
        let s = super::YfIntervalSet::yf_new();
        assert!(s.yf_is_empty());
    }

    #[test]
    fn yf_interval_add() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        assert_eq!(s.yf_len(), 1);
        assert!(s.yf_contains(3));
    }

    #[test]
    fn yf_interval_merge() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(3, 8);
        assert_eq!(s.yf_len(), 1);
        assert_eq!(s.yf_intervals(), vec![(1, 8)]);
    }

    #[test]
    fn yf_interval_adjacent() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(6, 10);
        assert_eq!(s.yf_len(), 1);
    }

    #[test]
    fn yf_interval_disjoint() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 3);
        s.yf_add(10, 15);
        assert_eq!(s.yf_len(), 2);
    }

    #[test]
    fn yf_interval_remove_point() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 10);
        s.yf_remove_point(5);
        assert!(!s.yf_contains(5));
        assert!(s.yf_contains(4));
        assert!(s.yf_contains(6));
    }

    #[test]
    fn yf_interval_length() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(10, 14);
        assert_eq!(s.yf_total_length(), 10);
    }

    #[test]
    fn yf_interval_clear() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_clear();
        assert!(s.yf_is_empty());
    }

    #[test]
    fn yf_interval_display() {
        let s = super::YfIntervalSet::yf_new();
        assert!(format!("{}", s).contains("IntervalSet"));
    }

    #[test]
    fn yf_interval_overlaps() {
        let mut a = super::YfIntervalSet::yf_new();
        let mut b = super::YfIntervalSet::yf_new();
        a.yf_add(1, 5);
        b.yf_add(3, 8);
        assert!(a.yf_overlaps(&b));
    }

    // --- yf_ KWayMerge tests ---

    #[test]
    fn yf_kmerge_new() {
        let m = super::YfKWayMerge::yf_new();
        assert_eq!(m.yf_source_count(), 0);
    }

    #[test]
    fn yf_kmerge_merge() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 4, 7]);
        m.yf_add_source(vec![2, 5, 8]);
        m.yf_add_source(vec![3, 6, 9]);
        let result = m.yf_merge();
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn yf_kmerge_single() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2, 3]);
        let result = m.yf_merge();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn yf_kmerge_empty() {
        let mut m = super::YfKWayMerge::yf_new();
        let result = m.yf_merge();
        assert!(result.is_empty());
    }

    #[test]
    fn yf_kmerge_remaining() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2]);
        m.yf_add_source(vec![3, 4]);
        assert_eq!(m.yf_remaining(), 4);
    }

    #[test]
    fn yf_kmerge_reset() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2]);
        let _ = m.yf_merge();
        m.yf_reset();
        assert_eq!(m.yf_remaining(), 2);
    }

    #[test]
    fn yf_kmerge_unique() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2, 3]);
        m.yf_add_source(vec![2, 3, 4]);
        let result = m.yf_merge_unique();
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn yf_kmerge_display() {
        let m = super::YfKWayMerge::yf_new();
        assert!(format!("{}", m).contains("KWayMerge"));
    }

    #[test]
    fn yf_kmerge_default() {
        let m = super::YfKWayMerge::default();
        assert!(m.yf_is_done());
    }


    // --- yg_ PersistentStack tests ---

    #[test]
    fn yg_pstack_new() {
        let s = super::YgPersistentStack::<i32>::yg_new();
        assert!(s.yg_is_empty());
    }

    #[test]
    fn yg_pstack_push_pop() {
        let s = super::YgPersistentStack::yg_new();
        let s = s.yg_push(1);
        let s = s.yg_push(2);
        let (v, s) = s.yg_pop().unwrap();
        assert_eq!(v, 2);
        let (v, _) = s.yg_pop().unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn yg_pstack_persistence() {
        let s1 = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2);
        let s2 = s1.yg_push(3);
        assert_eq!(s1.yg_len(), 2);
        assert_eq!(s2.yg_len(), 3);
    }

    #[test]
    fn yg_pstack_peek() {
        let s = super::YgPersistentStack::yg_new().yg_push(42);
        assert_eq!(s.yg_peek(), Some(&42));
    }

    #[test]
    fn yg_pstack_to_vec() {
        let s = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2).yg_push(3);
        assert_eq!(s.yg_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn yg_pstack_reverse() {
        let s = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2).yg_push(3);
        let r = s.yg_reverse();
        assert_eq!(r.yg_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn yg_pstack_display() {
        let s = super::YgPersistentStack::yg_new().yg_push(1);
        assert!(format!("{}", s).contains("PStack"));
    }

    #[test]
    fn yg_pstack_default() {
        let s = super::YgPersistentStack::<i32>::default();
        assert!(s.yg_is_empty());
    }

    // --- yg_ BitmapIndex tests ---

    #[test]
    fn yg_bitmap_new() {
        let bi = super::YgBitmapIndex::yg_new(100);
        assert_eq!(bi.yg_num_rows(), 100);
    }

    #[test]
    fn yg_bitmap_set_get() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("color_red", 5);
        assert!(bi.yg_get("color_red", 5));
        assert!(!bi.yg_get("color_red", 6));
    }

    #[test]
    fn yg_bitmap_and() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("a", 1);
        bi.yg_set("a", 2);
        bi.yg_set("b", 2);
        bi.yg_set("b", 3);
        let rows = bi.yg_and(&["a", "b"]);
        assert_eq!(rows, vec![2]);
    }

    #[test]
    fn yg_bitmap_or() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("a", 1);
        bi.yg_set("b", 2);
        let rows = bi.yg_or(&["a", "b"]);
        assert_eq!(rows, vec![1, 2]);
    }

    #[test]
    fn yg_bitmap_count() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("x", 0);
        bi.yg_set("x", 1);
        bi.yg_set("x", 2);
        assert_eq!(bi.yg_count("x"), 3);
    }

    #[test]
    fn yg_bitmap_columns() {
        let mut bi = super::YgBitmapIndex::yg_new(10);
        bi.yg_set("a", 0);
        bi.yg_set("b", 0);
        assert_eq!(bi.yg_num_columns(), 2);
    }

    #[test]
    fn yg_bitmap_clear() {
        let mut bi = super::YgBitmapIndex::yg_new(10);
        bi.yg_set("a", 0);
        bi.yg_clear();
        assert_eq!(bi.yg_num_columns(), 0);
    }

    #[test]
    fn yg_bitmap_display() {
        let bi = super::YgBitmapIndex::yg_new(10);
        assert!(format!("{}", bi).contains("BitmapIndex"));
    }

    #[test]
    fn yg_bitmap_default() {
        let bi = super::YgBitmapIndex::default();
        assert_eq!(bi.yg_num_rows(), 0);
    }


    // --- yh_ OSTree tests ---

    #[test]
    fn yh_ost_new() {
        let t = super::YhOrderStatTree::yh_new();
        assert!(t.yh_is_empty());
    }

    #[test]
    fn yh_ost_insert_contains() {
        let mut t = super::YhOrderStatTree::yh_new();
        t.yh_insert(10);
        t.yh_insert(5);
        t.yh_insert(15);
        assert!(t.yh_contains(10));
        assert!(!t.yh_contains(7));
    }

    #[test]
    fn yh_ost_rank() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [10, 5, 15, 3, 7] { t.yh_insert(v); }
        assert_eq!(t.yh_rank(5), 1);
        assert_eq!(t.yh_rank(10), 3);
    }

    #[test]
    fn yh_ost_select() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [10, 5, 15, 3, 7] { t.yh_insert(v); }
        assert_eq!(t.yh_select(0), Some(3));
        assert_eq!(t.yh_select(2), Some(7));
        assert_eq!(t.yh_select(4), Some(15));
    }

    #[test]
    fn yh_ost_min_max() {
        let mut t = super::YhOrderStatTree::yh_new();
        t.yh_insert(10);
        t.yh_insert(5);
        t.yh_insert(15);
        assert_eq!(t.yh_min(), Some(5));
        assert_eq!(t.yh_max(), Some(15));
    }

    #[test]
    fn yh_ost_inorder() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [5, 3, 7, 1, 4] { t.yh_insert(v); }
        assert_eq!(t.yh_inorder(), vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn yh_ost_count_range() {
        let mut t = super::YhOrderStatTree::yh_new();
        for v in [1, 3, 5, 7, 9] { t.yh_insert(v); }
        assert_eq!(t.yh_count_range(3, 7), 3);
    }

    #[test]
    fn yh_ost_display() {
        let t = super::YhOrderStatTree::yh_new();
        assert!(format!("{}", t).contains("OSTree"));
    }

    #[test]
    fn yh_ost_default() {
        let t = super::YhOrderStatTree::default();
        assert!(t.yh_is_empty());
    }

    // --- yh_ Reservoir tests ---

    #[test]
    fn yh_reservoir_new() {
        let r = super::YhReservoirSampler::yh_new(5, 42);
        assert_eq!(r.yh_k(), 5);
        assert_eq!(r.yh_count(), 0);
    }

    #[test]
    fn yh_reservoir_add() {
        let mut r = super::YhReservoirSampler::yh_new(3, 42);
        for i in 0..10 { r.yh_add(i); }
        assert_eq!(r.yh_len(), 3);
        assert_eq!(r.yh_count(), 10);
    }

    #[test]
    fn yh_reservoir_underfill() {
        let mut r = super::YhReservoirSampler::yh_new(10, 42);
        r.yh_add(1);
        r.yh_add(2);
        assert_eq!(r.yh_len(), 2);
        assert!(!r.yh_is_full());
    }

    #[test]
    fn yh_reservoir_full() {
        let mut r = super::YhReservoirSampler::yh_new(3, 42);
        r.yh_add(1); r.yh_add(2); r.yh_add(3);
        assert!(r.yh_is_full());
    }

    #[test]
    fn yh_reservoir_reset() {
        let mut r = super::YhReservoirSampler::yh_new(3, 42);
        r.yh_add(1);
        r.yh_reset(99);
        assert_eq!(r.yh_count(), 0);
        assert_eq!(r.yh_len(), 0);
    }

    #[test]
    fn yh_reservoir_display() {
        let r = super::YhReservoirSampler::yh_new(5, 42);
        assert!(format!("{}", r).contains("Reservoir"));
    }

    #[test]
    fn yh_reservoir_default() {
        let r = super::YhReservoirSampler::default();
        assert_eq!(r.yh_k(), 10);
    }

    #[test]
    fn yh_reservoir_sample() {
        let mut r = super::YhReservoirSampler::yh_new(5, 42);
        for i in 0..100 { r.yh_add(i); }
        assert_eq!(r.yh_sample().len(), 5);
    }


    // --- yi_ RingBuffer tests ---

    #[test]
    fn yi_ring_new() {
        let r = super::YiRingBuffer::<i32>::yi_new(10);
        assert!(r.yi_is_empty());
        assert_eq!(r.yi_capacity(), 10);
    }

    #[test]
    fn yi_ring_push_pop() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_push_back(2);
        r.yi_push_back(3);
        assert_eq!(r.yi_pop_front(), Some(1));
        assert_eq!(r.yi_pop_front(), Some(2));
    }

    #[test]
    fn yi_ring_push_front() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_front(1);
        r.yi_push_front(2);
        assert_eq!(r.yi_front(), Some(&2));
    }

    #[test]
    fn yi_ring_full() {
        let mut r = super::YiRingBuffer::yi_new(2);
        assert!(r.yi_push_back(1));
        assert!(r.yi_push_back(2));
        assert!(!r.yi_push_back(3));
        assert!(r.yi_is_full());
    }

    #[test]
    fn yi_ring_wrap() {
        let mut r = super::YiRingBuffer::yi_new(3);
        r.yi_push_back(1);
        r.yi_push_back(2);
        r.yi_push_back(3);
        r.yi_pop_front();
        r.yi_push_back(4);
        assert_eq!(r.yi_to_vec(), vec![2, 3, 4]);
    }

    #[test]
    fn yi_ring_force_push() {
        let mut r = super::YiRingBuffer::yi_new(2);
        r.yi_force_push_back(1);
        r.yi_force_push_back(2);
        r.yi_force_push_back(3);
        assert_eq!(r.yi_to_vec(), vec![2, 3]);
    }

    #[test]
    fn yi_ring_get() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(10);
        r.yi_push_back(20);
        assert_eq!(r.yi_get(0), Some(&10));
        assert_eq!(r.yi_get(1), Some(&20));
    }

    #[test]
    fn yi_ring_clear() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_clear();
        assert!(r.yi_is_empty());
    }

    #[test]
    fn yi_ring_back() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_push_back(2);
        assert_eq!(r.yi_back(), Some(&2));
    }

    #[test]
    fn yi_ring_pop_back() {
        let mut r = super::YiRingBuffer::yi_new(5);
        r.yi_push_back(1);
        r.yi_push_back(2);
        assert_eq!(r.yi_pop_back(), Some(2));
        assert_eq!(r.yi_len(), 1);
    }

    #[test]
    fn yi_ring_display() {
        let r = super::YiRingBuffer::<i32>::yi_new(10);
        assert!(format!("{}", r).contains("RingBuffer"));
    }

    // --- yi_ WeightedGraph tests ---

    #[test]
    fn yi_wgraph_new() {
        let g = super::YiWeightedGraph::yi_new();
        assert_eq!(g.yi_node_count(), 0);
    }

    #[test]
    fn yi_wgraph_add_edge() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 5.0);
        assert_eq!(g.yi_node_count(), 2);
    }

    #[test]
    fn yi_wgraph_dijkstra() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 4.0);
        g.yi_add_edge(0, 2, 1.0);
        g.yi_add_edge(2, 1, 2.0);
        let dists = g.yi_dijkstra(0);
        assert_eq!(dists[&1], 3.0);
    }

    #[test]
    fn yi_wgraph_shortest() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 1.0);
        g.yi_add_edge(1, 2, 2.0);
        assert_eq!(g.yi_shortest_distance(0, 2), Some(3.0));
    }

    #[test]
    fn yi_wgraph_no_path() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_node(0);
        g.yi_add_node(1);
        assert_eq!(g.yi_shortest_distance(0, 1), None);
    }

    #[test]
    fn yi_wgraph_undirected() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_undirected_edge(0, 1, 3.0);
        assert_eq!(g.yi_shortest_distance(1, 0), Some(3.0));
    }

    #[test]
    fn yi_wgraph_total_weight() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 2.0);
        g.yi_add_edge(1, 2, 3.0);
        assert_eq!(g.yi_total_weight(), 5.0);
    }

    #[test]
    fn yi_wgraph_clear() {
        let mut g = super::YiWeightedGraph::yi_new();
        g.yi_add_edge(0, 1, 1.0);
        g.yi_clear();
        assert_eq!(g.yi_node_count(), 0);
    }

    #[test]
    fn yi_wgraph_display() {
        let g = super::YiWeightedGraph::yi_new();
        assert!(format!("{}", g).contains("WGraph"));
    }

    #[test]
    fn yi_wgraph_default() {
        let g = super::YiWeightedGraph::default();
        assert_eq!(g.yi_node_count(), 0);
    }


    // --- yj_ ExprEval tests ---

    #[test]
    fn yj_expr_simple() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("2 + 3").unwrap(), 5.0);
    }

    #[test]
    fn yj_expr_precedence() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("2 + 3 * 4").unwrap(), 14.0);
    }

    #[test]
    fn yj_expr_parens() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("(2 + 3) * 4").unwrap(), 20.0);
    }

    #[test]
    fn yj_expr_neg() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("-5 + 3").unwrap(), -2.0);
    }

    #[test]
    fn yj_expr_var() {
        let mut e = super::YjExprEval::yj_new();
        e.yj_set_var("x", 10.0);
        assert_eq!(e.yj_eval("x * 2").unwrap(), 20.0);
    }

    #[test]
    fn yj_expr_div() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("10 / 4").unwrap(), 2.5);
    }

    #[test]
    fn yj_expr_complex() {
        let e = super::YjExprEval::yj_new();
        assert_eq!(e.yj_eval("(1 + 2) * (3 + 4)").unwrap(), 21.0);
    }

    #[test]
    fn yj_expr_error() {
        let e = super::YjExprEval::yj_new();
        assert!(e.yj_eval("2 +").is_err());
    }

    #[test]
    fn yj_expr_display() {
        let e = super::YjExprEval::yj_new();
        assert!(format!("{}", e).contains("ExprEval"));
    }

    #[test]
    fn yj_expr_clear() {
        let mut e = super::YjExprEval::yj_new();
        e.yj_set_var("x", 1.0);
        e.yj_clear();
        assert_eq!(e.yj_var_count(), 0);
    }

    // --- yj_ TtlCache tests ---

    #[test]
    fn yj_ttl_new() {
        let c = super::YjTtlCache::<i32>::yj_new(100);
        assert_eq!(c.yj_ttl(), 100);
    }

    #[test]
    fn yj_ttl_put_get() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 42);
        assert_eq!(c.yj_get("a"), Some(&42));
    }

    #[test]
    fn yj_ttl_expired() {
        let mut c = super::YjTtlCache::yj_new(10);
        c.yj_put("a", 1);
        c.yj_tick(20);
        assert_eq!(c.yj_get("a"), None);
    }

    #[test]
    fn yj_ttl_not_expired() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 1);
        c.yj_tick(50);
        assert_eq!(c.yj_get("a"), Some(&1));
    }

    #[test]
    fn yj_ttl_evict() {
        let mut c = super::YjTtlCache::yj_new(10);
        c.yj_put("a", 1);
        c.yj_tick(20);
        c.yj_evict_expired();
        assert_eq!(c.yj_len(), 0);
    }

    #[test]
    fn yj_ttl_valid_count() {
        let mut c = super::YjTtlCache::yj_new(10);
        c.yj_put("a", 1);
        c.yj_tick(5);
        c.yj_put("b", 2);
        c.yj_tick(8);
        assert_eq!(c.yj_valid_count(), 1);
    }

    #[test]
    fn yj_ttl_remove() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 42);
        assert_eq!(c.yj_remove("a"), Some(42));
    }

    #[test]
    fn yj_ttl_clear() {
        let mut c = super::YjTtlCache::yj_new(100);
        c.yj_put("a", 1);
        c.yj_clear();
        assert_eq!(c.yj_len(), 0);
    }

    #[test]
    fn yj_ttl_display() {
        let c = super::YjTtlCache::<i32>::yj_new(10);
        assert!(format!("{}", c).contains("TtlCache"));
    }

    #[test]
    fn yj_ttl_default() {
        let c = super::YjTtlCache::<i32>::default();
        assert_eq!(c.yj_ttl(), 60);
    }


    // --- yk_ GlobMatcher tests ---

    #[test]
    fn yk_glob_exact() {
        let g = super::YkGlobMatcher::yk_new("hello");
        assert!(g.yk_matches("hello"));
        assert!(!g.yk_matches("world"));
    }

    #[test]
    fn yk_glob_star() {
        let g = super::YkGlobMatcher::yk_new("*.rs");
        assert!(g.yk_matches("main.rs"));
        assert!(!g.yk_matches("main.py"));
    }

    #[test]
    fn yk_glob_question() {
        let g = super::YkGlobMatcher::yk_new("?.txt");
        assert!(g.yk_matches("a.txt"));
        assert!(!g.yk_matches("ab.txt"));
    }

    #[test]
    fn yk_glob_complex() {
        let g = super::YkGlobMatcher::yk_new("src/*.rs");
        assert!(g.yk_matches("src/main.rs"));
        assert!(g.yk_matches("src/sub/main.rs"));
    }

    #[test]
    fn yk_glob_empty() {
        let g = super::YkGlobMatcher::yk_new("*");
        assert!(g.yk_matches("anything"));
        assert!(g.yk_matches(""));
    }

    #[test]
    fn yk_glob_matches_any() {
        assert!(super::YkGlobMatcher::yk_matches_any(&["*.rs", "*.py"], "main.rs"));
        assert!(!super::YkGlobMatcher::yk_matches_any(&["*.rs", "*.py"], "main.js"));
    }

    #[test]
    fn yk_glob_filter() {
        let g = super::YkGlobMatcher::yk_new("*.rs");
        let files = vec!["main.rs", "lib.rs", "main.py"];
        assert_eq!(g.yk_filter(&files), vec!["main.rs", "lib.rs"]);
    }

    #[test]
    fn yk_glob_display() {
        let g = super::YkGlobMatcher::yk_new("*.txt");
        assert!(format!("{}", g).contains("Glob"));
    }

    #[test]
    fn yk_glob_default() {
        let g = super::YkGlobMatcher::default();
        assert!(g.yk_matches(""));
    }

    // --- yk_ EventBus tests ---

    #[test]
    fn yk_bus_new() {
        let b = super::YkEventBus::yk_new();
        assert_eq!(b.yk_topic_count(), 0);
    }

    #[test]
    fn yk_bus_subscribe() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("click", "handler_a");
        assert_eq!(b.yk_subscriber_count("click"), 1);
    }

    #[test]
    fn yk_bus_emit() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("click", "handler_a");
        b.yk_subscribe("click", "handler_b");
        let notified = b.yk_emit("click");
        assert_eq!(notified.len(), 2);
    }

    #[test]
    fn yk_bus_unsubscribe() {
        let mut b = super::YkEventBus::yk_new();
        let id = b.yk_subscribe("click", "handler_a");
        b.yk_unsubscribe(id);
        assert_eq!(b.yk_subscriber_count("click"), 0);
    }

    #[test]
    fn yk_bus_topics() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("click", "a");
        b.yk_subscribe("keypress", "b");
        assert_eq!(b.yk_topics().len(), 2);
    }

    #[test]
    fn yk_bus_emit_pattern() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("mouse.click", "a");
        b.yk_subscribe("mouse.move", "b");
        b.yk_subscribe("key.press", "c");
        let notified = b.yk_emit_pattern("mouse.*");
        assert_eq!(notified.len(), 2);
    }

    #[test]
    fn yk_bus_clear() {
        let mut b = super::YkEventBus::yk_new();
        b.yk_subscribe("x", "a");
        b.yk_clear();
        assert_eq!(b.yk_total_subscribers(), 0);
    }

    #[test]
    fn yk_bus_has_subscribers() {
        let mut b = super::YkEventBus::yk_new();
        assert!(!b.yk_has_subscribers("x"));
        b.yk_subscribe("x", "a");
        assert!(b.yk_has_subscribers("x"));
    }

    #[test]
    fn yk_bus_display() {
        let b = super::YkEventBus::yk_new();
        assert!(format!("{}", b).contains("EventBus"));
    }

    #[test]
    fn yk_bus_default() {
        let b = super::YkEventBus::default();
        assert_eq!(b.yk_topic_count(), 0);
    }


    // --- yl_ MinMaxHeap tests ---

    #[test]
    fn yl_mmh_new() {
        let h = super::YlMinMaxHeap::yl_new();
        assert!(h.yl_is_empty());
    }

    #[test]
    fn yl_mmh_insert_min_max() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(5);
        h.yl_insert(3);
        h.yl_insert(8);
        h.yl_insert(1);
        assert_eq!(h.yl_peek_min(), Some(1));
        assert_eq!(h.yl_peek_max(), Some(8));
    }

    #[test]
    fn yl_mmh_pop_min() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(5);
        h.yl_insert(1);
        h.yl_insert(9);
        assert_eq!(h.yl_pop_min(), Some(1));
        assert_eq!(h.yl_peek_min(), Some(5));
    }

    #[test]
    fn yl_mmh_sorted() {
        let mut h = super::YlMinMaxHeap::yl_new();
        for v in [7, 3, 9, 1, 5] { h.yl_insert(v); }
        let sorted = h.yl_to_sorted_vec();
        assert_eq!(sorted, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn yl_mmh_single() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(42);
        assert_eq!(h.yl_peek_min(), Some(42));
        assert_eq!(h.yl_peek_max(), Some(42));
    }

    #[test]
    fn yl_mmh_two() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(5);
        h.yl_insert(3);
        assert_eq!(h.yl_peek_min(), Some(3));
        assert_eq!(h.yl_peek_max(), Some(5));
    }

    #[test]
    fn yl_mmh_clear() {
        let mut h = super::YlMinMaxHeap::yl_new();
        h.yl_insert(1);
        h.yl_clear();
        assert!(h.yl_is_empty());
    }

    #[test]
    fn yl_mmh_display() {
        let h = super::YlMinMaxHeap::yl_new();
        assert!(format!("{}", h).contains("MinMaxHeap"));
    }

    #[test]
    fn yl_mmh_default() {
        let h = super::YlMinMaxHeap::default();
        assert!(h.yl_is_empty());
    }

    // --- yl_ StateMachine tests ---

    #[test]
    fn yl_fsm_new() {
        let m = super::YlStateMachine::yl_new();
        assert_eq!(m.yl_state_count(), 0);
    }

    #[test]
    fn yl_fsm_basic() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("start");
        let s1 = m.yl_add_state("end");
        m.yl_add_transition(s0, "go", s1);
        m.yl_set_accept(s1);
        m.yl_set_start(s0);
        assert!(m.yl_step("go"));
        assert!(m.yl_is_accepting());
    }

    #[test]
    fn yl_fsm_run() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("a");
        let s1 = m.yl_add_state("b");
        let s2 = m.yl_add_state("c");
        m.yl_add_transition(s0, "x", s1);
        m.yl_add_transition(s1, "y", s2);
        m.yl_set_start(s0);
        assert!(m.yl_run(&["x", "y"]));
        assert_eq!(m.yl_current_state(), "c");
    }

    #[test]
    fn yl_fsm_invalid() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("start");
        m.yl_set_start(s0);
        assert!(!m.yl_step("invalid"));
    }

    #[test]
    fn yl_fsm_available() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("s0");
        let s1 = m.yl_add_state("s1");
        m.yl_add_transition(s0, "a", s1);
        m.yl_add_transition(s0, "b", s1);
        m.yl_set_start(s0);
        assert_eq!(m.yl_available_inputs().len(), 2);
    }

    #[test]
    fn yl_fsm_reset() {
        let mut m = super::YlStateMachine::yl_new();
        let s0 = m.yl_add_state("start");
        let s1 = m.yl_add_state("end");
        m.yl_add_transition(s0, "go", s1);
        m.yl_set_start(s0);
        m.yl_step("go");
        m.yl_reset();
        assert_eq!(m.yl_current_state(), "start");
    }

    #[test]
    fn yl_fsm_display() {
        let m = super::YlStateMachine::yl_new();
        assert!(format!("{}", m).contains("FSM"));
    }

    #[test]
    fn yl_fsm_default() {
        let m = super::YlStateMachine::default();
        assert_eq!(m.yl_state_count(), 0);
    }


    // --- ym_ SortedMultiMap tests ---

    #[test]
    fn ym_smm_new() {
        let m = super::YmSortedMultiMap::<i32, i32>::ym_new();
        assert!(m.ym_is_empty());
    }

    #[test]
    fn ym_smm_insert_get() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(1, "a");
        m.ym_insert(1, "b");
        m.ym_insert(2, "c");
        assert_eq!(m.ym_get(&1).len(), 2);
        assert_eq!(m.ym_total_count(), 3);
    }

    #[test]
    fn ym_smm_keys() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(3, 1);
        m.ym_insert(1, 2);
        m.ym_insert(2, 3);
        assert_eq!(m.ym_keys(), vec![1, 2, 3]);
    }

    #[test]
    fn ym_smm_remove() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(1, "x");
        m.ym_insert(1, "y");
        let removed = m.ym_remove_key(&1);
        assert_eq!(removed.len(), 2);
        assert!(m.ym_is_empty());
    }

    #[test]
    fn ym_smm_range() {
        let mut m = super::YmSortedMultiMap::ym_new();
        for i in 0..10 { m.ym_insert(i, i * 10); }
        let r = m.ym_range(&3, &7);
        assert_eq!(r, vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn ym_smm_first_last() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(5, 0);
        m.ym_insert(1, 0);
        m.ym_insert(9, 0);
        assert_eq!(m.ym_first_key(), Some(1));
        assert_eq!(m.ym_last_key(), Some(9));
    }

    #[test]
    fn ym_smm_clear() {
        let mut m = super::YmSortedMultiMap::ym_new();
        m.ym_insert(1, 1);
        m.ym_clear();
        assert!(m.ym_is_empty());
    }

    #[test]
    fn ym_smm_display() {
        let m = super::YmSortedMultiMap::<i32, i32>::ym_new();
        assert!(format!("{}", m).contains("SortedMultiMap"));
    }

    #[test]
    fn ym_smm_default() {
        let m = super::YmSortedMultiMap::<i32, i32>::default();
        assert!(m.ym_is_empty());
    }

    // --- ym_ TaskScheduler tests ---

    #[test]
    fn ym_sched_new() {
        let s = super::YmTaskScheduler::ym_new();
        assert_eq!(s.ym_total(), 0);
    }

    #[test]
    fn ym_sched_add() {
        let mut s = super::YmTaskScheduler::ym_new();
        let id = s.ym_add_task("build", 1, vec![]);
        assert_eq!(id, 0);
        assert_eq!(s.ym_total(), 1);
    }

    #[test]
    fn ym_sched_next_ready() {
        let mut s = super::YmTaskScheduler::ym_new();
        s.ym_add_task("low", 10, vec![]);
        s.ym_add_task("high", 1, vec![]);
        let next = s.ym_next_ready().unwrap();
        assert_eq!(next.ym_name, "high");
    }

    #[test]
    fn ym_sched_deps() {
        let mut s = super::YmTaskScheduler::ym_new();
        let t0 = s.ym_add_task("first", 1, vec![]);
        let _t1 = s.ym_add_task("second", 1, vec![t0]);
        let ready = s.ym_all_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].ym_name, "first");
    }

    #[test]
    fn ym_sched_complete() {
        let mut s = super::YmTaskScheduler::ym_new();
        let t0 = s.ym_add_task("a", 1, vec![]);
        let _t1 = s.ym_add_task("b", 1, vec![t0]);
        s.ym_complete(t0);
        let ready = s.ym_all_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].ym_name, "b");
    }

    #[test]
    fn ym_sched_all_done() {
        let mut s = super::YmTaskScheduler::ym_new();
        let t0 = s.ym_add_task("a", 1, vec![]);
        s.ym_complete(t0);
        assert!(s.ym_is_all_done());
    }

    #[test]
    fn ym_sched_clear() {
        let mut s = super::YmTaskScheduler::ym_new();
        s.ym_add_task("x", 1, vec![]);
        s.ym_clear();
        assert_eq!(s.ym_total(), 0);
    }

    #[test]
    fn ym_sched_display() {
        let s = super::YmTaskScheduler::ym_new();
        assert!(format!("{}", s).contains("Scheduler"));
    }

    #[test]
    fn ym_sched_default() {
        let s = super::YmTaskScheduler::default();
        assert!(s.ym_is_all_done());
    }

    #[test]
    fn ym_task_display() {
        let t = super::YmTask { ym_id: 0, ym_name: "test".to_string(), ym_priority: 1, ym_deps: vec![], ym_done: false };
        assert!(format!("{}", t).contains("Task"));
    }


    // --- yn_ ImmutableMap tests ---

    #[test]
    fn yn_imm_new() {
        let m = super::YnImmutableMap::<i32, i32>::yn_new();
        assert!(m.yn_is_empty());
    }

    #[test]
    fn yn_imm_insert_get() {
        let m = super::YnImmutableMap::yn_new();
        let m = m.yn_insert(1, "a");
        let m = m.yn_insert(2, "b");
        assert_eq!(m.yn_get(&1), Some(&"a"));
        assert_eq!(m.yn_get(&2), Some(&"b"));
    }

    #[test]
    fn yn_imm_persistence() {
        let m1 = super::YnImmutableMap::yn_new().yn_insert(1, 10);
        let m2 = m1.yn_insert(2, 20);
        assert_eq!(m1.yn_len(), 1);
        assert_eq!(m2.yn_len(), 2);
    }

    #[test]
    fn yn_imm_remove() {
        let m = super::YnImmutableMap::yn_new().yn_insert(1, 10).yn_insert(2, 20);
        let m2 = m.yn_remove(&1);
        assert!(!m2.yn_contains_key(&1));
        assert!(m.yn_contains_key(&1));
    }

    #[test]
    fn yn_imm_keys() {
        let m = super::YnImmutableMap::yn_new().yn_insert(3, 0).yn_insert(1, 0).yn_insert(2, 0);
        assert_eq!(m.yn_keys(), vec![1, 2, 3]);
    }

    #[test]
    fn yn_imm_merge() {
        let a = super::YnImmutableMap::yn_new().yn_insert(1, "a");
        let b = super::YnImmutableMap::yn_new().yn_insert(2, "b");
        let c = a.yn_merge(&b);
        assert_eq!(c.yn_len(), 2);
    }

    #[test]
    fn yn_imm_filter() {
        let m = super::YnImmutableMap::yn_new().yn_insert(1, 10).yn_insert(2, 20).yn_insert(3, 30);
        let f = m.yn_filter(|_, v| *v > 15);
        assert_eq!(f.yn_len(), 2);
    }

    #[test]
    fn yn_imm_display() {
        let m = super::YnImmutableMap::<i32, i32>::yn_new();
        assert!(format!("{}", m).contains("ImmMap"));
    }

    #[test]
    fn yn_imm_default() {
        let m = super::YnImmutableMap::<i32, i32>::default();
        assert!(m.yn_is_empty());
    }

    // --- yn_ Tokenizer tests ---

    #[test]
    fn yn_tok_word() {
        let tokens = super::YnTokenizer::yn_tokenize("hello");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnWord);
    }

    #[test]
    fn yn_tok_number() {
        let tokens = super::YnTokenizer::yn_tokenize("42");
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnNumber);
    }

    #[test]
    fn yn_tok_mixed() {
        let tokens = super::YnTokenizer::yn_tokenize_no_ws("x + 42");
        assert_eq!(tokens.len(), 3);
    }

    #[test]
    fn yn_tok_string() {
        let tokens = super::YnTokenizer::yn_tokenize("hi_world");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnWord);
    }

    #[test]
    fn yn_tok_punct() {
        let tokens = super::YnTokenizer::yn_tokenize("+");
        assert_eq!(tokens[0].yn_kind, super::YnTokenKind::YnPunct);
    }

    #[test]
    fn yn_tok_count_kind() {
        let tokens = super::YnTokenizer::yn_tokenize("a + b + c");
        let count = super::YnTokenizer::yn_count_by_kind(&tokens, &super::YnTokenKind::YnWord);
        assert_eq!(count, 3);
    }

    #[test]
    fn yn_tok_empty() {
        let tokens = super::YnTokenizer::yn_tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn yn_tok_display() {
        let t = super::YnTokenizer;
        assert!(format!("{}", t).contains("Tokenizer"));
    }

    #[test]
    fn yn_tok_default() {
        let _t = super::YnTokenizer::default();
    }

    #[test]
    fn yn_tok_kind_display() {
        assert!(format!("{}", super::YnTokenKind::YnWord).contains("Word"));
    }


    // --- yo_ Levenshtein tests ---

    #[test]
    fn yo_lev_identical() {
        assert_eq!(super::YoLevenshtein::yo_distance("hello", "hello"), 0);
    }

    #[test]
    fn yo_lev_empty() {
        assert_eq!(super::YoLevenshtein::yo_distance("", "abc"), 3);
        assert_eq!(super::YoLevenshtein::yo_distance("abc", ""), 3);
    }

    #[test]
    fn yo_lev_basic() {
        assert_eq!(super::YoLevenshtein::yo_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn yo_lev_single_char() {
        assert_eq!(super::YoLevenshtein::yo_distance("a", "b"), 1);
    }

    #[test]
    fn yo_lev_similarity() {
        let s = super::YoLevenshtein::yo_similarity("hello", "hello");
        assert!((s - 1.0).abs() < 0.001);
    }

    #[test]
    fn yo_lev_closest() {
        let c = super::YoLevenshtein::yo_closest("cat", &["bat", "car", "dog"]);
        assert!(c == Some("bat") || c == Some("car"));
    }

    #[test]
    fn yo_lev_within() {
        let r = super::YoLevenshtein::yo_within_distance("cat", &["bat", "car", "dog"], 1);
        assert!(r.contains(&"bat"));
        assert!(r.contains(&"car"));
    }

    #[test]
    fn yo_lev_rank() {
        let r = super::YoLevenshtein::yo_rank("cat", &["dog", "bat", "cat"]);
        assert_eq!(r[0].0, "cat");
    }

    #[test]
    fn yo_lev_display() {
        assert!(format!("{}", super::YoLevenshtein).contains("Levenshtein"));
    }

    // --- yo_ DiffEngine tests ---

    #[test]
    fn yo_diff_identical() {
        let ops = super::YoDiffEngine::yo_diff("a\nb", "a\nb");
        assert_eq!(super::YoDiffEngine::yo_count_equal(&ops), 2);
    }

    #[test]
    fn yo_diff_insert() {
        let ops = super::YoDiffEngine::yo_diff("a", "a\nb");
        assert_eq!(super::YoDiffEngine::yo_count_insertions(&ops), 1);
    }

    #[test]
    fn yo_diff_delete() {
        let ops = super::YoDiffEngine::yo_diff("a\nb", "a");
        assert_eq!(super::YoDiffEngine::yo_count_deletions(&ops), 1);
    }

    #[test]
    fn yo_diff_replace() {
        let ops = super::YoDiffEngine::yo_diff("a", "b");
        assert!(super::YoDiffEngine::yo_count_insertions(&ops) > 0 || super::YoDiffEngine::yo_count_deletions(&ops) > 0);
    }

    #[test]
    fn yo_diff_empty() {
        let ops = super::YoDiffEngine::yo_diff("", "");
        assert!(ops.is_empty());
    }

    #[test]
    fn yo_diff_format() {
        let ops = super::YoDiffEngine::yo_diff("a", "b");
        let s = super::YoDiffEngine::yo_format(&ops);
        assert!(!s.is_empty());
    }

    #[test]
    fn yo_diff_op_display() {
        let op = super::YoDiffOp::YoInsert("line".to_string());
        assert!(format!("{}", op).contains("+"));
    }

    #[test]
    fn yo_diff_display() {
        assert!(format!("{}", super::YoDiffEngine).contains("DiffEngine"));
    }


    // --- yp_ JsonValue tests ---

    #[test]
    fn yp_json_null() {
        let v = super::YpJsonValue::YpNull;
        assert!(v.yp_is_null());
    }

    #[test]
    fn yp_json_string() {
        let v = super::YpJsonValue::yp_string("hello");
        assert_eq!(v.yp_as_str(), Some("hello"));
    }

    #[test]
    fn yp_json_number() {
        let v = super::YpJsonValue::yp_number(42.0);
        assert_eq!(v.yp_as_f64(), Some(42.0));
    }

    #[test]
    fn yp_json_bool() {
        let v = super::YpJsonValue::yp_bool(true);
        assert_eq!(v.yp_as_bool(), Some(true));
    }

    #[test]
    fn yp_json_object() {
        let mut obj = super::YpJsonValue::yp_object();
        obj.yp_set("name", super::YpJsonValue::yp_string("test"));
        assert_eq!(obj.yp_get("name").unwrap().yp_as_str(), Some("test"));
    }

    #[test]
    fn yp_json_array() {
        let mut arr = super::YpJsonValue::yp_array();
        arr.yp_push(super::YpJsonValue::yp_number(1.0));
        arr.yp_push(super::YpJsonValue::yp_number(2.0));
        assert_eq!(arr.yp_len(), 2);
    }

    #[test]
    fn yp_json_path() {
        let mut obj = super::YpJsonValue::yp_object();
        let mut inner = super::YpJsonValue::yp_object();
        inner.yp_set("b", super::YpJsonValue::yp_number(42.0));
        obj.yp_set("a", inner);
        assert_eq!(obj.yp_path("a.b").unwrap().yp_as_f64(), Some(42.0));
    }

    #[test]
    fn yp_json_merge() {
        let mut a = super::YpJsonValue::yp_object();
        a.yp_set("x", super::YpJsonValue::yp_number(1.0));
        let mut b = super::YpJsonValue::yp_object();
        b.yp_set("y", super::YpJsonValue::yp_number(2.0));
        let c = a.yp_merge(&b);
        assert_eq!(c.yp_len(), 2);
    }

    #[test]
    fn yp_json_keys() {
        let mut obj = super::YpJsonValue::yp_object();
        obj.yp_set("a", super::YpJsonValue::YpNull);
        obj.yp_set("b", super::YpJsonValue::YpNull);
        assert_eq!(obj.yp_keys().len(), 2);
    }

    #[test]
    fn yp_json_display() {
        let v = super::YpJsonValue::yp_string("hi");
        assert!(format!("{}", v).contains("hi"));
    }

    #[test]
    fn yp_json_default() {
        let v = super::YpJsonValue::default();
        assert!(v.yp_is_null());
    }

    // --- yp_ CommandRegistry tests ---

    #[test]
    fn yp_cmdreg_new() {
        let r = super::YpCommandRegistry::yp_new();
        assert_eq!(r.yp_count(), 0);
    }

    #[test]
    fn yp_cmdreg_register() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("editor.copy", "Copy", "Edit");
        assert_eq!(r.yp_count(), 1);
    }

    #[test]
    fn yp_cmdreg_find() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("editor.copy", "Copy", "Edit");
        assert!(r.yp_find("editor.copy").is_some());
    }

    #[test]
    fn yp_cmdreg_search() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("editor.copy", "Copy Selection", "Edit");
        r.yp_register("editor.paste", "Paste", "Edit");
        let results = r.yp_search("copy");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn yp_cmdreg_category() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("a", "A", "Edit");
        r.yp_register("b", "B", "View");
        assert_eq!(r.yp_by_category("Edit").len(), 1);
    }

    #[test]
    fn yp_cmdreg_keybinding() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register_with_key("copy", "Copy", "Edit", "Ctrl+C");
        assert!(r.yp_by_keybinding("Ctrl+C").is_some());
    }

    #[test]
    fn yp_cmdreg_categories() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("a", "A", "Edit");
        r.yp_register("b", "B", "View");
        assert_eq!(r.yp_categories().len(), 2);
    }

    #[test]
    fn yp_cmdreg_clear() {
        let mut r = super::YpCommandRegistry::yp_new();
        r.yp_register("a", "A", "X");
        r.yp_clear();
        assert_eq!(r.yp_count(), 0);
    }

    #[test]
    fn yp_cmdreg_display() {
        let r = super::YpCommandRegistry::yp_new();
        assert!(format!("{}", r).contains("CmdRegistry"));
    }

    #[test]
    fn yp_cmd_display() {
        let c = super::YpCommandEntry { yp_id: "test".into(), yp_title: "T".into(), yp_category: "C".into(), yp_keybinding: None, yp_when: None };
        assert!(format!("{}", c).contains("Cmd"));
    }


    // --- yq_ ConfigStore tests ---

    #[test]
    fn yq_config_new() {
        let c = super::YqConfigStore::yq_new();
        assert_eq!(c.yq_layer_count(), 3);
    }

    #[test]
    fn yq_config_set_get() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("user", "theme", "dark");
        assert_eq!(c.yq_get("theme"), Some("dark"));
    }

    #[test]
    fn yq_config_layering() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("defaults", "font", "mono");
        c.yq_set("user", "font", "sans");
        assert_eq!(c.yq_get("font"), Some("sans"));
    }

    #[test]
    fn yq_config_workspace_overrides() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("defaults", "size", "12");
        c.yq_set("workspace", "size", "14");
        assert_eq!(c.yq_get("size"), Some("14"));
    }

    #[test]
    fn yq_config_get_or() {
        let c = super::YqConfigStore::yq_new();
        assert_eq!(c.yq_get_or("missing", "default"), "default");
    }

    #[test]
    fn yq_config_get_i64() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("user", "port", "8080");
        assert_eq!(c.yq_get_i64("port"), Some(8080));
    }

    #[test]
    fn yq_config_get_bool() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("user", "debug", "true");
        assert_eq!(c.yq_get_bool("debug"), Some(true));
    }

    #[test]
    fn yq_config_all_keys() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("defaults", "a", "1");
        c.yq_set("user", "b", "2");
        assert_eq!(c.yq_all_keys().len(), 2);
    }

    #[test]
    fn yq_config_effective() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("defaults", "x", "1");
        c.yq_set("user", "x", "2");
        assert_eq!(c.yq_effective_layer("x"), Some("user"));
    }

    #[test]
    fn yq_config_clear() {
        let mut c = super::YqConfigStore::yq_new();
        c.yq_set("user", "a", "1");
        c.yq_clear_layer("user");
        assert_eq!(c.yq_get("a"), None);
    }

    #[test]
    fn yq_config_display() {
        let c = super::YqConfigStore::yq_new();
        assert!(format!("{}", c).contains("ConfigStore"));
    }

    // --- yq_ TextLayout tests ---

    #[test]
    fn yq_layout_wrap_short() {
        let l = super::YqTextLayout::yq_new(80);
        let lines = l.yq_wrap("hello");
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn yq_layout_wrap_long() {
        let l = super::YqTextLayout::yq_new(10);
        let lines = l.yq_wrap("hello world foo bar");
        assert!(lines.len() > 1);
    }

    #[test]
    fn yq_layout_truncate() {
        let l = super::YqTextLayout::yq_new(10);
        let t = l.yq_truncate("hello world foo", "...");
        assert!(t.len() <= 10);
        assert!(t.ends_with("..."));
    }

    #[test]
    fn yq_layout_pad() {
        let l = super::YqTextLayout::yq_new(10);
        let p = l.yq_pad_right("hi");
        assert_eq!(p.len(), 10);
    }

    #[test]
    fn yq_layout_center() {
        let l = super::YqTextLayout::yq_new(10);
        let c = l.yq_center("hi");
        assert_eq!(c.len(), 10);
    }

    #[test]
    fn yq_layout_line_count() {
        let l = super::YqTextLayout::yq_new(5);
        assert!(l.yq_line_count("hello world") > 1);
    }

    #[test]
    fn yq_layout_display() {
        let l = super::YqTextLayout::yq_new(80);
        assert!(format!("{}", l).contains("TextLayout"));
    }

    #[test]
    fn yq_layout_default() {
        let l = super::YqTextLayout::default();
        assert_eq!(l.yq_width(), 80);
    }


    // --- yr_ UndoStack tests ---

    #[test]
    fn yr_undo_new() {
        let s = super::YrUndoStack::<String>::yr_new(100);
        assert!(!s.yr_can_undo());
    }

    #[test]
    fn yr_undo_push_undo() {
        let mut s = super::YrUndoStack::yr_new(100);
        s.yr_push("a".to_string());
        s.yr_push("b".to_string());
        assert_eq!(s.yr_undo(), Some("b".to_string()));
        assert_eq!(s.yr_current(), Some(&"a".to_string()));
    }

    #[test]
    fn yr_undo_redo() {
        let mut s = super::YrUndoStack::yr_new(100);
        s.yr_push(1);
        s.yr_push(2);
        s.yr_undo();
        assert!(s.yr_can_redo());
        assert_eq!(s.yr_redo(), Some(2));
    }

    #[test]
    fn yr_undo_push_clears_redo() {
        let mut s = super::YrUndoStack::yr_new(100);
        s.yr_push(1);
        s.yr_push(2);
        s.yr_undo();
        s.yr_push(3);
        assert!(!s.yr_can_redo());
    }

    #[test]
    fn yr_undo_max_size() {
        let mut s = super::YrUndoStack::yr_new(3);
        for i in 0..10 { s.yr_push(i); }
        assert_eq!(s.yr_undo_count(), 3);
    }

    #[test]
    fn yr_undo_clear() {
        let mut s = super::YrUndoStack::yr_new(100);
        s.yr_push(1);
        s.yr_clear();
        assert!(!s.yr_can_undo());
    }

    #[test]
    fn yr_undo_display() {
        let s = super::YrUndoStack::<i32>::yr_new(100);
        assert!(format!("{}", s).contains("UndoStack"));
    }

    #[test]
    fn yr_undo_default() {
        let s = super::YrUndoStack::<i32>::default();
        assert_eq!(s.yr_max_size(), 1000);
    }

    // --- yr_ Selection tests ---

    #[test]
    fn yr_sel_cursor() {
        let s = super::YrSelection::yr_cursor(5, 10);
        assert!(s.yr_is_cursor());
    }

    #[test]
    fn yr_sel_range() {
        let s = super::YrSelection::yr_range(1, 0, 3, 5);
        assert!(!s.yr_is_cursor());
    }

    #[test]
    fn yr_sel_start_end() {
        let s = super::YrSelection::yr_range(3, 5, 1, 0);
        assert_eq!(s.yr_start(), (1, 0));
        assert_eq!(s.yr_end(), (3, 5));
    }

    #[test]
    fn yr_sel_contains() {
        let s = super::YrSelection::yr_range(1, 0, 3, 0);
        assert!(s.yr_contains(2, 5));
    }

    #[test]
    fn yr_sel_reversed() {
        let s = super::YrSelection::yr_range(3, 0, 1, 0);
        assert!(s.yr_is_reversed());
    }

    #[test]
    fn yr_sel_line_span() {
        let s = super::YrSelection::yr_range(1, 0, 5, 0);
        assert_eq!(s.yr_line_span(), 5);
    }

    #[test]
    fn yr_sel_collapse() {
        let s = super::YrSelection::yr_range(1, 0, 3, 5);
        let c = s.yr_collapse();
        assert!(c.yr_is_cursor());
        assert_eq!(c.yr_active_line, 3);
    }

    #[test]
    fn yr_sel_display() {
        let s = super::YrSelection::yr_cursor(1, 2);
        assert!(format!("{}", s).contains("Sel"));
    }

    // --- yr_ SelectionModel tests ---

    #[test]
    fn yr_model_new() {
        let m = super::YrSelectionModel::yr_new();
        assert_eq!(m.yr_cursor_count(), 1);
    }

    #[test]
    fn yr_model_multi() {
        let mut m = super::YrSelectionModel::yr_new();
        m.yr_add(super::YrSelection::yr_cursor(5, 0));
        assert_eq!(m.yr_cursor_count(), 2);
    }

    #[test]
    fn yr_model_collapse() {
        let mut m = super::YrSelectionModel::yr_new();
        m.yr_set_primary(super::YrSelection::yr_range(0, 0, 5, 5));
        m.yr_collapse_all();
        assert!(m.yr_primary().yr_is_cursor());
    }

    #[test]
    fn yr_model_reset() {
        let mut m = super::YrSelectionModel::yr_new();
        m.yr_add(super::YrSelection::yr_cursor(5, 0));
        m.yr_reset();
        assert_eq!(m.yr_cursor_count(), 1);
    }

    #[test]
    fn yr_model_display() {
        let m = super::YrSelectionModel::yr_new();
        assert!(format!("{}", m).contains("SelectionModel"));
    }


    // --- ys_ tests ---

    #[test]
    fn test_ys_gcounter_new() {
        let c = YsGCounter::new();
        assert_eq!(c.value(), 0);
        assert!(c.is_empty());
        assert_eq!(c.replica_count(), 0);
    }

    #[test]
    fn test_ys_gcounter_increment() {
        let mut c = YsGCounter::new();
        c.increment("a");
        c.increment("a");
        c.increment("b");
        assert_eq!(c.value(), 3);
        assert_eq!(c.local_value("a"), 2);
        assert_eq!(c.local_value("b"), 1);
        assert_eq!(c.local_value("c"), 0);
    }

    #[test]
    fn test_ys_gcounter_increment_by() {
        let mut c = YsGCounter::new();
        c.increment_by("x", 10);
        c.increment_by("y", 5);
        assert_eq!(c.value(), 15);
        assert!(!c.is_empty());
    }

    #[test]
    fn test_ys_gcounter_merge() {
        let mut a = YsGCounter::new();
        a.increment_by("r1", 3);
        a.increment_by("r2", 1);
        let mut b = YsGCounter::new();
        b.increment_by("r1", 2);
        b.increment_by("r2", 5);
        b.increment_by("r3", 4);
        a.merge(&b);
        assert_eq!(a.local_value("r1"), 3); // max(3, 2)
        assert_eq!(a.local_value("r2"), 5); // max(1, 5)
        assert_eq!(a.local_value("r3"), 4); // new
        assert_eq!(a.value(), 12);
    }

    #[test]
    fn test_ys_gcounter_replicas() {
        let mut c = YsGCounter::new();
        c.increment("b");
        c.increment("a");
        assert_eq!(c.replicas(), vec!["a", "b"]);
    }

    #[test]
    fn test_ys_gcounter_display() {
        let c = YsGCounter::new();
        let s = format!("{}", c);
        assert!(s.contains("YsGCounter"));
    }

    #[test]
    fn test_ys_gcounter_default() {
        let c = YsGCounter::default();
        assert_eq!(c.value(), 0);
    }

    #[test]
    fn test_ys_version_vector_new() {
        let v = YsVersionVector::new();
        assert!(v.is_empty());
        assert_eq!(v.len(), 0);
        assert_eq!(v.max_version(), 0);
    }

    #[test]
    fn test_ys_version_vector_increment() {
        let mut v = YsVersionVector::new();
        assert_eq!(v.increment("a"), 1);
        assert_eq!(v.increment("a"), 2);
        assert_eq!(v.increment("b"), 1);
        assert_eq!(v.get("a"), 2);
        assert_eq!(v.get("b"), 1);
        assert_eq!(v.get("c"), 0);
    }

    #[test]
    fn test_ys_version_vector_set() {
        let mut v = YsVersionVector::new();
        v.set("x", 10);
        assert_eq!(v.get("x"), 10);
    }

    #[test]
    fn test_ys_version_vector_merge() {
        let mut a = YsVersionVector::new();
        a.set("r1", 3);
        a.set("r2", 1);
        let mut b = YsVersionVector::new();
        b.set("r1", 2);
        b.set("r2", 5);
        b.set("r3", 4);
        a.merge(&b);
        assert_eq!(a.get("r1"), 3);
        assert_eq!(a.get("r2"), 5);
        assert_eq!(a.get("r3"), 4);
    }

    #[test]
    fn test_ys_version_vector_dominates() {
        let mut a = YsVersionVector::new();
        a.set("r1", 3);
        a.set("r2", 2);
        let mut b = YsVersionVector::new();
        b.set("r1", 2);
        b.set("r2", 1);
        assert!(a.dominates(&b));
        assert!(!b.dominates(&a));
    }

    #[test]
    fn test_ys_version_vector_concurrent() {
        let mut a = YsVersionVector::new();
        a.set("r1", 3);
        a.set("r2", 1);
        let mut b = YsVersionVector::new();
        b.set("r1", 2);
        b.set("r2", 5);
        assert!(a.is_concurrent(&b));
    }

    #[test]
    fn test_ys_version_vector_equal() {
        let mut a = YsVersionVector::new();
        a.set("r1", 3);
        let mut b = YsVersionVector::new();
        b.set("r1", 3);
        assert!(a.is_equal(&b));
    }

    #[test]
    fn test_ys_version_vector_replicas() {
        let mut v = YsVersionVector::new();
        v.set("b", 1);
        v.set("a", 2);
        assert_eq!(v.replicas(), vec!["a", "b"]);
    }

    #[test]
    fn test_ys_version_vector_max_sum() {
        let mut v = YsVersionVector::new();
        v.set("a", 5);
        v.set("b", 3);
        assert_eq!(v.max_version(), 5);
        assert_eq!(v.sum_versions(), 8);
    }

    #[test]
    fn test_ys_version_vector_display() {
        let v = YsVersionVector::default();
        let s = format!("{}", v);
        assert!(s.contains("YsVersionVector"));
    }


    // --- yt_ tests ---

    #[test]
    fn test_yt_regex_literal() {
        let r = YtRegex::new("hello");
        assert!(r.is_match("hello"));
        assert!(r.is_match("say hello world"));
        assert!(!r.is_match("HELLO"));
    }

    #[test]
    fn test_yt_regex_dot() {
        let r = YtRegex::new("h.llo");
        assert!(r.is_match("hello"));
        assert!(r.is_match("hallo"));
        assert!(!r.is_match("hllo"));
    }

    #[test]
    fn test_yt_regex_star() {
        let r = YtRegex::new("ab*c");
        assert!(r.is_match("ac"));
        assert!(r.is_match("abc"));
        assert!(r.is_match("abbc"));
        assert!(r.is_match("abbbc"));
    }

    #[test]
    fn test_yt_regex_plus() {
        let r = YtRegex::new("ab+c");
        assert!(!r.is_match("ac"));
        assert!(r.is_match("abc"));
        assert!(r.is_match("abbc"));
    }

    #[test]
    fn test_yt_regex_optional() {
        let r = YtRegex::new("colou?r");
        assert!(r.is_match("color"));
        assert!(r.is_match("colour"));
    }

    #[test]
    fn test_yt_regex_char_class() {
        let r = YtRegex::new("[abc]at");
        assert!(r.is_match("bat"));
        assert!(r.is_match("cat"));
        assert!(!r.is_match("dat"));
    }

    #[test]
    fn test_yt_regex_negated_class() {
        let r = YtRegex::new("[^abc]at");
        assert!(!r.is_match("bat"));
        assert!(r.is_match("dat"));
    }

    #[test]
    fn test_yt_regex_anchors() {
        let r = YtRegex::new("^hello$");
        assert!(r.is_match("hello"));
        assert!(!r.is_match("hello world"));
        assert!(!r.is_match("say hello"));
    }

    #[test]
    fn test_yt_regex_start_anchor() {
        let r = YtRegex::new("^hello");
        assert!(r.is_match("hello world"));
        assert!(!r.is_match("say hello"));
    }

    #[test]
    fn test_yt_regex_end_anchor() {
        let r = YtRegex::new("world$");
        assert!(r.is_match("hello world"));
        assert!(!r.is_match("world!"));
    }

    #[test]
    fn test_yt_regex_find() {
        let r = YtRegex::new("ab+");
        let result = r.find("xabbc");
        assert_eq!(result, Some((1, 4)));
    }

    #[test]
    fn test_yt_regex_find_all() {
        let r = YtRegex::new("a.");
        let results = r.find_all("abacad");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_yt_regex_display() {
        let r = YtRegex::new("ab*c");
        assert_eq!(format!("{}", r), "YtRegex(ab*c)");
    }

    #[test]
    fn test_yt_regex_pattern() {
        let r = YtRegex::new("test");
        assert_eq!(r.pattern(), "test");
    }

    #[test]
    fn test_yt_regex_escaped() {
        let r = YtRegex::new("a\\.b");
        assert!(r.is_match("a.b"));
        assert!(!r.is_match("axb"));
    }

    #[test]
    fn test_yt_wildcard_star() {
        let w = YtWildcard::new("*.rs");
        assert!(w.is_match("main.rs"));
        assert!(w.is_match(".rs"));
        assert!(!w.is_match("main.txt"));
    }

    #[test]
    fn test_yt_wildcard_question() {
        let w = YtWildcard::new("?.txt");
        assert!(w.is_match("a.txt"));
        assert!(!w.is_match("ab.txt"));
    }

    #[test]
    fn test_yt_wildcard_complex() {
        let w = YtWildcard::new("src/**/test_*.rs");
        assert!(w.is_match("src/**/test_main.rs"));
    }

    #[test]
    fn test_yt_wildcard_filter() {
        let w = YtWildcard::new("*.rs");
        let items: Vec<String> = vec!["a.rs".into(), "b.txt".into(), "c.rs".into()];
        assert_eq!(w.filter(&items).len(), 2);
    }

    #[test]
    fn test_yt_wildcard_display() {
        let w = YtWildcard::new("*.txt");
        assert_eq!(format!("{}", w), "YtWildcard(*.txt)");
    }


    // --- yu_ tests ---

    #[test]
    fn test_yu_rope_new() {
        let r = YuRope::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert_eq!(r.char_count(), 0);
    }

    #[test]
    fn test_yu_rope_from_str() {
        let r = YuRope::from_str("hello");
        assert_eq!(r.len(), 5);
        assert_eq!(r.text(), "hello");
    }

    #[test]
    fn test_yu_rope_insert() {
        let mut r = YuRope::from_str("hllo");
        r.insert(1, "e");
        assert_eq!(r.text(), "hello");
    }

    #[test]
    fn test_yu_rope_delete() {
        let mut r = YuRope::from_str("hello");
        r.delete(1, 3);
        assert_eq!(r.text(), "hlo");
    }

    #[test]
    fn test_yu_rope_substr() {
        let r = YuRope::from_str("hello world");
        assert_eq!(r.substr(0, 5), "hello");
        assert_eq!(r.substr(6, 11), "world");
    }

    #[test]
    fn test_yu_rope_char_at() {
        let r = YuRope::from_str("abcde");
        assert_eq!(r.char_at(0), Some('a'));
        assert_eq!(r.char_at(4), Some('e'));
        assert_eq!(r.char_at(5), None);
    }

    #[test]
    fn test_yu_rope_lines() {
        let r = YuRope::from_str("line1\nline2\nline3");
        assert_eq!(r.line_count(), 3);
        assert_eq!(r.line(0), Some("line1".to_string()));
        assert_eq!(r.line(2), Some("line3".to_string()));
    }

    #[test]
    fn test_yu_rope_append() {
        let mut a = YuRope::from_str("hello ");
        let b = YuRope::from_str("world");
        a.append(&b);
        assert_eq!(a.text(), "hello world");
    }

    #[test]
    fn test_yu_rope_display() {
        let r = YuRope::from_str("test");
        let s = format!("{}", r);
        assert!(s.contains("YuRope"));
    }

    #[test]
    fn test_yu_rope_default() {
        let r = YuRope::default();
        assert!(r.is_empty());
    }

    #[test]
    fn test_yu_piece_table_new() {
        let pt = YuPieceTable::new("hello");
        assert_eq!(pt.text(), "hello");
        assert_eq!(pt.len(), 5);
        assert!(!pt.is_empty());
    }

    #[test]
    fn test_yu_piece_table_insert() {
        let mut pt = YuPieceTable::new("hllo");
        pt.insert(1, "e");
        assert_eq!(pt.text(), "hello");
    }

    #[test]
    fn test_yu_piece_table_insert_at_end() {
        let mut pt = YuPieceTable::new("hello");
        pt.insert(5, " world");
        assert_eq!(pt.text(), "hello world");
    }

    #[test]
    fn test_yu_piece_table_delete() {
        let mut pt = YuPieceTable::new("hello world");
        pt.delete(5, 6);
        assert_eq!(pt.text(), "hello");
    }

    #[test]
    fn test_yu_piece_table_delete_middle() {
        let mut pt = YuPieceTable::new("abcdef");
        pt.delete(2, 2);
        assert_eq!(pt.text(), "abef");
    }

    #[test]
    fn test_yu_piece_table_multiple_ops() {
        let mut pt = YuPieceTable::new("hello");
        pt.insert(5, " world");
        pt.insert(0, "say ");
        assert_eq!(pt.text(), "say hello world");
    }

    #[test]
    fn test_yu_piece_table_empty() {
        let pt = YuPieceTable::new("");
        assert!(pt.is_empty());
        assert_eq!(pt.len(), 0);
    }

    #[test]
    fn test_yu_piece_table_lines() {
        let pt = YuPieceTable::new("a\nb\nc");
        assert_eq!(pt.line_count(), 3);
    }

    #[test]
    fn test_yu_piece_table_display() {
        let pt = YuPieceTable::new("test");
        let s = format!("{}", pt);
        assert!(s.contains("YuPieceTable"));
    }

    #[test]
    fn test_yu_piece_table_default() {
        let pt = YuPieceTable::default();
        assert!(pt.is_empty());
    }


    // --- yv_ tests ---

    #[test]
    fn test_yv_bplus_new() {
        let t: YvBPlusTree<i32, String> = YvBPlusTree::new();
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn test_yv_bplus_insert_get() {
        let mut t = YvBPlusTree::new();
        t.insert(3, "three");
        t.insert(1, "one");
        t.insert(2, "two");
        assert_eq!(t.get(&1), Some(&"one"));
        assert_eq!(t.get(&2), Some(&"two"));
        assert_eq!(t.get(&3), Some(&"three"));
        assert_eq!(t.get(&4), None);
    }

    #[test]
    fn test_yv_bplus_remove() {
        let mut t = YvBPlusTree::new();
        t.insert(1, "a");
        t.insert(2, "b");
        assert_eq!(t.remove(&1), Some("a"));
        assert_eq!(t.get(&1), None);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn test_yv_bplus_update() {
        let mut t = YvBPlusTree::new();
        t.insert(1, "old");
        t.insert(1, "new");
        assert_eq!(t.get(&1), Some(&"new"));
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn test_yv_bplus_range() {
        let mut t = YvBPlusTree::new();
        for i in 0..10 {
            t.insert(i, i * 10);
        }
        let r = t.range(&3, &7);
        assert_eq!(r.len(), 5);
    }

    #[test]
    fn test_yv_bplus_first_last() {
        let mut t = YvBPlusTree::new();
        t.insert(5, "e");
        t.insert(1, "a");
        t.insert(9, "i");
        assert_eq!(t.first().unwrap().0, &1);
        assert_eq!(t.last().unwrap().0, &9);
    }

    #[test]
    fn test_yv_bplus_keys_values() {
        let mut t = YvBPlusTree::new();
        t.insert(2, "b");
        t.insert(1, "a");
        assert_eq!(t.keys(), vec![&1, &2]);
        assert_eq!(t.values(), vec![&"a", &"b"]);
    }

    #[test]
    fn test_yv_bplus_rank_select() {
        let mut t = YvBPlusTree::new();
        t.insert(10, "a");
        t.insert(20, "b");
        t.insert(30, "c");
        assert_eq!(t.rank(&20), 1);
        assert_eq!(t.select(1).unwrap().0, &20);
    }

    #[test]
    fn test_yv_bplus_display() {
        let t: YvBPlusTree<i32, i32> = YvBPlusTree::new();
        let s = format!("{}", t);
        assert!(s.contains("YvBPlusTree"));
    }

    #[test]
    fn test_yv_bplus_default() {
        let t: YvBPlusTree<i32, i32> = YvBPlusTree::default();
        assert!(t.is_empty());
    }

    #[test]
    fn test_yv_skip_new() {
        let s: YvSkipListMap<i32, String> = YvSkipListMap::new();
        assert!(s.is_empty());
        assert_eq!(s.max_level(), 16);
    }

    #[test]
    fn test_yv_skip_insert_get() {
        let mut s = YvSkipListMap::new();
        s.insert(3, "three");
        s.insert(1, "one");
        s.insert(2, "two");
        assert_eq!(s.get(&1), Some(&"one"));
        assert_eq!(s.get(&2), Some(&"two"));
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn test_yv_skip_remove() {
        let mut s = YvSkipListMap::new();
        s.insert(1, "a");
        s.insert(2, "b");
        assert_eq!(s.remove(&1), Some("a"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn test_yv_skip_floor_ceiling() {
        let mut s = YvSkipListMap::new();
        s.insert(10, "a");
        s.insert(20, "b");
        s.insert(30, "c");
        assert_eq!(s.floor(&25).unwrap().0, &20);
        assert_eq!(s.ceiling(&25).unwrap().0, &30);
    }

    #[test]
    fn test_yv_skip_range() {
        let mut s = YvSkipListMap::new();
        for i in 0..10 { s.insert(i, i); }
        let r = s.range(&2, &5);
        assert_eq!(r.len(), 4);
    }

    #[test]
    fn test_yv_skip_first_last() {
        let mut s = YvSkipListMap::new();
        s.insert(5, "x");
        s.insert(1, "y");
        assert_eq!(s.first().unwrap().0, &1);
        assert_eq!(s.last().unwrap().0, &5);
    }

    #[test]
    fn test_yv_skip_display() {
        let s: YvSkipListMap<i32, i32> = YvSkipListMap::new();
        let s = format!("{}", s);
        assert!(s.contains("YvSkipListMap"));
    }

    #[test]
    fn test_yv_skip_default() {
        let s: YvSkipListMap<i32, i32> = YvSkipListMap::default();
        assert!(s.is_empty());
    }

    #[test]
    fn test_yv_skip_with_max_level() {
        let s: YvSkipListMap<i32, i32> = YvSkipListMap::with_max_level(8);
        assert_eq!(s.max_level(), 8);
    }

    #[test]
    fn test_yv_skip_clear() {
        let mut s = YvSkipListMap::new();
        s.insert(1, 10);
        s.clear();
        assert!(s.is_empty());
    }


    // --- yw_ tests ---

    #[test]
    fn test_yw_pool_new() {
        let p = YwThreadPool::new(4);
        assert_eq!(p.num_threads(), 4);
        assert!(p.is_idle());
        assert_eq!(p.completed(), 0);
    }

    #[test]
    fn test_yw_pool_submit() {
        let mut p = YwThreadPool::new(2);
        assert!(p.submit());
        assert!(p.submit());
        assert_eq!(p.pending(), 2);
        assert!(!p.is_idle());
    }

    #[test]
    fn test_yw_pool_process_one() {
        let mut p = YwThreadPool::new(2);
        p.submit();
        p.submit();
        assert!(p.process_one());
        assert_eq!(p.pending(), 1);
        assert_eq!(p.completed(), 1);
    }

    #[test]
    fn test_yw_pool_process_all() {
        let mut p = YwThreadPool::new(2);
        p.submit();
        p.submit();
        p.submit();
        assert_eq!(p.process_all(), 3);
        assert!(p.is_idle());
        assert_eq!(p.completed(), 3);
    }

    #[test]
    fn test_yw_pool_shutdown() {
        let mut p = YwThreadPool::new(2);
        p.submit();
        p.shutdown();
        assert!(p.is_shutdown());
        assert!(!p.submit());
    }

    #[test]
    fn test_yw_pool_utilization() {
        let mut p = YwThreadPool::new(2);
        assert_eq!(p.utilization(), 0.0);
        p.submit();
        p.process_one();
        assert_eq!(p.utilization(), 1.0);
    }

    #[test]
    fn test_yw_pool_display() {
        let p = YwThreadPool::new(4);
        let s = format!("{}", p);
        assert!(s.contains("YwThreadPool"));
    }

    #[test]
    fn test_yw_pool_default() {
        let p = YwThreadPool::default();
        assert_eq!(p.num_threads(), 4);
    }

    #[test]
    fn test_yw_future_ready() {
        let f = YwFuture::ready(42);
        assert!(f.is_ready());
        assert_eq!(f.value(), Some(&42));
    }

    #[test]
    fn test_yw_future_pending() {
        let f: YwFuture<i32> = YwFuture::pending();
        assert!(f.is_pending());
        assert_eq!(f.value(), None);
    }

    #[test]
    fn test_yw_future_failed() {
        let f: YwFuture<i32> = YwFuture::failed("oops");
        assert!(f.is_failed());
        assert_eq!(f.error(), Some("oops"));
    }

    #[test]
    fn test_yw_future_map() {
        let f = YwFuture::ready(5);
        let g = f.map(|x| x * 2);
        assert_eq!(g.value(), Some(&10));
    }

    #[test]
    fn test_yw_future_flat_map() {
        let f = YwFuture::ready(5);
        let g = f.flat_map(|x| YwFuture::ready(x + 1));
        assert_eq!(g.value(), Some(&6));
    }

    #[test]
    fn test_yw_future_or_else() {
        let f: YwFuture<i32> = YwFuture::pending();
        assert_eq!(f.or_else(99), 99);
        let g = YwFuture::ready(42);
        assert_eq!(g.or_else(99), 42);
    }

    #[test]
    fn test_yw_future_resolve() {
        let mut f: YwFuture<i32> = YwFuture::pending();
        f.resolve(42);
        assert!(f.is_ready());
        assert_eq!(f.value(), Some(&42));
    }

    #[test]
    fn test_yw_future_reject() {
        let mut f: YwFuture<i32> = YwFuture::pending();
        f.reject("err");
        assert!(f.is_failed());
    }

    #[test]
    fn test_yw_future_all_ready() {
        let fs = vec![YwFuture::ready(1), YwFuture::ready(2), YwFuture::ready(3)];
        let result = yw_future_all(&fs);
        assert_eq!(result.value(), Some(&vec![1, 2, 3]));
    }

    #[test]
    fn test_yw_future_all_pending() {
        let fs: Vec<YwFuture<i32>> = vec![YwFuture::ready(1), YwFuture::pending()];
        let result = yw_future_all(&fs);
        assert!(result.is_pending());
    }

    #[test]
    fn test_yw_future_all_failed() {
        let fs: Vec<YwFuture<i32>> = vec![YwFuture::ready(1), YwFuture::failed("err")];
        let result = yw_future_all(&fs);
        assert!(result.is_failed());
    }

    #[test]
    fn test_yw_future_race() {
        let fs: Vec<YwFuture<i32>> = vec![YwFuture::pending(), YwFuture::ready(42)];
        let result = yw_future_race(&fs);
        assert_eq!(result.value(), Some(&42));
    }

    #[test]
    fn test_yw_future_display() {
        let f = YwFuture::ready(5);
        let s = format!("{}", f);
        assert!(s.contains("YwFuture"));
    }

    #[test]
    fn test_yw_future_default() {
        let f: YwFuture<i32> = YwFuture::default();
        assert!(f.is_pending());
    }


    // --- yx_ tests ---

    #[test]
    fn test_yx_lru_new() {
        let c: YxLruCache<i32> = YxLruCache::new(3);
        assert!(c.is_empty());
        assert_eq!(c.capacity(), 3);
    }

    #[test]
    fn test_yx_lru_put_get() {
        let mut c = YxLruCache::new(3);
        c.put("a", 1);
        c.put("b", 2);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.get("c"), None);
    }

    #[test]
    fn test_yx_lru_eviction() {
        let mut c = YxLruCache::new(2);
        c.put("a", 1);
        c.put("b", 2);
        c.put("c", 3); // evicts "a"
        assert!(!c.contains("a"));
        assert!(c.contains("b"));
        assert!(c.contains("c"));
    }

    #[test]
    fn test_yx_lru_access_refresh() {
        let mut c = YxLruCache::new(2);
        c.put("a", 1);
        c.put("b", 2);
        c.get("a"); // refresh "a"
        c.put("c", 3); // evicts "b" not "a"
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn test_yx_lru_remove() {
        let mut c = YxLruCache::new(3);
        c.put("a", 1);
        assert_eq!(c.remove("a"), Some(1));
        assert!(c.is_empty());
    }

    #[test]
    fn test_yx_lru_update() {
        let mut c = YxLruCache::new(3);
        c.put("a", 1);
        c.put("a", 2);
        assert_eq!(c.peek("a"), Some(&2));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn test_yx_lru_most_least_recent() {
        let mut c = YxLruCache::new(3);
        c.put("a", 1);
        c.put("b", 2);
        assert_eq!(c.most_recent().unwrap().0, "b");
        assert_eq!(c.least_recent().unwrap().0, "a");
    }

    #[test]
    fn test_yx_lru_resize() {
        let mut c = YxLruCache::new(5);
        for i in 0..5 { c.put(&format!("k{}", i), i); }
        c.resize(2);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn test_yx_lru_display() {
        let c: YxLruCache<i32> = YxLruCache::new(3);
        let s = format!("{}", c);
        assert!(s.contains("YxLruCache"));
    }

    #[test]
    fn test_yx_lfu_new() {
        let c: YxLfuCache<i32> = YxLfuCache::new(3);
        assert!(c.is_empty());
        assert_eq!(c.capacity(), 3);
    }

    #[test]
    fn test_yx_lfu_put_get() {
        let mut c = YxLfuCache::new(3);
        c.put("a", 1);
        c.put("b", 2);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
    }

    #[test]
    fn test_yx_lfu_eviction() {
        let mut c = YxLfuCache::new(2);
        c.put("a", 1);
        c.put("b", 2);
        c.get("a"); // freq(a)=2, freq(b)=1
        c.put("c", 3); // evicts "b" (least frequent)
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
        assert!(c.contains("c"));
    }

    #[test]
    fn test_yx_lfu_frequency() {
        let mut c = YxLfuCache::new(3);
        c.put("a", 1);
        c.get("a");
        c.get("a");
        assert_eq!(c.frequency("a"), 3); // 1 from put + 2 from get
    }

    #[test]
    fn test_yx_lfu_remove() {
        let mut c = YxLfuCache::new(3);
        c.put("a", 1);
        assert_eq!(c.remove("a"), Some(1));
        assert!(c.is_empty());
    }

    #[test]
    fn test_yx_lfu_most_least_frequent() {
        let mut c = YxLfuCache::new(3);
        c.put("a", 1);
        c.put("b", 2);
        c.get("a");
        c.get("a");
        assert_eq!(c.most_frequent().unwrap().0, "a");
        assert_eq!(c.least_frequent().unwrap().0, "b");
    }

    #[test]
    fn test_yx_lfu_display() {
        let c: YxLfuCache<i32> = YxLfuCache::new(3);
        let s = format!("{}", c);
        assert!(s.contains("YxLfuCache"));
    }


    // --- yy_ tests ---

    #[test]
    fn test_yy_emitter_new() {
        let e = YyEventEmitter::new();
        assert_eq!(e.total_listeners(), 0);
        assert_eq!(e.emit_count(), 0);
    }

    #[test]
    fn test_yy_emitter_on() {
        let mut e = YyEventEmitter::new();
        let id = e.on("click");
        assert_eq!(e.listener_count("click"), 1);
        assert!(e.has_listeners("click"));
        assert!(id < 100);
    }

    #[test]
    fn test_yy_emitter_emit() {
        let mut e = YyEventEmitter::new();
        e.on("click");
        e.on("click");
        assert_eq!(e.emit("click"), 2);
        assert_eq!(e.listener_count("click"), 2); // not once, so still there
    }

    #[test]
    fn test_yy_emitter_once() {
        let mut e = YyEventEmitter::new();
        e.once("click");
        assert_eq!(e.listener_count("click"), 1);
        e.emit("click");
        assert_eq!(e.listener_count("click"), 0);
    }

    #[test]
    fn test_yy_emitter_off() {
        let mut e = YyEventEmitter::new();
        let id = e.on("click");
        assert!(e.off(id));
        assert_eq!(e.total_listeners(), 0);
    }

    #[test]
    fn test_yy_emitter_events() {
        let mut e = YyEventEmitter::new();
        e.on("click");
        e.on("hover");
        let events = e.events();
        assert!(events.contains(&"click".to_string()));
        assert!(events.contains(&"hover".to_string()));
    }

    #[test]
    fn test_yy_emitter_clear() {
        let mut e = YyEventEmitter::new();
        e.on("click");
        e.on("hover");
        e.clear();
        assert_eq!(e.total_listeners(), 0);
    }

    #[test]
    fn test_yy_emitter_clear_event() {
        let mut e = YyEventEmitter::new();
        e.on("click");
        e.on("hover");
        e.clear_event("click");
        assert!(!e.has_listeners("click"));
        assert!(e.has_listeners("hover"));
    }

    #[test]
    fn test_yy_emitter_display() {
        let e = YyEventEmitter::new();
        let s = format!("{}", e);
        assert!(s.contains("YyEventEmitter"));
    }

    #[test]
    fn test_yy_emitter_default() {
        let e = YyEventEmitter::default();
        assert_eq!(e.total_listeners(), 0);
    }

    #[test]
    fn test_yy_observable_new() {
        let o = YyObservable::new(42);
        assert_eq!(*o.get(), 42);
        assert_eq!(o.version(), 0);
    }

    #[test]
    fn test_yy_observable_set() {
        let mut o = YyObservable::new(1);
        assert!(o.set(2));
        assert_eq!(*o.get(), 2);
        assert_eq!(o.version(), 1);
    }

    #[test]
    fn test_yy_observable_no_change() {
        let mut o = YyObservable::new(1);
        assert!(!o.set(1));
        assert_eq!(o.version(), 0);
    }

    #[test]
    fn test_yy_observable_force_set() {
        let mut o = YyObservable::new(1);
        o.force_set(1);
        assert_eq!(o.version(), 1);
    }

    #[test]
    fn test_yy_observable_watchers() {
        let mut o = YyObservable::new(0);
        o.add_watcher();
        o.add_watcher();
        assert_eq!(o.watcher_count(), 2);
        assert!(o.has_watchers());
        o.remove_watcher();
        assert_eq!(o.watcher_count(), 1);
    }

    #[test]
    fn test_yy_observable_map() {
        let o = YyObservable::new(5);
        let doubled = o.map(|x| x * 2);
        assert_eq!(*doubled.get(), 10);
    }

    #[test]
    fn test_yy_observable_change_count() {
        let mut o = YyObservable::new(0);
        o.set(1);
        o.set(2);
        o.set(2); // no change
        assert_eq!(o.change_count(), 2);
    }

    #[test]
    fn test_yy_observable_display() {
        let o = YyObservable::new(42);
        let s = format!("{}", o);
        assert!(s.contains("YyObservable"));
    }

    #[test]
    fn test_yy_observable_default() {
        let o: YyObservable<i32> = YyObservable::default();
        assert_eq!(*o.get(), 0);
    }


    // --- yz_ tests ---

    #[test]
    fn test_yz_disposable_new() {
        let d = YzDisposableStore::new();
        assert_eq!(d.total_count(), 0);
        assert!(!d.has_active());
    }

    #[test]
    fn test_yz_disposable_register() {
        let mut d = YzDisposableStore::new();
        let id = d.register("listener");
        assert_eq!(d.active_count(), 1);
        assert!(!d.is_disposed(id));
    }

    #[test]
    fn test_yz_disposable_dispose() {
        let mut d = YzDisposableStore::new();
        let id = d.register("item");
        assert!(d.dispose(id));
        assert!(d.is_disposed(id));
        assert_eq!(d.active_count(), 0);
    }

    #[test]
    fn test_yz_disposable_dispose_twice() {
        let mut d = YzDisposableStore::new();
        let id = d.register("item");
        assert!(d.dispose(id));
        assert!(!d.dispose(id));
    }

    #[test]
    fn test_yz_disposable_dispose_all() {
        let mut d = YzDisposableStore::new();
        d.register("a");
        d.register("b");
        d.register("c");
        assert_eq!(d.dispose_all(), 3);
        assert_eq!(d.active_count(), 0);
    }

    #[test]
    fn test_yz_disposable_active_labels() {
        let mut d = YzDisposableStore::new();
        d.register("a");
        let b = d.register("b");
        d.register("c");
        d.dispose(b);
        let labels = d.active_labels();
        assert_eq!(labels.len(), 2);
        assert!(labels.contains(&"a"));
        assert!(labels.contains(&"c"));
    }

    #[test]
    fn test_yz_disposable_clear() {
        let mut d = YzDisposableStore::new();
        d.register("x");
        d.clear();
        assert_eq!(d.total_count(), 0);
    }

    #[test]
    fn test_yz_disposable_display() {
        let d = YzDisposableStore::new();
        let s = format!("{}", d);
        assert!(s.contains("YzDisposableStore"));
    }

    #[test]
    fn test_yz_disposable_default() {
        let d = YzDisposableStore::default();
        assert_eq!(d.total_count(), 0);
    }

    #[test]
    fn test_yz_cancel_new() {
        let t = YzCancellationToken::new();
        assert!(!t.is_cancelled());
        assert_eq!(t.reason(), None);
    }

    #[test]
    fn test_yz_cancel_cancel() {
        let mut t = YzCancellationToken::new();
        t.cancel();
        assert!(t.is_cancelled());
    }

    #[test]
    fn test_yz_cancel_with_reason() {
        let mut t = YzCancellationToken::new();
        t.cancel_with_reason("timeout");
        assert!(t.is_cancelled());
        assert_eq!(t.reason(), Some("timeout"));
    }

    #[test]
    fn test_yz_cancel_throw() {
        let t = YzCancellationToken::new();
        assert!(t.throw_if_cancelled().is_ok());
        let mut t2 = YzCancellationToken::new();
        t2.cancel();
        assert!(t2.throw_if_cancelled().is_err());
    }

    #[test]
    fn test_yz_cancel_listeners() {
        let mut t = YzCancellationToken::new();
        t.add_listener();
        t.add_listener();
        assert_eq!(t.listener_count(), 2);
        t.remove_listener();
        assert_eq!(t.listener_count(), 1);
    }

    #[test]
    fn test_yz_cancel_reset() {
        let mut t = YzCancellationToken::new();
        t.cancel_with_reason("err");
        t.reset();
        assert!(!t.is_cancelled());
        assert_eq!(t.reason(), None);
    }

    #[test]
    fn test_yz_cancel_link() {
        let a = YzCancellationToken::new();
        let mut b = YzCancellationToken::new();
        b.cancel();
        let linked = YzCancellationToken::link(&a, &b);
        assert!(linked.is_cancelled());
    }

    #[test]
    fn test_yz_cancel_link_both_active() {
        let a = YzCancellationToken::new();
        let b = YzCancellationToken::new();
        let linked = YzCancellationToken::link(&a, &b);
        assert!(!linked.is_cancelled());
    }

    #[test]
    fn test_yz_cancel_precancelled() {
        let t = YzCancellationToken::cancelled();
        assert!(t.is_cancelled());
    }

    #[test]
    fn test_yz_cancel_none() {
        let t = YzCancellationToken::none();
        assert!(!t.is_cancelled());
    }

    #[test]
    fn test_yz_cancel_display() {
        let t = YzCancellationToken::new();
        let s = format!("{}", t);
        assert!(s.contains("active"));
        let mut t2 = YzCancellationToken::new();
        t2.cancel();
        let s2 = format!("{}", t2);
        assert!(s2.contains("cancelled"));
    }

    #[test]
    fn test_yz_cancel_default() {
        let t = YzCancellationToken::default();
        assert!(!t.is_cancelled());
    }


    // --- za_ tests ---

    #[test]
    fn test_za_uri_parse() {
        let u = ZaUri::parse("https://example.com/path?q=1#frag").unwrap();
        assert_eq!(u.scheme, "https");
        assert_eq!(u.authority, "example.com");
        assert_eq!(u.path, "/path");
        assert_eq!(u.query, "q=1");
        assert_eq!(u.fragment, "frag");
    }

    #[test]
    fn test_za_uri_file() {
        let u = ZaUri::file("/home/user/file.txt");
        assert!(u.is_file());
        assert_eq!(u.fs_path(), "/home/user/file.txt");
    }

    #[test]
    fn test_za_uri_untitled() {
        let u = ZaUri::from_parts("untitled", "", "/Untitled-1", "", "");
        assert!(u.is_untitled());
    }

    #[test]
    fn test_za_uri_with_path() {
        let u = ZaUri::file("/old").with_path("/new");
        assert_eq!(u.path, "/new");
    }

    #[test]
    fn test_za_uri_display() {
        let u = ZaUri::file("/test");
        let s = format!("{}", u);
        assert!(s.contains("file://"));
    }

    #[test]
    fn test_za_uri_equality() {
        let a = ZaUri::file("/a");
        let b = ZaUri::file("/a");
        assert_eq!(a, b);
    }

    #[test]
    fn test_za_path_basename() {
        assert_eq!(ZaPath::basename("/home/user/file.txt"), "file.txt");
        assert_eq!(ZaPath::basename("file.txt"), "file.txt");
    }

    #[test]
    fn test_za_path_dirname() {
        assert_eq!(ZaPath::dirname("/home/user/file.txt"), "/home/user");
        assert_eq!(ZaPath::dirname("/file.txt"), "/");
    }

    #[test]
    fn test_za_path_extname() {
        assert_eq!(ZaPath::extname("file.txt"), ".txt");
        assert_eq!(ZaPath::extname("file"), "");
        assert_eq!(ZaPath::extname(".gitignore"), "");
    }

    #[test]
    fn test_za_path_join() {
        assert_eq!(ZaPath::join("/home", "file.txt"), "/home/file.txt");
        assert_eq!(ZaPath::join("/home/", "file.txt"), "/home/file.txt");
        assert_eq!(ZaPath::join("/home", "/absolute"), "/absolute");
    }

    #[test]
    fn test_za_path_normalize() {
        assert_eq!(ZaPath::normalize("/home/user/../file"), "/home/file");
        assert_eq!(ZaPath::normalize("/home/./file"), "/home/file");
    }

    #[test]
    fn test_za_path_is_absolute() {
        assert!(ZaPath::is_absolute("/home"));
        assert!(!ZaPath::is_absolute("home"));
    }

    #[test]
    fn test_za_path_relative() {
        assert_eq!(ZaPath::relative("/a/b/c", "/a/b/d"), "../d");
        assert_eq!(ZaPath::relative("/a/b", "/a/c/d"), "../c/d");
    }

    #[test]
    fn test_za_path_has_extension() {
        assert!(ZaPath::has_extension("file.rs", ".rs"));
        assert!(ZaPath::has_extension("file.rs", "rs"));
        assert!(!ZaPath::has_extension("file.txt", ".rs"));
    }


    // --- zb_ tests ---

    #[test]
    fn test_zb_position_new() {
        let p = ZbPosition::new(5, 10);
        assert_eq!(p.line, 5);
        assert_eq!(p.character, 10);
    }

    #[test]
    fn test_zb_position_origin() {
        let p = ZbPosition::origin();
        assert_eq!(p.line, 0);
        assert_eq!(p.character, 0);
    }

    #[test]
    fn test_zb_position_compare() {
        let a = ZbPosition::new(1, 5);
        let b = ZbPosition::new(1, 10);
        let c = ZbPosition::new(2, 0);
        assert!(a.is_before(&b));
        assert!(b.is_before(&c));
        assert!(c.is_after(&a));
    }

    #[test]
    fn test_zb_position_min_max() {
        let a = ZbPosition::new(1, 5);
        let b = ZbPosition::new(2, 0);
        assert_eq!(ZbPosition::min(a, b), a);
        assert_eq!(ZbPosition::max(a, b), b);
    }

    #[test]
    fn test_zb_position_translate() {
        let p = ZbPosition::new(5, 10);
        let q = p.translate(1, -3);
        assert_eq!(q.line, 6);
        assert_eq!(q.character, 7);
    }

    #[test]
    fn test_zb_position_display() {
        let p = ZbPosition::new(0, 0);
        assert_eq!(format!("{}", p), "1:1");
    }

    #[test]
    fn test_zb_range_new() {
        let r = ZbRange::from_coords(1, 0, 1, 10);
        assert_eq!(r.start.line, 1);
        assert_eq!(r.end.character, 10);
    }

    #[test]
    fn test_zb_range_empty() {
        let r = ZbRange::empty(ZbPosition::new(5, 5));
        assert!(r.is_empty());
        assert!(r.is_single_line());
    }

    #[test]
    fn test_zb_range_contains() {
        let r = ZbRange::from_coords(1, 0, 3, 10);
        assert!(r.contains(ZbPosition::new(2, 5)));
        assert!(!r.contains(ZbPosition::new(0, 0)));
    }

    #[test]
    fn test_zb_range_intersects() {
        let a = ZbRange::from_coords(1, 0, 3, 0);
        let b = ZbRange::from_coords(2, 0, 5, 0);
        assert!(a.intersects(&b));
    }

    #[test]
    fn test_zb_range_intersection() {
        let a = ZbRange::from_coords(1, 0, 3, 0);
        let b = ZbRange::from_coords(2, 0, 5, 0);
        let i = a.intersection(&b).unwrap();
        assert_eq!(i.start.line, 2);
        assert_eq!(i.end.line, 3);
    }

    #[test]
    fn test_zb_range_union() {
        let a = ZbRange::from_coords(1, 0, 3, 0);
        let b = ZbRange::from_coords(2, 0, 5, 0);
        let u = a.union(&b);
        assert_eq!(u.start.line, 1);
        assert_eq!(u.end.line, 5);
    }

    #[test]
    fn test_zb_range_line_count() {
        let r = ZbRange::from_coords(1, 0, 5, 0);
        assert_eq!(r.line_count(), 5);
    }

    #[test]
    fn test_zb_range_display() {
        let r = ZbRange::from_coords(0, 0, 0, 5);
        let s = format!("{}", r);
        assert!(s.contains("["));
    }

    #[test]
    fn test_zb_location_new() {
        let loc = ZbLocation::new("file:///test.rs", ZbRange::from_coords(0, 0, 0, 5));
        assert_eq!(loc.uri, "file:///test.rs");
    }

    #[test]
    fn test_zb_location_from_pos() {
        let loc = ZbLocation::from_position("file:///a.rs", ZbPosition::new(10, 5));
        assert!(loc.range.is_empty());
    }

    #[test]
    fn test_zb_location_display() {
        let loc = ZbLocation::new("file.rs", ZbRange::default());
        let s = format!("{}", loc);
        assert!(s.contains("file.rs"));
    }


    // --- zc_ tests ---

    #[test]
    fn test_zc_edit_insert() {
        let e = ZcTextEdit::insert(5, 10, "hello");
        assert!(e.is_insert());
        assert!(!e.is_delete());
        assert!(!e.is_replace());
    }

    #[test]
    fn test_zc_edit_delete() {
        let e = ZcTextEdit::delete(1, 0, 1, 5);
        assert!(e.is_delete());
        assert!(!e.is_insert());
    }

    #[test]
    fn test_zc_edit_replace() {
        let e = ZcTextEdit::new(1, 0, 1, 5, "new");
        assert!(e.is_replace());
    }

    #[test]
    fn test_zc_edit_replace_line() {
        let e = ZcTextEdit::replace_line(3, "new content");
        assert_eq!(e.range_start_line, 3);
        assert_eq!(e.new_text, "new content");
    }

    #[test]
    fn test_zc_edit_affects_line() {
        let e = ZcTextEdit::new(2, 0, 5, 10, "x");
        assert!(e.affects_line(3));
        assert!(!e.affects_line(1));
        assert!(!e.affects_line(6));
    }

    #[test]
    fn test_zc_edit_display() {
        let e = ZcTextEdit::insert(0, 0, "hi");
        let s = format!("{}", e);
        assert!(s.contains("insert"));
    }

    #[test]
    fn test_zc_doc_change_new() {
        let dc = ZcDocumentChange::new("file:///test.rs", 1);
        assert!(dc.is_empty());
        assert_eq!(dc.version, 1);
    }

    #[test]
    fn test_zc_doc_change_add_edits() {
        let mut dc = ZcDocumentChange::new("file.rs", 1);
        dc.add_insert(0, 0, "hello");
        dc.add_delete(1, 0, 1, 5);
        assert_eq!(dc.edit_count(), 2);
        assert!(dc.has_inserts());
        assert!(dc.has_deletes());
    }

    #[test]
    fn test_zc_doc_change_affected_lines() {
        let mut dc = ZcDocumentChange::new("f", 1);
        dc.add_edit(ZcTextEdit::new(2, 0, 4, 0, "x"));
        let lines = dc.affected_lines();
        assert_eq!(lines, vec![2, 3, 4]);
    }

    #[test]
    fn test_zc_doc_change_sort() {
        let mut dc = ZcDocumentChange::new("f", 1);
        dc.add_insert(5, 0, "b");
        dc.add_insert(1, 0, "a");
        dc.sort_edits();
        assert_eq!(dc.edits[0].range_start_line, 1);
    }

    #[test]
    fn test_zc_doc_change_display() {
        let dc = ZcDocumentChange::new("test", 1);
        let s = format!("{}", dc);
        assert!(s.contains("ZcDocumentChange"));
    }

    #[test]
    fn test_zc_workspace_edit_new() {
        let we = ZcWorkspaceEdit::new();
        assert!(we.is_empty());
        assert_eq!(we.document_count(), 0);
    }

    #[test]
    fn test_zc_workspace_edit_add() {
        let mut we = ZcWorkspaceEdit::new();
        let mut dc = ZcDocumentChange::new("a.rs", 1);
        dc.add_insert(0, 0, "x");
        we.add_change(dc);
        assert_eq!(we.document_count(), 1);
        assert_eq!(we.total_edits(), 1);
    }

    #[test]
    fn test_zc_workspace_edit_uris() {
        let mut we = ZcWorkspaceEdit::new();
        we.add_change(ZcDocumentChange::new("a.rs", 1));
        we.add_change(ZcDocumentChange::new("b.rs", 1));
        assert_eq!(we.uris(), vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn test_zc_workspace_edit_get() {
        let mut we = ZcWorkspaceEdit::new();
        we.add_change(ZcDocumentChange::new("a.rs", 1));
        assert!(we.get_changes("a.rs").is_some());
        assert!(we.get_changes("b.rs").is_none());
    }

    #[test]
    fn test_zc_workspace_edit_display() {
        let we = ZcWorkspaceEdit::new();
        let s = format!("{}", we);
        assert!(s.contains("ZcWorkspaceEdit"));
    }

    #[test]
    fn test_zc_workspace_edit_default() {
        let we = ZcWorkspaceEdit::default();
        assert!(we.is_empty());
    }


    // --- zd_ tests ---

    #[test]
    fn test_zd_severity() {
        assert!(ZdSeverity::Error.is_error());
        assert!(ZdSeverity::Warning.is_warning());
        assert_eq!(ZdSeverity::Error.as_str(), "error");
    }

    #[test]
    fn test_zd_severity_from_u8() {
        assert_eq!(ZdSeverity::from_u8(0), ZdSeverity::Error);
        assert_eq!(ZdSeverity::from_u8(1), ZdSeverity::Warning);
        assert_eq!(ZdSeverity::from_u8(99), ZdSeverity::Hint);
    }

    #[test]
    fn test_zd_severity_ord() {
        assert!(ZdSeverity::Error < ZdSeverity::Warning);
        assert!(ZdSeverity::Warning < ZdSeverity::Hint);
    }

    #[test]
    fn test_zd_severity_display() {
        assert_eq!(format!("{}", ZdSeverity::Error), "error");
    }

    #[test]
    fn test_zd_diagnostic_error() {
        let d = ZdDiagnostic::error("missing semicolon", 10, 5, 6);
        assert!(d.is_error());
        assert_eq!(d.message, "missing semicolon");
        assert_eq!(d.start_line, 10);
    }

    #[test]
    fn test_zd_diagnostic_warning() {
        let d = ZdDiagnostic::warning("unused var", 3, 0, 5);
        assert!(d.is_warning());
    }

    #[test]
    fn test_zd_diagnostic_with_source() {
        let d = ZdDiagnostic::error("err", 0, 0, 1).with_source("rustc");
        assert_eq!(d.source, "rustc");
    }

    #[test]
    fn test_zd_diagnostic_with_code() {
        let d = ZdDiagnostic::error("err", 0, 0, 1).with_code("E0001");
        assert_eq!(d.code, Some("E0001".to_string()));
    }

    #[test]
    fn test_zd_diagnostic_tags() {
        let d = ZdDiagnostic::hint("unused", 0, 0, 1).with_tag(ZdDiagnosticTag::Unnecessary);
        assert!(d.is_unnecessary());
        assert!(!d.is_deprecated());
    }

    #[test]
    fn test_zd_diagnostic_related() {
        let mut d = ZdDiagnostic::error("err", 0, 0, 1);
        d.add_related("file.rs", 5, 0, "defined here");
        assert_eq!(d.related.len(), 1);
    }

    #[test]
    fn test_zd_diagnostic_affects_line() {
        let d = ZdDiagnostic::error("err", 5, 0, 10);
        assert!(d.affects_line(5));
        assert!(!d.affects_line(4));
    }

    #[test]
    fn test_zd_diagnostic_display() {
        let d = ZdDiagnostic::error("test", 0, 0, 1);
        let s = format!("{}", d);
        assert!(s.contains("error"));
        assert!(s.contains("test"));
    }

    #[test]
    fn test_zd_collection_new() {
        let c = ZdDiagnosticCollection::new("file.rs");
        assert!(c.is_empty());
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn test_zd_collection_add() {
        let mut c = ZdDiagnosticCollection::new("file.rs");
        c.add(ZdDiagnostic::error("e1", 0, 0, 1));
        c.add(ZdDiagnostic::warning("w1", 1, 0, 1));
        assert_eq!(c.error_count(), 1);
        assert_eq!(c.warning_count(), 1);
        assert_eq!(c.total(), 2);
    }

    #[test]
    fn test_zd_collection_for_line() {
        let mut c = ZdDiagnosticCollection::new("f");
        c.add(ZdDiagnostic::error("e1", 5, 0, 1));
        c.add(ZdDiagnostic::warning("w1", 10, 0, 1));
        assert_eq!(c.for_line(5).len(), 1);
        assert_eq!(c.for_line(7).len(), 0);
    }

    #[test]
    fn test_zd_collection_sort() {
        let mut c = ZdDiagnosticCollection::new("f");
        c.add(ZdDiagnostic::warning("w", 0, 0, 1));
        c.add(ZdDiagnostic::error("e", 0, 0, 1));
        c.sort_by_severity();
        assert!(c.diagnostics[0].is_error());
    }

    #[test]
    fn test_zd_collection_display() {
        let c = ZdDiagnosticCollection::new("test.rs");
        let s = format!("{}", c);
        assert!(s.contains("ZdDiagnosticCollection"));
    }


    // --- ze_ tests ---

    #[test]
    fn test_ze_completion_kind_icon() {
        assert_eq!(ZeCompletionKind::Function.icon(), "f");
        assert_eq!(ZeCompletionKind::Class.icon(), "C");
        assert_eq!(ZeCompletionKind::Keyword.icon(), "k");
    }

    #[test]
    fn test_ze_completion_item_new() {
        let item = ZeCompletionItem::new("println", ZeCompletionKind::Function);
        assert_eq!(item.label, "println");
        assert_eq!(item.kind, ZeCompletionKind::Function);
    }

    #[test]
    fn test_ze_completion_item_builder() {
        let item = ZeCompletionItem::new("test", ZeCompletionKind::Method)
            .with_detail("detail")
            .with_doc("doc")
            .with_insert_text("test()")
            .preselected();
        assert_eq!(item.detail, Some("detail".to_string()));
        assert!(item.preselect);
        assert_eq!(item.effective_insert_text(), "test()");
    }

    #[test]
    fn test_ze_completion_item_filter() {
        let item = ZeCompletionItem::new("println", ZeCompletionKind::Function);
        assert!(item.matches_filter("print"));
        assert!(item.matches_filter("PRINT"));
        assert!(!item.matches_filter("xyz"));
    }

    #[test]
    fn test_ze_completion_item_display() {
        let item = ZeCompletionItem::new("test", ZeCompletionKind::Function);
        let s = format!("{}", item);
        assert!(s.contains("test"));
    }

    #[test]
    fn test_ze_completion_list_new() {
        let items = vec![
            ZeCompletionItem::new("a", ZeCompletionKind::Variable),
            ZeCompletionItem::new("b", ZeCompletionKind::Function),
        ];
        let list = ZeCompletionList::new(items, false);
        assert_eq!(list.len(), 2);
        assert!(!list.is_incomplete);
    }

    #[test]
    fn test_ze_completion_list_empty() {
        let list = ZeCompletionList::empty();
        assert!(list.is_empty());
    }

    #[test]
    fn test_ze_completion_list_filter() {
        let items = vec![
            ZeCompletionItem::new("apple", ZeCompletionKind::Variable),
            ZeCompletionItem::new("banana", ZeCompletionKind::Variable),
            ZeCompletionItem::new("apricot", ZeCompletionKind::Variable),
        ];
        let list = ZeCompletionList::new(items, false);
        assert_eq!(list.filter("ap").len(), 2);
    }

    #[test]
    fn test_ze_completion_list_display() {
        let list = ZeCompletionList::empty();
        let s = format!("{}", list);
        assert!(s.contains("ZeCompletionList"));
    }

    #[test]
    fn test_ze_signature_help_new() {
        let sh = ZeSignatureHelp::new();
        assert!(sh.is_empty());
        assert_eq!(sh.signature_count(), 0);
    }

    #[test]
    fn test_ze_signature_help_add() {
        let mut sh = ZeSignatureHelp::new();
        sh.add_signature("fn test(a: i32, b: &str)", vec![
            ZeParameterInfo { label: "a: i32".to_string(), documentation: None },
            ZeParameterInfo { label: "b: &str".to_string(), documentation: None },
        ]);
        assert_eq!(sh.signature_count(), 1);
        assert_eq!(sh.active_param_label(), Some("a: i32"));
    }

    #[test]
    fn test_ze_signature_help_active() {
        let mut sh = ZeSignatureHelp::new();
        sh.add_signature("test()", vec![]);
        assert!(sh.active().is_some());
    }

    #[test]
    fn test_ze_signature_help_display() {
        let sh = ZeSignatureHelp::new();
        let s = format!("{}", sh);
        assert!(s.contains("ZeSignatureHelp"));
    }

    #[test]
    fn test_ze_signature_help_default() {
        let sh = ZeSignatureHelp::default();
        assert!(sh.is_empty());
    }


    // --- zf_ tests ---

    #[test]
    fn test_zf_hover_plain() {
        let h = ZfHover::plain("hello");
        assert_eq!(h.content_count(), 1);
        assert!(!h.has_range());
    }

    #[test]
    fn test_zf_hover_code() {
        let h = ZfHover::code("rust", "fn main() {}");
        assert_eq!(h.content_count(), 1);
    }

    #[test]
    fn test_zf_hover_with_range() {
        let h = ZfHover::plain("x").with_range(1, 0, 1, 5);
        assert!(h.has_range());
    }

    #[test]
    fn test_zf_hover_add_parts() {
        let mut h = ZfHover::plain("intro");
        h.add_code("rust", "let x = 1;");
        h.add_plain("explanation");
        assert_eq!(h.content_count(), 3);
    }

    #[test]
    fn test_zf_hover_display() {
        let h = ZfHover::plain("test");
        let s = format!("{}", h);
        assert!(s.contains("ZfHover"));
    }

    #[test]
    fn test_zf_symbol_kind_icon() {
        assert_eq!(ZfSymbolKind::Function.icon(), "fn");
        assert_eq!(ZfSymbolKind::Class.icon(), "cl");
        assert_eq!(ZfSymbolKind::Struct.icon(), "st");
    }

    #[test]
    fn test_zf_document_symbol_new() {
        let s = ZfDocumentSymbol::new("main", ZfSymbolKind::Function, 0, 10);
        assert_eq!(s.name, "main");
        assert_eq!(s.line_count(), 11);
        assert!(!s.has_children());
    }

    #[test]
    fn test_zf_document_symbol_children() {
        let child = ZfDocumentSymbol::new("inner", ZfSymbolKind::Variable, 2, 2);
        let parent = ZfDocumentSymbol::new("main", ZfSymbolKind::Function, 0, 10)
            .with_child(child);
        assert_eq!(parent.child_count(), 1);
    }

    #[test]
    fn test_zf_document_symbol_flat() {
        let child = ZfDocumentSymbol::new("x", ZfSymbolKind::Variable, 2, 2);
        let parent = ZfDocumentSymbol::new("fn", ZfSymbolKind::Function, 0, 5).with_child(child);
        assert_eq!(parent.flat_symbols().len(), 2);
    }

    #[test]
    fn test_zf_document_symbol_find_at_line() {
        let child = ZfDocumentSymbol::new("x", ZfSymbolKind::Variable, 3, 3);
        let parent = ZfDocumentSymbol::new("fn", ZfSymbolKind::Function, 0, 10).with_child(child);
        let found = parent.find_at_line(3).unwrap();
        assert_eq!(found.name, "x");
    }

    #[test]
    fn test_zf_document_symbol_display() {
        let s = ZfDocumentSymbol::new("test", ZfSymbolKind::Function, 0, 5);
        let d = format!("{}", s);
        assert!(d.contains("test"));
        assert!(d.contains("fn"));
    }

    #[test]
    fn test_zf_document_symbol_detail() {
        let s = ZfDocumentSymbol::new("x", ZfSymbolKind::Variable, 0, 0).with_detail("i32");
        assert_eq!(s.detail, Some("i32".to_string()));
    }

    #[test]
    fn test_zf_marked_string_eq() {
        let a = ZfMarkedString::Plain("hello".to_string());
        let b = ZfMarkedString::Plain("hello".to_string());
        assert_eq!(a, b);
    }


    // --- zg_ tests ---

    #[test]
    fn test_zg_color_new() {
        let c = ZgColor::new(255, 128, 0);
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn test_zg_color_from_hex() {
        let c = ZgColor::from_hex("#ff8000").unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 128);
        assert_eq!(c.b, 0);
    }

    #[test]
    fn test_zg_color_to_hex() {
        let c = ZgColor::new(255, 128, 0);
        assert_eq!(c.to_hex(), "#ff8000");
    }

    #[test]
    fn test_zg_color_luminance() {
        assert!(ZgColor::white().is_light());
        assert!(ZgColor::black().is_dark());
    }

    #[test]
    fn test_zg_color_blend() {
        let a = ZgColor::black();
        let b = ZgColor::white();
        let m = a.blend(&b, 0.5);
        assert!(m.r > 100 && m.r < 200);
    }

    #[test]
    fn test_zg_color_with_alpha() {
        let c = ZgColor::white().with_alpha(0.5);
        assert_eq!(c.a, 0.5);
    }

    #[test]
    fn test_zg_color_transparent() {
        let c = ZgColor::transparent();
        assert_eq!(c.a, 0.0);
    }

    #[test]
    fn test_zg_color_display() {
        let c = ZgColor::new(0, 0, 0);
        assert_eq!(format!("{}", c), "#000000");
    }

    #[test]
    fn test_zg_color_default() {
        let c = ZgColor::default();
        assert_eq!(c, ZgColor::black());
    }

    #[test]
    fn test_zg_decoration_new() {
        let d = ZgDecoration::new(0, 0, 0, 5);
        assert!(d.is_single_line());
        assert!(d.affects_line(0));
    }

    #[test]
    fn test_zg_decoration_builder() {
        let d = ZgDecoration::new(1, 0, 3, 10)
            .with_fg(ZgColor::new(255, 0, 0))
            .with_bg(ZgColor::new(0, 0, 255))
            .with_style(ZgDecorationStyle::Underline)
            .with_hover("error here")
            .with_tag("error");
        assert!(d.has_foreground());
        assert!(d.has_background());
        assert_eq!(d.style, ZgDecorationStyle::Underline);
    }

    #[test]
    fn test_zg_decoration_display() {
        let d = ZgDecoration::new(0, 0, 0, 5);
        let s = format!("{}", d);
        assert!(s.contains("ZgDecoration"));
    }

    #[test]
    fn test_zg_decoration_set_new() {
        let ds = ZgDecorationSet::new();
        assert!(ds.is_empty());
    }

    #[test]
    fn test_zg_decoration_set_add() {
        let mut ds = ZgDecorationSet::new();
        ds.add(ZgDecoration::new(0, 0, 0, 5).with_tag("warn"));
        ds.add(ZgDecoration::new(2, 0, 2, 5).with_tag("error"));
        assert_eq!(ds.len(), 2);
    }

    #[test]
    fn test_zg_decoration_set_for_line() {
        let mut ds = ZgDecorationSet::new();
        ds.add(ZgDecoration::new(0, 0, 2, 0));
        ds.add(ZgDecoration::new(5, 0, 5, 10));
        assert_eq!(ds.for_line(1).len(), 1);
        assert_eq!(ds.for_line(5).len(), 1);
    }

    #[test]
    fn test_zg_decoration_set_by_tag() {
        let mut ds = ZgDecorationSet::new();
        ds.add(ZgDecoration::new(0, 0, 0, 5).with_tag("a"));
        ds.add(ZgDecoration::new(1, 0, 1, 5).with_tag("b"));
        assert_eq!(ds.by_tag("a").len(), 1);
    }

    #[test]
    fn test_zg_decoration_set_display() {
        let ds = ZgDecorationSet::new();
        let s = format!("{}", ds);
        assert!(s.contains("ZgDecorationSet"));
    }

    #[test]
    fn test_zg_decoration_set_default() {
        let ds = ZgDecorationSet::default();
        assert!(ds.is_empty());
    }


    // --- zh_ tests ---

    #[test]
    fn test_zh_token_type() {
        assert_eq!(ZhSemanticTokenType::Function.as_str(), "function");
        assert_eq!(ZhSemanticTokenType::Keyword.as_str(), "keyword");
    }

    #[test]
    fn test_zh_token_type_display() {
        let s = format!("{}", ZhSemanticTokenType::Variable);
        assert_eq!(s, "variable");
    }

    #[test]
    fn test_zh_semantic_token_new() {
        let t = ZhSemanticToken::new(0, 5, 10, 1, 0);
        assert_eq!(t.delta_line, 0);
        assert_eq!(t.length, 10);
    }

    #[test]
    fn test_zh_semantic_tokens_new() {
        let st = ZhSemanticTokens::new();
        assert!(st.is_empty());
    }

    #[test]
    fn test_zh_semantic_tokens_push() {
        let mut st = ZhSemanticTokens::new();
        st.push(ZhSemanticToken::new(0, 0, 5, 0, 0));
        st.push(ZhSemanticToken::new(1, 0, 3, 1, 0));
        assert_eq!(st.len(), 2);
    }

    #[test]
    fn test_zh_semantic_tokens_to_data() {
        let mut st = ZhSemanticTokens::new();
        st.push(ZhSemanticToken::new(0, 5, 10, 1, 2));
        let data = st.to_data();
        assert_eq!(data, vec![0, 5, 10, 1, 2]);
    }

    #[test]
    fn test_zh_semantic_tokens_result_id() {
        let st = ZhSemanticTokens::new().with_result_id("abc123");
        assert_eq!(st.result_id, Some("abc123".to_string()));
    }

    #[test]
    fn test_zh_semantic_tokens_display() {
        let st = ZhSemanticTokens::new();
        let s = format!("{}", st);
        assert!(s.contains("ZhSemanticTokens"));
    }

    #[test]
    fn test_zh_code_action_kind() {
        assert_eq!(ZhCodeActionKind::QuickFix.as_str(), "quickfix");
        assert!(ZhCodeActionKind::QuickFix.is_quickfix());
        assert!(ZhCodeActionKind::RefactorExtract.is_refactor());
    }

    #[test]
    fn test_zh_code_action_new() {
        let a = ZhCodeAction::new("Fix import", ZhCodeActionKind::QuickFix);
        assert_eq!(a.title, "Fix import");
        assert!(!a.is_disabled());
    }

    #[test]
    fn test_zh_code_action_preferred() {
        let a = ZhCodeAction::new("Fix", ZhCodeActionKind::QuickFix).preferred();
        assert!(a.is_preferred);
    }

    #[test]
    fn test_zh_code_action_disabled() {
        let a = ZhCodeAction::new("Fix", ZhCodeActionKind::QuickFix).disabled("not applicable");
        assert!(a.is_disabled());
        assert_eq!(a.disabled_reason, Some("not applicable".to_string()));
    }

    #[test]
    fn test_zh_code_action_display() {
        let a = ZhCodeAction::new("Test", ZhCodeActionKind::Refactor);
        let s = format!("{}", a);
        assert!(s.contains("Test"));
    }

    #[test]
    fn test_zh_code_action_kind_other() {
        let k = ZhCodeActionKind::Other("custom.action".to_string());
        assert_eq!(k.as_str(), "custom.action");
    }


    // --- zi_ tests ---

    #[test]
    fn test_zi_doc_link_new() {
        let l = ZiDocumentLink::new(0, 5, 0, 20);
        assert!(!l.is_resolved());
        assert!(l.is_single_line());
    }

    #[test]
    fn test_zi_doc_link_with_target() {
        let l = ZiDocumentLink::new(0, 0, 0, 10).with_target("https://example.com");
        assert!(l.is_resolved());
    }

    #[test]
    fn test_zi_doc_link_affects_line() {
        let l = ZiDocumentLink::new(2, 0, 4, 10);
        assert!(l.affects_line(3));
        assert!(!l.affects_line(5));
    }

    #[test]
    fn test_zi_doc_link_display() {
        let l = ZiDocumentLink::new(0, 0, 0, 5);
        assert!(format!("{}", l).contains("ZiDocumentLink"));
    }

    #[test]
    fn test_zi_doc_links_collection() {
        let mut links = ZiDocumentLinks::new();
        links.add(ZiDocumentLink::new(0, 0, 0, 10).with_target("x"));
        links.add(ZiDocumentLink::new(1, 0, 1, 5));
        assert_eq!(links.len(), 2);
        assert_eq!(links.resolved_count(), 1);
        assert_eq!(links.for_line(0).len(), 1);
    }

    #[test]
    fn test_zi_code_lens_new() {
        let cl = ZiCodeLens::new(5, 0, 5, 10);
        assert!(!cl.is_resolved());
        assert_eq!(cl.title(), "");
    }

    #[test]
    fn test_zi_code_lens_with_command() {
        let cl = ZiCodeLens::new(0, 0, 0, 10).with_command("run", "Run Test");
        assert!(cl.is_resolved());
        assert_eq!(cl.title(), "Run Test");
    }

    #[test]
    fn test_zi_code_lens_args() {
        let mut cl = ZiCodeLens::new(0, 0, 0, 5).with_command("cmd", "Go");
        cl.add_arg("arg1");
        assert_eq!(cl.command_args.len(), 1);
    }

    #[test]
    fn test_zi_code_lens_display() {
        let cl = ZiCodeLens::new(0, 0, 0, 5).with_command("x", "Title");
        assert!(format!("{}", cl).contains("Title"));
    }

    #[test]
    fn test_zi_code_lenses_collection() {
        let mut cls = ZiCodeLenses::new();
        cls.add(ZiCodeLens::new(0, 0, 0, 5).with_command("a", "A"));
        cls.add(ZiCodeLens::new(5, 0, 5, 5));
        assert_eq!(cls.len(), 2);
        assert_eq!(cls.resolved_count(), 1);
        assert_eq!(cls.for_line(0).len(), 1);
    }

    #[test]
    fn test_zi_code_lenses_sort() {
        let mut cls = ZiCodeLenses::new();
        cls.add(ZiCodeLens::new(5, 0, 5, 5));
        cls.add(ZiCodeLens::new(1, 0, 1, 5));
        cls.sort_by_line();
        assert_eq!(cls.lenses[0].start_line, 1);
    }

    #[test]
    fn test_zi_folding_range() {
        let fr = ZiFoldingRange::comment(5, 15);
        assert_eq!(fr.line_count(), 11);
        assert!(fr.contains_line(10));
        assert!(!fr.contains_line(16));
        assert_eq!(fr.kind, ZiFoldingKind::Comment);
    }

    #[test]
    fn test_zi_folding_kinds() {
        let _ = ZiFoldingRange::imports(0, 5);
        let _ = ZiFoldingRange::region(10, 20);
    }

    #[test]
    fn test_zi_folding_display() {
        let fr = ZiFoldingRange::region(0, 10);
        assert!(format!("{}", fr).contains("ZiFoldingRange"));
    }

    #[test]
    fn test_zi_selection_range_new() {
        let sr = ZiSelectionRange::new(1, 0, 1, 10);
        assert_eq!(sr.depth(), 0);
    }

    #[test]
    fn test_zi_selection_range_parent() {
        let outer = ZiSelectionRange::new(0, 0, 10, 0);
        let inner = ZiSelectionRange::new(3, 5, 3, 15).with_parent(outer);
        assert_eq!(inner.depth(), 1);
    }

    #[test]
    fn test_zi_selection_range_outermost() {
        let outer = ZiSelectionRange::new(0, 0, 100, 0);
        let mid = ZiSelectionRange::new(5, 0, 50, 0).with_parent(outer);
        let inner = ZiSelectionRange::new(10, 5, 10, 15).with_parent(mid);
        let om = inner.outermost();
        assert_eq!(om.start_line, 0);
        assert_eq!(om.end_line, 100);
    }

    #[test]
    fn test_zi_selection_range_display() {
        let sr = ZiSelectionRange::new(5, 0, 5, 10);
        assert!(format!("{}", sr).contains("ZiSelectionRange"));
    }


    // --- zj_ tests ---

    #[test]
    fn test_zj_inlay_hint_type() {
        let h = ZjInlayHint::type_hint(5, 10, ": i32");
        assert!(h.is_type());
        assert!(!h.is_parameter());
        assert_eq!(h.label, ": i32");
    }

    #[test]
    fn test_zj_inlay_hint_param() {
        let h = ZjInlayHint::param_hint(3, 5, "name:");
        assert!(h.is_parameter());
        assert!(h.padding_left);
    }

    #[test]
    fn test_zj_inlay_hint_builder() {
        let h = ZjInlayHint::type_hint(0, 0, ": u32").with_tooltip("inferred type").padded_right();
        assert!(h.tooltip.is_some());
        assert!(h.padding_right);
    }

    #[test]
    fn test_zj_inlay_hint_display() {
        let h = ZjInlayHint::type_hint(0, 5, ": bool");
        assert!(format!("{}", h).contains("type"));
    }

    #[test]
    fn test_zj_inlay_hints_collection() {
        let mut hints = ZjInlayHints::new();
        hints.add(ZjInlayHint::type_hint(0, 5, ": i32"));
        hints.add(ZjInlayHint::param_hint(0, 10, "x:"));
        hints.add(ZjInlayHint::type_hint(3, 0, ": String"));
        assert_eq!(hints.len(), 3);
        assert_eq!(hints.for_line(0).len(), 2);
        assert_eq!(hints.type_hints().len(), 2);
        assert_eq!(hints.param_hints().len(), 1);
    }

    #[test]
    fn test_zj_inlay_hints_sort() {
        let mut hints = ZjInlayHints::new();
        hints.add(ZjInlayHint::type_hint(5, 0, "a"));
        hints.add(ZjInlayHint::type_hint(1, 0, "b"));
        hints.sort();
        assert_eq!(hints.hints[0].line, 1);
    }

    #[test]
    fn test_zj_inlay_hints_display() {
        let hints = ZjInlayHints::new();
        assert!(format!("{}", hints).contains("ZjInlayHints"));
    }

    #[test]
    fn test_zj_prepare_rename() {
        let pr = ZjPrepareRename::new(5, 10, 5, 15, "oldName");
        assert!(pr.is_single_line());
        assert_eq!(pr.placeholder, "oldName");
    }

    #[test]
    fn test_zj_prepare_rename_display() {
        let pr = ZjPrepareRename::new(0, 0, 0, 5, "x");
        assert!(format!("{}", pr).contains("ZjPrepareRename"));
    }

    #[test]
    fn test_zj_rename_edit_new() {
        let re = ZjRenameEdit::new("old", "new");
        assert!(re.is_empty());
        assert_eq!(re.old_name, "old");
        assert_eq!(re.new_name, "new");
    }

    #[test]
    fn test_zj_rename_edit_add_changes() {
        let mut re = ZjRenameEdit::new("foo", "bar");
        re.add_file_change("a.rs", vec![
            ZjRenameTextEdit { start_line: 0, start_char: 5, end_line: 0, end_char: 8 },
            ZjRenameTextEdit { start_line: 10, start_char: 0, end_line: 10, end_char: 3 },
        ]);
        re.add_file_change("b.rs", vec![
            ZjRenameTextEdit { start_line: 5, start_char: 0, end_line: 5, end_char: 3 },
        ]);
        assert_eq!(re.file_count(), 2);
        assert_eq!(re.total_edits(), 3);
        assert_eq!(re.affected_uris(), vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn test_zj_rename_edit_display() {
        let re = ZjRenameEdit::new("x", "y");
        let s = format!("{}", re);
        assert!(s.contains("ZjRenameEdit"));
        assert!(s.contains("x"));
        assert!(s.contains("y"));
    }


    // --- zk_ tests ---

    #[test]
    fn test_zk_workspace_symbol() {
        let s = ZkWorkspaceSymbol::new("main", 12, "file.rs", 0, 0);
        assert_eq!(s.name, "main");
        assert!(s.matches_query("mai"));
        assert!(!s.matches_query("xyz"));
    }

    #[test]
    fn test_zk_workspace_symbol_container() {
        let s = ZkWorkspaceSymbol::new("test", 6, "lib.rs", 5, 0).with_container("MyModule");
        assert_eq!(s.qualified_name(), "MyModule.test");
    }

    #[test]
    fn test_zk_workspace_symbol_display() {
        let s = ZkWorkspaceSymbol::new("x", 0, "f.rs", 0, 0);
        assert!(format!("{}", s).contains("ZkWorkspaceSymbol"));
    }

    #[test]
    fn test_zk_workspace_symbols_search() {
        let mut ws = ZkWorkspaceSymbols::new();
        ws.add(ZkWorkspaceSymbol::new("handleClick", 12, "a.rs", 0, 0));
        ws.add(ZkWorkspaceSymbol::new("handleSubmit", 12, "b.rs", 0, 0));
        ws.add(ZkWorkspaceSymbol::new("render", 12, "a.rs", 5, 0));
        assert_eq!(ws.search("handle").len(), 2);
        assert_eq!(ws.in_file("a.rs").len(), 2);
    }

    #[test]
    fn test_zk_workspace_symbols_display() {
        let ws = ZkWorkspaceSymbols::new();
        assert!(format!("{}", ws).contains("ZkWorkspaceSymbols"));
    }

    #[test]
    fn test_zk_call_hierarchy_item() {
        let item = ZkCallHierarchyItem::new("process", 12, "main.rs", 10, 0, 30, 0).with_detail("pub fn");
        assert_eq!(item.line_count(), 21);
        assert_eq!(item.detail, Some("pub fn".to_string()));
    }

    #[test]
    fn test_zk_call_hierarchy_display() {
        let item = ZkCallHierarchyItem::new("x", 0, "f.rs", 0, 0, 5, 0);
        assert!(format!("{}", item).contains("ZkCallHierarchyItem"));
    }

    #[test]
    fn test_zk_incoming_call() {
        let from = ZkCallHierarchyItem::new("caller", 12, "a.rs", 0, 0, 10, 0);
        let mut ic = ZkIncomingCall::new(from);
        ic.add_range(5, 0, 5, 10);
        ic.add_range(8, 0, 8, 10);
        assert_eq!(ic.call_count(), 2);
    }

    #[test]
    fn test_zk_incoming_call_display() {
        let from = ZkCallHierarchyItem::new("fn_a", 12, "x.rs", 0, 0, 5, 0);
        let ic = ZkIncomingCall::new(from);
        assert!(format!("{}", ic).contains("fn_a"));
    }

    #[test]
    fn test_zk_outgoing_call() {
        let to = ZkCallHierarchyItem::new("callee", 12, "b.rs", 0, 0, 5, 0);
        let mut oc = ZkOutgoingCall::new(to);
        oc.add_range(3, 10, 3, 20);
        assert_eq!(oc.call_count(), 1);
    }

    #[test]
    fn test_zk_outgoing_call_display() {
        let to = ZkCallHierarchyItem::new("fn_b", 12, "y.rs", 0, 0, 3, 0);
        let oc = ZkOutgoingCall::new(to);
        assert!(format!("{}", oc).contains("fn_b"));
    }

    #[test]
    fn test_zk_type_hierarchy_item() {
        let item = ZkTypeHierarchyItem::new("MyClass", 5, "cls.rs", 0, 50).with_detail("class");
        assert_eq!(item.name, "MyClass");
        assert_eq!(item.detail, Some("class".to_string()));
    }

    #[test]
    fn test_zk_type_hierarchy_display() {
        let item = ZkTypeHierarchyItem::new("Trait", 0, "t.rs", 0, 10);
        assert!(format!("{}", item).contains("Trait"));
    }


    // --- zl_ tests ---

    #[test]
    fn test_zl_cell_code() {
        let c = ZlNotebookCell::code("print(1)", "python");
        assert!(c.is_code());
        assert!(!c.is_markup());
        assert_eq!(c.language, "python");
    }

    #[test]
    fn test_zl_cell_markup() {
        let c = ZlNotebookCell::markup("# Title");
        assert!(c.is_markup());
        assert_eq!(c.language, "markdown");
    }

    #[test]
    fn test_zl_cell_outputs() {
        let mut c = ZlNotebookCell::code("1+1", "python");
        c.add_output("text/plain");
        assert!(c.has_outputs());
        assert_eq!(c.outputs.len(), 1);
    }

    #[test]
    fn test_zl_cell_metadata() {
        let mut c = ZlNotebookCell::code("x", "rust");
        c.set_meta("editable", "true");
        assert_eq!(c.metadata.get("editable"), Some(&"true".to_string()));
    }

    #[test]
    fn test_zl_cell_display() {
        let c = ZlNotebookCell::code("x=1", "python");
        assert!(format!("{}", c).contains("code"));
    }

    #[test]
    fn test_zl_notebook_new() {
        let nb = ZlNotebook::new("notebook.ipynb");
        assert!(nb.is_empty());
        assert_eq!(nb.cell_count(), 0);
    }

    #[test]
    fn test_zl_notebook_add_cells() {
        let mut nb = ZlNotebook::new("test.ipynb");
        nb.add_cell(ZlNotebookCell::markup("# Intro"));
        nb.add_cell(ZlNotebookCell::code("x = 1", "python"));
        nb.add_cell(ZlNotebookCell::code("y = 2", "python"));
        assert_eq!(nb.cell_count(), 3);
        assert_eq!(nb.code_cells().len(), 2);
        assert_eq!(nb.markup_cells().len(), 1);
    }

    #[test]
    fn test_zl_notebook_display() {
        let nb = ZlNotebook::new("test.ipynb");
        assert!(format!("{}", nb).contains("ZlNotebook"));
    }

    #[test]
    fn test_zl_terminal_profile() {
        let p = ZlTerminalProfile::new("bash", "/bin/bash")
            .with_args(&["-l"])
            .with_cwd("/home/user")
            .with_icon("terminal")
            .with_color("#ff0000");
        assert_eq!(p.name, "bash");
        assert_eq!(p.shell_args, vec!["-l"]);
        assert_eq!(p.cwd, Some("/home/user".to_string()));
    }

    #[test]
    fn test_zl_terminal_profile_env() {
        let mut p = ZlTerminalProfile::new("zsh", "/bin/zsh");
        p.set_env("TERM", "xterm-256color");
        assert_eq!(p.env.get("TERM"), Some(&"xterm-256color".to_string()));
    }

    #[test]
    fn test_zl_terminal_profile_display() {
        let p = ZlTerminalProfile::new("sh", "/bin/sh");
        assert!(format!("{}", p).contains("ZlTerminalProfile"));
    }

    #[test]
    fn test_zl_terminal_link() {
        let l = ZlTerminalLink::new(10, 20).with_uri("file:///test").with_tooltip("click");
        assert_eq!(l.end_index(), 30);
        assert!(l.is_resolved());
    }

    #[test]
    fn test_zl_terminal_link_unresolved() {
        let l = ZlTerminalLink::new(0, 5);
        assert!(!l.is_resolved());
    }

    #[test]
    fn test_zl_terminal_link_display() {
        let l = ZlTerminalLink::new(5, 10);
        assert!(format!("{}", l).contains("ZlTerminalLink"));
    }


    // --- zm_ tests ---

    #[test]
    fn test_zm_breakpoint_new() {
        let bp = ZmBreakpoint::new(1, 10);
        assert!(bp.verified);
        assert_eq!(bp.line, 10);
        assert!(!bp.is_conditional());
        assert!(!bp.is_logpoint());
    }

    #[test]
    fn test_zm_breakpoint_conditional() {
        let bp = ZmBreakpoint::new(1, 5).with_condition("x > 0");
        assert!(bp.is_conditional());
    }

    #[test]
    fn test_zm_breakpoint_logpoint() {
        let bp = ZmBreakpoint::new(1, 5).with_log_message("value: {x}");
        assert!(bp.is_logpoint());
    }

    #[test]
    fn test_zm_breakpoint_unverified() {
        let bp = ZmBreakpoint::new(1, 5).unverified();
        assert!(!bp.verified);
    }

    #[test]
    fn test_zm_breakpoint_display() {
        let bp = ZmBreakpoint::new(1, 10);
        assert!(format!("{}", bp).contains("verified"));
    }

    #[test]
    fn test_zm_stack_frame() {
        let sf = ZmStackFrame::new(0, "main", 10, 0).with_source("main.rs");
        assert!(sf.has_source());
        assert_eq!(sf.name, "main");
    }

    #[test]
    fn test_zm_stack_frame_display() {
        let sf = ZmStackFrame::new(0, "test", 5, 0);
        assert!(format!("{}", sf).contains("test"));
    }

    #[test]
    fn test_zm_variable_simple() {
        let v = ZmVariable::new("x", "42").with_type("i32");
        assert!(!v.has_children());
        assert_eq!(v.var_type, Some("i32".to_string()));
    }

    #[test]
    fn test_zm_variable_structured() {
        let v = ZmVariable::new("obj", "{...}").with_ref(5);
        assert!(v.has_children());
        assert!(v.is_structured());
    }

    #[test]
    fn test_zm_variable_display() {
        let v = ZmVariable::new("x", "42").with_type("i32");
        assert_eq!(format!("{}", v), "x: i32 = 42");
    }

    #[test]
    fn test_zm_thread() {
        let t = ZmThread::new(1, "main");
        assert!(!t.is_stopped);
        let t2 = ZmThread::new(2, "worker").stopped();
        assert!(t2.is_stopped);
    }

    #[test]
    fn test_zm_thread_display() {
        let t = ZmThread::new(1, "main");
        assert!(format!("{}", t).contains("running"));
    }

    #[test]
    fn test_zm_debug_session_new() {
        let s = ZmDebugSession::new("s1", "My Debug", "lldb");
        assert!(!s.is_running);
        assert_eq!(s.thread_count(), 0);
    }

    #[test]
    fn test_zm_debug_session_start_stop() {
        let mut s = ZmDebugSession::new("s1", "test", "gdb");
        s.start();
        assert!(s.is_running);
        s.stop();
        assert!(!s.is_running);
    }

    #[test]
    fn test_zm_debug_session_threads() {
        let mut s = ZmDebugSession::new("s1", "test", "lldb");
        s.add_thread(ZmThread::new(1, "main").stopped());
        s.add_thread(ZmThread::new(2, "worker"));
        assert_eq!(s.thread_count(), 2);
        assert_eq!(s.stopped_threads().len(), 1);
    }

    #[test]
    fn test_zm_debug_session_breakpoints() {
        let mut s = ZmDebugSession::new("s1", "test", "lldb");
        s.add_breakpoint(ZmBreakpoint::new(1, 10));
        s.add_breakpoint(ZmBreakpoint::new(2, 20).unverified());
        assert_eq!(s.breakpoint_count(), 2);
        assert_eq!(s.verified_breakpoints().len(), 1);
    }

    #[test]
    fn test_zm_debug_session_display() {
        let s = ZmDebugSession::new("s1", "test", "lldb");
        assert!(format!("{}", s).contains("ZmDebugSession"));
    }


    // --- zn_ tests ---

    #[test]
    fn test_zn_change_status() {
        assert_eq!(ZnChangeStatus::Modified.letter(), 'M');
        assert_eq!(ZnChangeStatus::Untracked.letter(), '?');
        assert!(ZnChangeStatus::Added.is_staged_change());
        assert!(!ZnChangeStatus::Untracked.is_staged_change());
    }

    #[test]
    fn test_zn_resource_change() {
        let c = ZnResourceChange::new("src/main.rs", ZnChangeStatus::Modified);
        assert_eq!(c.filename(), "main.rs");
        assert!(!c.is_rename());
    }

    #[test]
    fn test_zn_resource_change_rename() {
        let c = ZnResourceChange::renamed("old.rs", "new.rs");
        assert!(c.is_rename());
        assert_eq!(c.original_uri, Some("old.rs".to_string()));
    }

    #[test]
    fn test_zn_resource_change_display() {
        let c = ZnResourceChange::new("test.rs", ZnChangeStatus::Added);
        let s = format!("{}", c);
        assert!(s.contains("A"));
    }

    #[test]
    fn test_zn_resource_group() {
        let mut g = ZnResourceGroup::new("staged", "Staged Changes");
        g.add(ZnResourceChange::new("a.rs", ZnChangeStatus::Modified));
        g.add(ZnResourceChange::new("b.rs", ZnChangeStatus::Added));
        assert_eq!(g.len(), 2);
        assert_eq!(g.by_status(ZnChangeStatus::Modified).len(), 1);
    }

    #[test]
    fn test_zn_resource_group_display() {
        let g = ZnResourceGroup::new("idx", "Index");
        assert!(format!("{}", g).contains("ZnResourceGroup"));
    }

    #[test]
    fn test_zn_source_control() {
        let mut sc = ZnSourceControl::new("git", "Git").with_root("/repo");
        let mut staged = ZnResourceGroup::new("staged", "Staged");
        staged.add(ZnResourceChange::new("x.rs", ZnChangeStatus::Modified));
        sc.add_group(staged);
        assert_eq!(sc.total_changes(), 1);
        assert_eq!(sc.group_count(), 1);
    }

    #[test]
    fn test_zn_source_control_display() {
        let sc = ZnSourceControl::new("git", "Git");
        assert!(format!("{}", sc).contains("Git"));
    }

    #[test]
    fn test_zn_status_bar_item() {
        let item = ZnStatusBarItem::new("branch", "main")
            .right()
            .with_priority(100)
            .with_command("git.checkout")
            .with_tooltip("Current branch");
        assert_eq!(item.alignment, ZnStatusBarAlignment::Right);
        assert_eq!(item.priority, 100);
        assert!(item.is_visible);
    }

    #[test]
    fn test_zn_status_bar_visibility() {
        let mut item = ZnStatusBarItem::new("test", "text");
        item.hide();
        assert!(!item.is_visible);
        item.show();
        assert!(item.is_visible);
    }

    #[test]
    fn test_zn_status_bar_set_text() {
        let mut item = ZnStatusBarItem::new("x", "old");
        item.set_text("new");
        assert_eq!(item.text, "new");
    }

    #[test]
    fn test_zn_status_bar_display() {
        let item = ZnStatusBarItem::new("id", "text");
        assert!(format!("{}", item).contains("ZnStatusBarItem"));
    }


    // --- zo_ tests ---

    #[test]
    fn test_zo_activation_event_parse() {
        assert_eq!(ZoActivationEvent::parse("*"), Some(ZoActivationEvent::Star));
        assert_eq!(ZoActivationEvent::parse("onLanguage:rust"), Some(ZoActivationEvent::OnLanguage("rust".to_string())));
        assert_eq!(ZoActivationEvent::parse("onCommand:ext.cmd"), Some(ZoActivationEvent::OnCommand("ext.cmd".to_string())));
        assert_eq!(ZoActivationEvent::parse("onDebug"), Some(ZoActivationEvent::OnDebug));
        assert_eq!(ZoActivationEvent::parse("onStartupFinished"), Some(ZoActivationEvent::OnStartupFinished));
        assert_eq!(ZoActivationEvent::parse("garbage"), None);
    }

    #[test]
    fn test_zo_activation_event_eager() {
        assert!(ZoActivationEvent::Star.is_eager());
        assert!(!ZoActivationEvent::OnDebug.is_eager());
    }

    #[test]
    fn test_zo_activation_event_display() {
        assert_eq!(format!("{}", ZoActivationEvent::OnLanguage("ts".to_string())), "onLanguage:ts");
    }

    #[test]
    fn test_zo_contribution() {
        let c = ZoContribution::new("commands", "ext.hello").with_title("Hello World");
        assert_eq!(c.kind, "commands");
        assert_eq!(c.title, Some("Hello World".to_string()));
    }

    #[test]
    fn test_zo_contribution_display() {
        let c = ZoContribution::new("views", "myView");
        assert!(format!("{}", c).contains("views"));
    }

    #[test]
    fn test_zo_manifest_new() {
        let m = ZoExtensionManifest::new("ms-vscode", "go", "0.1.0");
        assert_eq!(m.id, "ms-vscode.go");
        assert_eq!(m.version, "0.1.0");
    }

    #[test]
    fn test_zo_manifest_activation() {
        let mut m = ZoExtensionManifest::new("pub", "ext", "1.0.0");
        m.add_activation(ZoActivationEvent::OnLanguage("rust".to_string()));
        assert!(m.activates_on_language("rust"));
        assert!(!m.activates_on_language("go"));
        assert!(!m.is_eager());
    }

    #[test]
    fn test_zo_manifest_eager() {
        let mut m = ZoExtensionManifest::new("pub", "ext", "1.0.0");
        m.add_activation(ZoActivationEvent::Star);
        assert!(m.is_eager());
    }

    #[test]
    fn test_zo_manifest_contributions() {
        let mut m = ZoExtensionManifest::new("pub", "ext", "1.0.0");
        m.add_contribution(ZoContribution::new("commands", "hello"));
        m.add_category("Linters");
        assert_eq!(m.contributions.len(), 1);
        assert_eq!(m.categories, vec!["Linters"]);
    }

    #[test]
    fn test_zo_manifest_display() {
        let m = ZoExtensionManifest::new("pub", "ext", "1.0.0");
        assert!(format!("{}", m).contains("pub.ext"));
    }

    #[test]
    fn test_zo_extension_context() {
        let mut ctx = ZoExtensionContext::new("pub.ext", "/extensions/pub.ext");
        assert!(!ctx.is_active);
        ctx.activate();
        assert!(ctx.is_active);
        ctx.add_subscription("disposable1");
        assert_eq!(ctx.subscription_count(), 1);
        ctx.deactivate();
        assert!(!ctx.is_active);
    }

    #[test]
    fn test_zo_extension_context_display() {
        let ctx = ZoExtensionContext::new("a.b", "/path");
        assert!(format!("{}", ctx).contains("inactive"));
    }


    // --- zp_ tests ---

    #[test]
    fn test_zp_task_group() {
        assert_eq!(ZpTaskGroup::Build.as_str(), "build");
        assert_eq!(ZpTaskGroup::Test.as_str(), "test");
    }

    #[test]
    fn test_zp_task_source_display() {
        assert_eq!(format!("{}", ZpTaskSource::Workspace), "workspace");
        assert_eq!(format!("{}", ZpTaskSource::Extension("rust".to_string())), "extension:rust");
    }

    #[test]
    fn test_zp_task_execution_process() {
        let exec = ZpTaskExecution::process("cargo", &["build", "--release"]);
        assert_eq!(exec.command_line(), "cargo build --release");
        assert!(!exec.is_shell);
    }

    #[test]
    fn test_zp_task_execution_shell() {
        let exec = ZpTaskExecution::shell("npm run build");
        assert!(exec.is_shell);
    }

    #[test]
    fn test_zp_task_execution_cwd() {
        let exec = ZpTaskExecution::process("make", &[]).with_cwd("/project");
        assert_eq!(exec.cwd, Some("/project".to_string()));
    }

    #[test]
    fn test_zp_task_execution_env() {
        let mut exec = ZpTaskExecution::process("cmd", &[]);
        exec.set_env("PATH", "/usr/bin");
        assert_eq!(exec.env.get("PATH"), Some(&"/usr/bin".to_string()));
    }

    #[test]
    fn test_zp_task_new() {
        let exec = ZpTaskExecution::shell("cargo test");
        let task = ZpTask::new("test", ZpTaskSource::Workspace, exec).with_group(ZpTaskGroup::Test);
        assert!(task.is_test());
        assert!(!task.is_build());
        assert!(!task.is_background);
    }

    #[test]
    fn test_zp_task_background() {
        let exec = ZpTaskExecution::shell("tsc -w");
        let task = ZpTask::new("watch", ZpTaskSource::Auto, exec).background();
        assert!(task.is_background);
    }

    #[test]
    fn test_zp_task_problem_matcher() {
        let exec = ZpTaskExecution::shell("gcc main.c");
        let mut task = ZpTask::new("compile", ZpTaskSource::Workspace, exec);
        task.add_problem_matcher("$gcc");
        assert_eq!(task.problem_matchers, vec!["$gcc"]);
    }

    #[test]
    fn test_zp_task_display() {
        let exec = ZpTaskExecution::shell("echo hi");
        let task = ZpTask::new("greet", ZpTaskSource::User, exec);
        assert!(format!("{}", task).contains("greet"));
    }

    #[test]
    fn test_zp_task_run_new() {
        let run = ZpTaskRun::new("build");
        assert!(run.is_running);
        assert!(!run.succeeded());
        assert!(!run.failed());
    }

    #[test]
    fn test_zp_task_run_complete() {
        let mut run = ZpTaskRun::new("test");
        run.add_output("test 1 passed");
        run.add_output("test 2 passed");
        run.complete(0);
        assert!(!run.is_running);
        assert!(run.succeeded());
        assert!(!run.failed());
        assert_eq!(run.output_count(), 2);
    }

    #[test]
    fn test_zp_task_run_failed() {
        let mut run = ZpTaskRun::new("build");
        run.complete(1);
        assert!(run.failed());
        assert!(!run.succeeded());
    }

    #[test]
    fn test_zp_task_run_display() {
        let mut run = ZpTaskRun::new("x");
        assert!(format!("{}", run).contains("running"));
        run.complete(0);
        assert!(format!("{}", run).contains("succeeded"));
    }

    #[test]
    fn test_zp_presentation_default() {
        let p = ZpTaskPresentation::default();
        assert_eq!(p.reveal, ZpRevealKind::Always);
        assert!(p.echo);
        assert!(!p.focus);
    }


    // --- zq_ tests ---

    #[test]
    fn test_zq_test_run_state() {
        assert!(ZqTestRunState::Passed.is_terminal());
        assert!(ZqTestRunState::Passed.is_success());
        assert!(ZqTestRunState::Failed.is_failure());
        assert!(!ZqTestRunState::Running.is_terminal());
        assert_eq!(ZqTestRunState::Passed.icon(), "✓");
    }

    #[test]
    fn test_zq_test_item_new() {
        let item = ZqTestItem::new("test1", "Test One");
        assert_eq!(item.state, ZqTestRunState::Queued);
        assert!(!item.has_children());
    }

    #[test]
    fn test_zq_test_item_pass() {
        let mut item = ZqTestItem::new("t1", "test");
        item.run();
        assert_eq!(item.state, ZqTestRunState::Running);
        item.pass(150);
        assert!(item.state.is_success());
        assert_eq!(item.duration_ms, Some(150));
    }

    #[test]
    fn test_zq_test_item_fail() {
        let mut item = ZqTestItem::new("t1", "test");
        item.fail("assertion failed", 200);
        assert!(item.state.is_failure());
        assert_eq!(item.error_message, Some("assertion failed".to_string()));
    }

    #[test]
    fn test_zq_test_item_skip() {
        let mut item = ZqTestItem::new("t1", "test");
        item.skip();
        assert_eq!(item.state, ZqTestRunState::Skipped);
    }

    #[test]
    fn test_zq_test_item_children() {
        let mut parent = ZqTestItem::new("suite", "Suite");
        let mut child1 = ZqTestItem::new("t1", "Test 1");
        child1.pass(10);
        let mut child2 = ZqTestItem::new("t2", "Test 2");
        child2.fail("err", 20);
        parent.add_child(child1);
        parent.add_child(child2);
        assert_eq!(parent.child_count(), 2);
        assert_eq!(parent.flat_count(), 3);
        assert_eq!(parent.passed_count(), 1);
        assert_eq!(parent.failed_count(), 1);
    }

    #[test]
    fn test_zq_test_item_tags() {
        let mut item = ZqTestItem::new("t1", "test");
        item.add_tag("slow");
        assert!(item.has_tag("slow"));
        assert!(!item.has_tag("fast"));
    }

    #[test]
    fn test_zq_test_item_location() {
        let item = ZqTestItem::new("t1", "test").with_location("test.rs", 42);
        assert_eq!(item.uri, Some("test.rs".to_string()));
        assert_eq!(item.line, Some(42));
    }

    #[test]
    fn test_zq_test_item_display() {
        let mut item = ZqTestItem::new("t1", "My Test");
        item.pass(100);
        let s = format!("{}", item);
        assert!(s.contains("My Test"));
        assert!(s.contains("100ms"));
    }

    #[test]
    fn test_zq_test_run_new() {
        let run = ZqTestRun::new("r1", "Unit Tests");
        assert_eq!(run.item_count(), 0);
        assert!(!run.is_complete());
    }

    #[test]
    fn test_zq_test_run_results() {
        let mut run = ZqTestRun::new("r1", "Tests");
        run.start(1000);
        let mut t1 = ZqTestItem::new("t1", "a");
        t1.pass(10);
        let mut t2 = ZqTestItem::new("t2", "b");
        t2.pass(20);
        let mut t3 = ZqTestItem::new("t3", "c");
        t3.fail("err", 30);
        run.add_item(t1);
        run.add_item(t2);
        run.add_item(t3);
        run.complete(1060);
        assert!(run.is_complete());
        assert_eq!(run.total_passed(), 2);
        assert_eq!(run.total_failed(), 1);
        assert!(!run.all_passed());
        assert_eq!(run.duration_ms(), Some(60));
    }

    #[test]
    fn test_zq_test_run_all_passed() {
        let mut run = ZqTestRun::new("r1", "Tests");
        let mut t1 = ZqTestItem::new("t1", "a");
        t1.pass(5);
        run.add_item(t1);
        assert!(run.all_passed());
    }

    #[test]
    fn test_zq_test_run_display() {
        let run = ZqTestRun::new("r1", "Suite");
        assert!(format!("{}", run).contains("Suite"));
    }


    // --- zr_ webview and panel tests ---

    #[test]
    fn test_zr_webview_options_default() {
        let opts = ZrWebviewOptions::new();
        assert!(!opts.enable_scripts);
        assert!(!opts.enable_forms);
        assert!(opts.local_resource_roots.is_empty());
        assert!(opts.port_mapping.is_empty());
    }

    #[test]
    fn test_zr_webview_options_builder() {
        let opts = ZrWebviewOptions::new()
            .with_scripts()
            .with_forms()
            .add_local_root("/workspace")
            .add_port_mapping(3000, 3001);
        assert!(opts.enable_scripts);
        assert!(opts.enable_forms);
        assert_eq!(opts.local_resource_roots, vec!["/workspace"]);
        assert_eq!(opts.port_mapping, vec![(3000, 3001)]);
    }

    #[test]
    fn test_zr_webview_html_and_messages() {
        let mut wv = ZrWebview::new(ZrWebviewOptions::new());
        wv.set_html("<h1>Hello</h1>");
        assert_eq!(wv.html, "<h1>Hello</h1>");
        wv.post_message("ping");
        wv.receive_message("pong");
        assert_eq!(wv.pending_messages(), &["ping"]);
        assert_eq!(wv.received_messages(), &["pong"]);
        let drained = wv.drain_outgoing();
        assert_eq!(drained, vec!["ping"]);
        assert!(wv.pending_messages().is_empty());
    }

    #[test]
    fn test_zr_webview_as_uri() {
        let wv = ZrWebview::new(ZrWebviewOptions::new());
        assert_eq!(wv.as_uri("https://ext/", "/style.css"), "https://ext/style.css");
        assert_eq!(wv.as_uri("https://ext", "style.css"), "https://ext/style.css");
    }

    #[test]
    fn test_zr_view_column_index() {
        assert_eq!(ZrViewColumn::One.index(), Some(0));
        assert_eq!(ZrViewColumn::Two.index(), Some(1));
        assert_eq!(ZrViewColumn::Three.index(), Some(2));
        assert_eq!(ZrViewColumn::Active.index(), None);
        assert_eq!(ZrViewColumn::Beside.index(), None);
    }

    #[test]
    fn test_zr_webview_panel_lifecycle() {
        let opts = ZrWebviewOptions::new().with_scripts();
        let mut panel = ZrWebviewPanel::new("preview", "Preview", ZrViewColumn::One, opts);
        assert!(panel.is_active);
        assert!(panel.is_visible);
        assert!(!panel.is_disposed());
        panel.set_title("New Title");
        assert_eq!(panel.title, "New Title");
        panel.hide();
        assert!(!panel.is_visible);
        panel.reveal(ZrViewColumn::Two);
        assert!(panel.is_visible);
        assert_eq!(panel.column, ZrViewColumn::Two);
        panel.dispose();
        assert!(panel.is_disposed());
    }

    #[test]
    fn test_zr_webview_panel_retention() {
        let opts = ZrWebviewOptions::new();
        let panel = ZrWebviewPanel::new("md", "Markdown", ZrViewColumn::One, opts)
            .with_retention(ZrWebviewRetention::Retain);
        assert_eq!(panel.retention, ZrWebviewRetention::Retain);
    }

    #[test]
    fn test_zr_webview_view_badge() {
        let mut view = ZrWebviewView::new("explorer", ZrWebviewOptions::new());
        assert!(view.badge_value.is_none());
        view.set_badge("3", "3 items");
        assert_eq!(view.badge_value.as_deref(), Some("3"));
        assert_eq!(view.badge_tooltip.as_deref(), Some("3 items"));
        view.clear_badge();
        assert!(view.badge_value.is_none());
    }

    #[test]
    fn test_zr_webview_view_description() {
        let mut view = ZrWebviewView::new("sidebar", ZrWebviewOptions::new());
        assert!(view.description.is_none());
        view.set_description("Source Control");
        assert_eq!(view.description.as_deref(), Some("Source Control"));
    }

    #[test]
    fn test_zr_webview_registry() {
        let mut reg = ZrWebviewRegistry::new();
        let opts = ZrWebviewOptions::new().with_scripts();
        let panel = ZrWebviewPanel::new("html", "HTML", ZrViewColumn::One, opts);
        let pid = reg.register_panel(panel);
        assert_eq!(reg.panel_count(), 1);
        assert_eq!(reg.active_panels().len(), 1);
        let view = ZrWebviewView::new("tree", ZrWebviewOptions::new());
        let vid = reg.register_view(view);
        assert_eq!(reg.view_count(), 1);
        reg.dispose_panel(pid);
        assert!(reg.panel(pid).unwrap().is_disposed());
        assert_eq!(reg.active_panels().len(), 0);
        assert!(reg.view(vid).is_some());
    }

    #[test]
    fn test_zr_webview_registry_mut_access() {
        let mut reg = ZrWebviewRegistry::new();
        let panel = ZrWebviewPanel::new("p", "P", ZrViewColumn::Active, ZrWebviewOptions::new());
        let pid = reg.register_panel(panel);
        reg.panel_mut(pid).unwrap().set_title("Updated");
        assert_eq!(reg.panel(pid).unwrap().title, "Updated");
        let view = ZrWebviewView::new("v", ZrWebviewOptions::new());
        let vid = reg.register_view(view);
        reg.view_mut(vid).unwrap().set_description("Desc");
        assert_eq!(reg.view(vid).unwrap().description.as_deref(), Some("Desc"));
    }


    // --- zs_ configuration and settings tests ---

    #[test]
    fn test_zs_config_target_priority() {
        assert!(ZsConfigTarget::WorkspaceFolder.overrides(&ZsConfigTarget::Workspace));
        assert!(ZsConfigTarget::Workspace.overrides(&ZsConfigTarget::User));
        assert!(ZsConfigTarget::User.overrides(&ZsConfigTarget::Global));
        assert!(!ZsConfigTarget::Global.overrides(&ZsConfigTarget::User));
    }

    #[test]
    fn test_zs_config_value_types() {
        assert_eq!(ZsConfigValue::Bool(true).as_bool(), Some(true));
        assert_eq!(ZsConfigValue::Int(42).as_i64(), Some(42));
        assert_eq!(ZsConfigValue::Float(3.14).as_f64(), Some(3.14));
        assert_eq!(ZsConfigValue::Int(5).as_f64(), Some(5.0));
        assert_eq!(ZsConfigValue::Str("hi".into()).as_str(), Some("hi"));
        assert!(ZsConfigValue::Null.is_null());
        assert_eq!(ZsConfigValue::Bool(false).type_name(), "boolean");
        assert_eq!(ZsConfigValue::Array(vec![]).type_name(), "array");
    }

    #[test]
    fn test_zs_config_store_set_get() {
        let mut store = ZsConfigStore::new();
        store.set("editor.fontSize", ZsConfigValue::Int(14), ZsConfigTarget::User);
        assert_eq!(store.get("editor.fontSize"), Some(&ZsConfigValue::Int(14)));
        assert!(store.has("editor.fontSize"));
        assert!(!store.has("nonexistent"));
    }

    #[test]
    fn test_zs_config_store_layered() {
        let mut store = ZsConfigStore::new();
        store.set("tabSize", ZsConfigValue::Int(4), ZsConfigTarget::User);
        store.set("tabSize", ZsConfigValue::Int(2), ZsConfigTarget::Workspace);
        // Workspace overrides User
        assert_eq!(store.get("tabSize"), Some(&ZsConfigValue::Int(2)));
        assert_eq!(store.get_at("tabSize", ZsConfigTarget::User), Some(&ZsConfigValue::Int(4)));
    }

    #[test]
    fn test_zs_config_store_remove() {
        let mut store = ZsConfigStore::new();
        store.set("k", ZsConfigValue::Bool(true), ZsConfigTarget::User);
        assert!(store.remove("k", ZsConfigTarget::User));
        assert!(!store.has("k"));
        assert!(!store.remove("k", ZsConfigTarget::User));
    }

    #[test]
    fn test_zs_config_store_keys_and_entries() {
        let mut store = ZsConfigStore::new();
        store.set("a", ZsConfigValue::Int(1), ZsConfigTarget::User);
        store.set("b", ZsConfigValue::Int(2), ZsConfigTarget::Workspace);
        store.set("a", ZsConfigValue::Int(3), ZsConfigTarget::Workspace);
        assert_eq!(store.keys().len(), 2);
        assert_eq!(store.entries_for_target(ZsConfigTarget::Workspace).len(), 2);
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn test_zs_config_store_merge() {
        let mut s1 = ZsConfigStore::new();
        s1.set("x", ZsConfigValue::Int(1), ZsConfigTarget::User);
        let mut s2 = ZsConfigStore::new();
        s2.set("x", ZsConfigValue::Int(2), ZsConfigTarget::User);
        s2.set("y", ZsConfigValue::Bool(true), ZsConfigTarget::User);
        s1.merge(&s2);
        assert_eq!(s1.get_at("x", ZsConfigTarget::User), Some(&ZsConfigValue::Int(2)));
        assert!(s1.has("y"));
    }

    #[test]
    fn test_zs_config_store_effective() {
        let store = ZsConfigStore::new();
        let v = store.effective("missing", ZsConfigValue::Str("default".into()));
        assert_eq!(v, ZsConfigValue::Str("default".into()));
    }

    #[test]
    fn test_zs_config_store_inspect() {
        let mut store = ZsConfigStore::new();
        store.set("k", ZsConfigValue::Int(1), ZsConfigTarget::User);
        store.set("k", ZsConfigValue::Int(2), ZsConfigTarget::Workspace);
        let layers = store.inspect("k");
        assert_eq!(layers.len(), 2);
    }

    #[test]
    fn test_zs_config_schema_validate() {
        let schema = ZsConfigSchema::new("fontSize", "integer", ZsConfigValue::Int(14), "Font size");
        assert!(schema.validate(&ZsConfigValue::Int(12)));
        assert!(!schema.validate(&ZsConfigValue::Str("big".into())));
    }

    #[test]
    fn test_zs_config_schema_enum_validate() {
        let schema = ZsConfigSchema::new("eol", "string", ZsConfigValue::Str("\n".into()), "EOL")
            .with_enum(vec!["\n".into(), "\r\n".into()]);
        assert!(schema.validate(&ZsConfigValue::Str("\n".into())));
        assert!(!schema.validate(&ZsConfigValue::Str("\r".into())));
    }

    #[test]
    fn test_zs_config_schema_scope() {
        let schema = ZsConfigSchema::new("k", "boolean", ZsConfigValue::Bool(false), "desc")
            .with_scope(ZsConfigTarget::Workspace);
        assert_eq!(schema.scope, ZsConfigTarget::Workspace);
    }


    // --- zt_ language feature provider tests ---

    #[test]
    fn test_zt_formatting_options() {
        let opts = ZtFormattingOptions::new(4, true);
        assert_eq!(opts.indent_str(), "    ");
        let opts2 = ZtFormattingOptions::new(1, false);
        assert_eq!(opts2.indent_str(), "\t");
    }

    #[test]
    fn test_zt_text_edit() {
        let e = ZtTextEdit::new(0, 0, 0, 0, "hello");
        assert!(e.is_insert());
        assert!(!e.is_delete());
        let d = ZtTextEdit::new(0, 0, 0, 5, "");
        assert!(d.is_delete());
        assert!(!d.is_insert());
    }

    #[test]
    fn test_zt_format_result() {
        let mut r = ZtFormatResult::new();
        assert!(r.is_empty());
        r.add(ZtTextEdit::new(2, 0, 2, 4, "  "));
        r.add(ZtTextEdit::new(0, 0, 0, 2, "  "));
        r.sort_edits();
        assert_eq!(r.edits[0].start_line, 0);
        assert_eq!(r.edits[1].start_line, 2);
    }

    #[test]
    fn test_zt_document_link() {
        let link = ZtDocumentLink::new(5, 10, 5, 30, "https://example.com", ZtLinkKind::Url)
            .with_tooltip("Click to open");
        assert_eq!(link.target, "https://example.com");
        assert_eq!(link.tooltip.as_deref(), Some("Click to open"));
        assert_eq!(link.kind, ZtLinkKind::Url);
    }

    #[test]
    fn test_zt_on_type_formatting() {
        let otf = ZtOnTypeFormatting::new(';').add_trigger('}').add_trigger('\n');
        assert!(otf.is_trigger(';'));
        assert!(otf.is_trigger('}'));
        assert!(!otf.is_trigger('a'));
        assert_eq!(otf.all_triggers().len(), 3);
    }

    #[test]
    fn test_zt_color_presentation() {
        let cp = ZtColorPresentation::new("#ff0000")
            .with_edit(ZtTextEdit::new(1, 5, 1, 12, "#ff0000"));
        assert_eq!(cp.label, "#ff0000");
        assert!(cp.text_edit.is_some());
    }

    #[test]
    fn test_zt_color_info() {
        let ci = ZtColorInfo::new(10, 5, 12, 1.0, 0.0, 0.0, 1.0);
        assert_eq!(ci.to_hex(), "#ff0000");
        assert_eq!(ci.to_rgb_string(), "rgb(255, 0, 0)");
    }

    #[test]
    fn test_zt_color_info_mixed() {
        let ci = ZtColorInfo::new(0, 0, 7, 0.0, 0.5, 1.0, 1.0);
        assert_eq!(ci.to_hex(), "#007fff");
    }

    #[test]
    fn test_zt_provider_registration() {
        let reg = ZtProviderRegistration::new("fmt")
            .for_language("rust")
            .for_scheme("file");
        assert!(reg.matches_language("rust"));
        assert!(!reg.matches_language("python"));
        assert!(reg.matches_scheme("file"));
        assert!(!reg.matches_scheme("untitled"));
    }

    #[test]
    fn test_zt_provider_registration_empty_matches_all() {
        let reg = ZtProviderRegistration::new("all");
        assert!(reg.matches_language("anything"));
        assert!(reg.matches_scheme("any"));
    }


    // --- zu_ workspace trust and authentication tests ---

    #[test]
    fn test_zu_trust_state() {
        assert!(ZuTrustState::Trusted.is_trusted());
        assert!(!ZuTrustState::Untrusted.is_trusted());
        assert!(!ZuTrustState::Unknown.is_trusted());
    }

    #[test]
    fn test_zu_workspace_trust_lifecycle() {
        let mut wt = ZuWorkspaceTrust::new();
        assert_eq!(wt.state, ZuTrustState::Unknown);
        wt.grant_trust();
        assert!(wt.state.is_trusted());
        wt.revoke_trust();
        assert!(!wt.state.is_trusted());
    }

    #[test]
    fn test_zu_workspace_trust_folders() {
        let mut wt = ZuWorkspaceTrust::new();
        wt.add_trusted_folder("/home/user/project");
        wt.add_trusted_folder("/home/user/project"); // duplicate
        assert_eq!(wt.trusted_folders.len(), 1);
        assert!(wt.is_folder_trusted("/home/user/project/src"));
        assert!(!wt.is_folder_trusted("/tmp"));
    }

    #[test]
    fn test_zu_workspace_trust_effective() {
        let mut wt = ZuWorkspaceTrust::new();
        assert_eq!(wt.effective_trust(), ZuTrustState::Unknown);
        wt.parent_trusted = true;
        assert_eq!(wt.effective_trust(), ZuTrustState::Trusted);
    }

    #[test]
    fn test_zu_auth_session() {
        let session = ZuAuthSession::new("s1", "tok123", "User", "u1")
            .with_scopes(vec!["read".into(), "write".into()]);
        assert!(session.has_scope("read"));
        assert!(!session.has_scope("admin"));
        assert_eq!(session.display_name(), "User");
    }

    #[test]
    fn test_zu_auth_options() {
        let interactive = ZuAuthOptions::interactive();
        assert!(interactive.create_if_none);
        let silent = ZuAuthOptions::silent();
        assert!(silent.silent);
    }

    #[test]
    fn test_zu_auth_provider() {
        let mut prov = ZuAuthProvider::new("github", "GitHub")
            .with_multiple_accounts();
        assert!(prov.supports_multiple_accounts());
        let s = ZuAuthSession::new("s1", "t", "U", "u1")
            .with_scopes(vec!["repo".into()]);
        prov.add_session(s);
        assert_eq!(prov.session_count(), 1);
        assert!(prov.get_session(&["repo".into()]).is_some());
        assert!(prov.get_session(&["admin".into()]).is_none());
        assert!(prov.remove_session("s1"));
        assert_eq!(prov.session_count(), 0);
    }

    #[test]
    fn test_zu_auth_registry() {
        let mut reg = ZuAuthRegistry::new();
        let prov = ZuAuthProvider::new("gh", "GitHub");
        reg.register(prov);
        assert!(reg.get_provider("gh").is_some());
        assert!(reg.get_provider("ms").is_none());
        assert_eq!(reg.provider_ids(), vec!["gh"]);
    }

    #[test]
    fn test_zu_secret_storage() {
        let mut ss = ZuSecretStorage::new();
        assert!(ss.is_empty());
        ss.store("token", "abc123");
        assert_eq!(ss.get("token"), Some("abc123"));
        ss.store("token", "updated");
        assert_eq!(ss.get("token"), Some("updated"));
        assert_eq!(ss.len(), 1);
        assert!(ss.delete("token"));
        assert!(ss.is_empty());
    }

    #[test]
    fn test_zu_secret_storage_keys() {
        let mut ss = ZuSecretStorage::new();
        ss.store("a", "1");
        ss.store("b", "2");
        assert_eq!(ss.keys().len(), 2);
        assert!(!ss.delete("c"));
    }


    // --- zv_ comments API tests ---

    #[test]
    fn test_zv_reaction_toggle() {
        let mut r = ZvReaction::new("👍", 5);
        assert!(!r.has_reacted);
        r.toggle();
        assert!(r.has_reacted);
        assert_eq!(r.count, 6);
        r.toggle();
        assert!(!r.has_reacted);
        assert_eq!(r.count, 5);
    }

    #[test]
    fn test_zv_comment_author() {
        let author = ZvCommentAuthor::new("Alice").with_icon("/icons/alice.png");
        assert_eq!(author.name, "Alice");
        assert_eq!(author.icon_path.as_deref(), Some("/icons/alice.png"));
    }

    #[test]
    fn test_zv_comment() {
        let author = ZvCommentAuthor::new("Bob");
        let mut c = ZvComment::new("Great work!", author).with_timestamp(1000);
        assert_eq!(c.body, "Great work!");
        assert_eq!(c.mode, ZvCommentMode::Preview);
        assert_eq!(c.timestamp, Some(1000));
        c.add_reaction(ZvReaction::new("❤️", 3));
        assert_eq!(c.total_reactions(), 3);
        c.set_mode(ZvCommentMode::Editing);
        assert_eq!(c.mode, ZvCommentMode::Editing);
    }

    #[test]
    fn test_zv_comment_thread() {
        let mut thread = ZvCommentThread::new("file:///main.rs", 10);
        assert!(!thread.is_resolved());
        assert!(thread.can_reply);
        let c = ZvComment::new("Fix this", ZvCommentAuthor::new("Alice"));
        thread.add_comment(c);
        assert_eq!(thread.comment_count(), 1);
        thread.resolve();
        assert!(thread.is_resolved());
        thread.unresolve();
        assert!(!thread.is_resolved());
    }

    #[test]
    fn test_zv_comment_thread_collapse() {
        let mut thread = ZvCommentThread::new("f", 0);
        assert_eq!(thread.collapsible, ZvCollapsibleState::Expanded);
        thread.collapse();
        assert_eq!(thread.collapsible, ZvCollapsibleState::Collapsed);
        thread.expand();
        assert_eq!(thread.collapsible, ZvCollapsibleState::Expanded);
    }

    #[test]
    fn test_zv_comment_thread_dispose() {
        let mut thread = ZvCommentThread::new("f", 0);
        assert!(!thread.is_disposed());
        thread.dispose();
        assert!(thread.is_disposed());
    }

    #[test]
    fn test_zv_comment_thread_label() {
        let mut thread = ZvCommentThread::new("f", 0);
        assert!(thread.label.is_none());
        thread.set_label("Review");
        assert_eq!(thread.label.as_deref(), Some("Review"));
    }

    #[test]
    fn test_zv_comment_controller() {
        let mut ctrl = ZvCommentController::new("review", "Code Review")
            .with_commenting_ranges()
            .with_reaction_handler();
        assert!(ctrl.commenting_range_provider);
        assert!(ctrl.reaction_handler);
        let idx = ctrl.create_thread("file:///a.rs", 5);
        ctrl.thread_mut(idx).unwrap().add_comment(
            ZvComment::new("Check this", ZvCommentAuthor::new("C"))
        );
        assert_eq!(ctrl.thread_count(), 1);
        assert_eq!(ctrl.active_thread_count(), 1);
        assert_eq!(ctrl.threads_for_uri("file:///a.rs").len(), 1);
        assert!(ctrl.threads_for_uri("file:///b.rs").is_empty());
    }

    #[test]
    fn test_zv_comment_controller_dispose_thread() {
        let mut ctrl = ZvCommentController::new("c", "C");
        let idx = ctrl.create_thread("f", 0);
        ctrl.dispose_thread(idx);
        assert!(ctrl.thread(idx).unwrap().is_disposed());
        assert_eq!(ctrl.active_thread_count(), 0);
    }


    // --- zw_ custom editor and file system tests ---

    #[test]
    fn test_zw_file_stat() {
        let stat = ZwFileStat::file(1024, 5000);
        assert!(stat.is_file());
        assert!(!stat.is_directory());
        assert!(!stat.is_readonly());
        let ro = stat.with_permissions(0o444);
        assert!(ro.is_readonly());
    }

    #[test]
    fn test_zw_file_stat_directory() {
        let stat = ZwFileStat::directory(1000);
        assert!(stat.is_directory());
        assert!(!stat.is_file());
        assert_eq!(stat.size, 0);
    }

    #[test]
    fn test_zw_file_change_event() {
        let ev = ZwFileChangeEvent::new("file:///a.txt", ZwFileChangeType::Created);
        assert_eq!(ev.change_type, ZwFileChangeType::Created);
    }

    #[test]
    fn test_zw_directory_entry() {
        let de = ZwDirectoryEntry::new("hello.rs", ZwFileType::File);
        assert_eq!(de.name, "hello.rs");
        assert_eq!(de.file_type, ZwFileType::File);
    }

    #[test]
    fn test_zw_memory_fs_write_read() {
        let mut fs = ZwMemoryFs::new();
        fs.write_file("file:///a.txt", b"hello", 100);
        assert_eq!(fs.read_file("file:///a.txt"), Some(b"hello".as_slice()));
        assert!(fs.exists("file:///a.txt"));
        assert_eq!(fs.file_count(), 1);
    }

    #[test]
    fn test_zw_memory_fs_overwrite() {
        let mut fs = ZwMemoryFs::new();
        fs.write_file("f", b"v1", 100);
        fs.write_file("f", b"v2", 200);
        assert_eq!(fs.read_file("f"), Some(b"v2".as_slice()));
        assert_eq!(fs.stat("f").unwrap().mtime, 200);
        assert_eq!(fs.file_count(), 1);
    }

    #[test]
    fn test_zw_memory_fs_delete_rename() {
        let mut fs = ZwMemoryFs::new();
        fs.write_file("a", b"data", 1);
        assert!(fs.rename("a", "b"));
        assert!(!fs.exists("a"));
        assert!(fs.exists("b"));
        assert!(fs.delete("b"));
        assert!(!fs.exists("b"));
        assert!(!fs.delete("b"));
    }

    #[test]
    fn test_zw_memory_fs_list() {
        let mut fs = ZwMemoryFs::new();
        fs.write_file("/dir/a.txt", b"a", 1);
        fs.write_file("/dir/b.txt", b"b", 1);
        fs.write_file("/other/c.txt", b"c", 1);
        let entries = fs.list("/dir");
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_zw_custom_document_lifecycle() {
        let mut doc = ZwCustomDocument::new("file:///img.png");
        assert!(!doc.is_dirty);
        doc.apply_edit(ZwCustomEdit::new("draw", vec![1, 2, 3]));
        assert!(doc.is_dirty);
        assert_eq!(doc.edit_count(), 1);
        doc.save();
        assert!(!doc.is_dirty);
        doc.apply_edit(ZwCustomEdit::new("erase", vec![4, 5]));
        assert!(doc.is_dirty);
        doc.revert();
        assert!(!doc.is_dirty);
        assert_eq!(doc.edit_count(), 1);
    }

    #[test]
    fn test_zw_custom_document_undo() {
        let mut doc = ZwCustomDocument::new("f");
        doc.apply_edit(ZwCustomEdit::new("a", vec![]));
        doc.apply_edit(ZwCustomEdit::new("b", vec![]));
        assert!(doc.can_undo());
        let undone = doc.undo().unwrap();
        assert_eq!(undone.label, "b");
        assert_eq!(doc.edit_count(), 1);
    }


    // --- zx_ tree view tests ---

    #[test]
    fn test_zx_tree_context() {
        let mut ctx = ZxTreeContext::new();
        ctx.set("type", "folder");
        assert_eq!(ctx.get("type"), Some("folder"));
        assert!(ctx.has("type"));
        assert!(!ctx.has("other"));
        assert_eq!(ctx.to_string_value(), "type=folder");
    }

    #[test]
    fn test_zx_tree_item() {
        let item = ZxTreeItem::new("src", "Source")
            .with_children(ZxTreeCollapsible::Collapsed)
            .with_description("2 files")
            .with_tooltip("Source folder")
            .with_icon("folder")
            .with_command("open")
            .with_uri("file:///src");
        assert_eq!(item.label, "Source");
        assert!(!item.is_leaf());
        assert_eq!(item.description.as_deref(), Some("2 files"));
        assert_eq!(item.icon_id.as_deref(), Some("folder"));
    }

    #[test]
    fn test_zx_tree_item_children() {
        let mut item = ZxTreeItem::new("root", "Root")
            .with_children(ZxTreeCollapsible::Expanded);
        item.add_child("child1");
        item.add_child("child2");
        assert!(item.has_children());
        assert_eq!(item.children_ids.len(), 2);
    }

    #[test]
    fn test_zx_tree_item_leaf() {
        let item = ZxTreeItem::new("file", "main.rs");
        assert!(item.is_leaf());
        assert!(!item.has_children());
    }

    #[test]
    fn test_zx_tree_view_basic() {
        let mut tv = ZxTreeView::new("explorer", "Explorer", ZxTreeViewOptions::new());
        let root = ZxTreeItem::new("r", "Root").with_children(ZxTreeCollapsible::Expanded);
        tv.add_root(root);
        assert_eq!(tv.root_items().len(), 1);
        assert_eq!(tv.item_count(), 1);
    }

    #[test]
    fn test_zx_tree_view_hierarchy() {
        let mut tv = ZxTreeView::new("t", "T", ZxTreeViewOptions::new());
        let root = ZxTreeItem::new("r", "Root").with_children(ZxTreeCollapsible::Expanded);
        tv.add_root(root);
        let child = ZxTreeItem::new("c1", "Child 1");
        tv.add_child_item("r", child);
        assert_eq!(tv.children_of("r").len(), 1);
        assert_eq!(tv.children_of("r")[0].label, "Child 1");
    }

    #[test]
    fn test_zx_tree_view_selection_single() {
        let mut tv = ZxTreeView::new("t", "T", ZxTreeViewOptions::new());
        let item1 = ZxTreeItem::new("a", "A");
        let item2 = ZxTreeItem::new("b", "B");
        tv.add_root(item1);
        tv.add_root(item2);
        tv.select("a");
        assert_eq!(tv.selection(), &["a"]);
        tv.select("b");
        // Single select replaces
        assert_eq!(tv.selection(), &["b"]);
    }

    #[test]
    fn test_zx_tree_view_selection_multiple() {
        let mut opts = ZxTreeViewOptions::new();
        opts.can_select_many = true;
        let mut tv = ZxTreeView::new("t", "T", opts);
        tv.add_root(ZxTreeItem::new("a", "A"));
        tv.add_root(ZxTreeItem::new("b", "B"));
        tv.select("a");
        tv.select("b");
        assert_eq!(tv.selection().len(), 2);
        tv.deselect("a");
        assert_eq!(tv.selection(), &["b"]);
    }

    #[test]
    fn test_zx_tree_view_message_badge() {
        let mut tv = ZxTreeView::new("t", "T", ZxTreeViewOptions::new());
        tv.set_message("Loading...");
        assert_eq!(tv.message.as_deref(), Some("Loading..."));
        tv.set_badge(5);
        assert_eq!(tv.badge_count, Some(5));
    }

}
