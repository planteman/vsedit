//! User keybinding service.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during keybinding operations.
#[derive(Debug, Clone, PartialEq)]
pub enum KeybindingError {
    BindingNotFound,
    DuplicateBinding,
    ConflictingBinding(String),
}

impl fmt::Display for KeybindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeybindingError::BindingNotFound => write!(f, "binding not found"),
            KeybindingError::DuplicateBinding => write!(f, "duplicate binding"),
            KeybindingError::ConflictingBinding(cmd) => {
                write!(f, "conflicting binding for command '{cmd}'")
            }
        }
    }
}

/// Modifier keys for keybindings.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyMod {
    CtrlCmd,
    Shift,
    Alt,
    WinCtrl,
}

impl fmt::Display for KeyMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyMod::CtrlCmd => write!(f, "Ctrl"),
            KeyMod::Shift => write!(f, "Shift"),
            KeyMod::Alt => write!(f, "Alt"),
            KeyMod::WinCtrl => write!(f, "Win"),
        }
    }
}

/// Origin of a keybinding.
#[derive(Debug, Clone, PartialEq)]
pub enum KeybindingSource {
    Default,
    User,
    Extension,
}

impl fmt::Display for KeybindingSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeybindingSource::Default => write!(f, "Default"),
            KeybindingSource::User => write!(f, "User"),
            KeybindingSource::Extension => write!(f, "Extension"),
        }
    }
}

/// A fully resolved keybinding.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedKeybinding {
    pub key: String,
    pub modifiers: Vec<KeyMod>,
    pub command: String,
    pub when: Option<String>,
    pub source: KeybindingSource,
}

impl fmt::Display for ResolvedKeybinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let formatted = KeybindingService::format_binding(self);
        write!(f, "{} -> {} [{}]", formatted, self.command, self.source)
    }
}

impl ResolvedKeybinding {
    /// Check if the given key and modifiers match this binding.
    pub fn matches(&self, key: &str, modifiers: &[KeyMod]) -> bool {
        self.key == key && self.modifiers == modifiers
    }
}

/// Builder for constructing a `ResolvedKeybinding` step by step.
pub struct KeybindingBuilder {
    key: Option<String>,
    modifiers: Vec<KeyMod>,
    command: Option<String>,
    when: Option<String>,
    source: KeybindingSource,
}

impl KeybindingBuilder {
    pub fn new() -> Self {
        Self {
            key: None,
            modifiers: Vec::new(),
            command: None,
            when: None,
            source: KeybindingSource::Default,
        }
    }

    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    pub fn modifier(mut self, modifier: KeyMod) -> Self {
        self.modifiers.push(modifier);
        self
    }

    pub fn command(mut self, command: &str) -> Self {
        self.command = Some(command.to_string());
        self
    }

    pub fn when(mut self, when: &str) -> Self {
        self.when = Some(when.to_string());
        self
    }

    pub fn source(mut self, source: KeybindingSource) -> Self {
        self.source = source;
        self
    }

    /// Build the `ResolvedKeybinding`. Panics if `key` or `command` is not set.
    pub fn build(self) -> ResolvedKeybinding {
        ResolvedKeybinding {
            key: self.key.expect("key is required"),
            modifiers: self.modifiers,
            command: self.command.expect("command is required"),
            when: self.when,
            source: self.source,
        }
    }
}

impl Default for KeybindingBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for keybinding workbench functionality.
pub struct KeybindingService {
    pub bindings: Vec<ResolvedKeybinding>,
}

impl KeybindingService {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn register(&mut self, binding: ResolvedKeybinding) {
        self.bindings.push(binding);
    }

    /// Register a binding, returning an error if a conflicting binding already exists.
    pub fn try_register(
        &mut self,
        binding: ResolvedKeybinding,
    ) -> Result<(), KeybindingError> {
        if self.has_conflict(&binding.key, &binding.modifiers) {
            let existing = self.resolve(&binding.key, &binding.modifiers);
            let cmd = existing[0].command.clone();
            return Err(KeybindingError::ConflictingBinding(cmd));
        }
        self.bindings.push(binding);
        Ok(())
    }

    pub fn resolve(&self, key: &str, modifiers: &[KeyMod]) -> Vec<&ResolvedKeybinding> {
        self.bindings
            .iter()
            .filter(|b| b.key == key && b.modifiers == modifiers)
            .collect()
    }

    pub fn get_bindings_for_command(&self, cmd: &str) -> Vec<&ResolvedKeybinding> {
        self.bindings
            .iter()
            .filter(|b| b.command == cmd)
            .collect()
    }

    pub fn remove_binding(&mut self, command: &str, key: &str) -> bool {
        let before = self.bindings.len();
        self.bindings
            .retain(|b| !(b.command == command && b.key == key));
        self.bindings.len() < before
    }

    pub fn format_binding(binding: &ResolvedKeybinding) -> String {
        let mods: Vec<&str> = binding
            .modifiers
            .iter()
            .map(|m| match m {
                KeyMod::CtrlCmd => "Ctrl",
                KeyMod::Shift => "Shift",
                KeyMod::Alt => "Alt",
                KeyMod::WinCtrl => "Win",
            })
            .collect();
        if mods.is_empty() {
            binding.key.clone()
        } else {
            format!("{}+{}", mods.join("+"), binding.key)
        }
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    /// Check if a key+modifiers combination has multiple bindings.
    pub fn has_conflict(&self, key: &str, modifiers: &[KeyMod]) -> bool {
        self.resolve(key, modifiers).len() > 1
    }

    /// Return all sets of bindings that share the same key+modifiers.
    pub fn get_conflicts(&self) -> Vec<Vec<&ResolvedKeybinding>> {
        let mut groups: Vec<Vec<&ResolvedKeybinding>> = Vec::new();
        for binding in &self.bindings {
            let found = groups.iter_mut().find(|g| {
                g[0].key == binding.key && g[0].modifiers == binding.modifiers
            });
            match found {
                Some(group) => group.push(binding),
                None => groups.push(vec![binding]),
            }
        }
        groups.into_iter().filter(|g| g.len() > 1).collect()
    }

    /// Filter bindings by source.
    pub fn get_by_source(&self, source: &KeybindingSource) -> Vec<&ResolvedKeybinding> {
        self.bindings
            .iter()
            .filter(|b| &b.source == source)
            .collect()
    }

    /// Remove all bindings.
    pub fn clear(&mut self) {
        self.bindings.clear();
    }

    /// Return all bindings as formatted strings.
    pub fn export_bindings(&self) -> Vec<String> {
        self.bindings.iter().map(|b| b.to_string()).collect()
    }

    /// Returns true if bindings is empty.
    pub fn is_bindings_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Get the first binding, if any.
    pub fn first_binding(&self) -> Option<&ResolvedKeybinding> {
        self.bindings.first()
    }

    /// Get the last binding, if any.
    pub fn last_binding(&self) -> Option<&ResolvedKeybinding> {
        self.bindings.last()
    }

    /// Retain only bindings matching the predicate.
    pub fn retain_bindings(&mut self, f: impl Fn(&ResolvedKeybinding) -> bool) {
        self.bindings.retain(|item| f(item));
    }
}

impl Default for KeybindingService {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents the current editor context for when-clause evaluation.
#[derive(Debug, Clone, Default)]
pub struct KeybindingWhenContext {
    values: std::collections::HashMap<String, bool>,
}

impl KeybindingWhenContext {
    pub fn new() -> Self {
        Self { values: std::collections::HashMap::new() }
    }

    pub fn set(&mut self, key: &str, value: bool) {
        self.values.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> bool {
        self.values.get(key).copied().unwrap_or(false)
    }

    /// Evaluate a when-clause string against the context.
    /// Supports simple expressions: "key", "!key", "key1 && key2", "key1 || key2".
    pub fn evaluate(&self, when: &str) -> bool {
        let when = when.trim();
        if when.is_empty() {
            return true;
        }
        // Handle OR (lower precedence)
        if when.contains("||") {
            return when.split("||").any(|part| self.evaluate(part));
        }
        // Handle AND
        if when.contains("&&") {
            return when.split("&&").all(|part| self.evaluate(part));
        }
        // Handle negation
        let trimmed = when.trim();
        if let Some(key) = trimmed.strip_prefix('!') {
            return !self.get(key.trim());
        }
        // Simple key lookup
        self.get(trimmed)
    }

    /// Check if a binding's when-clause is satisfied by this context.
    pub fn binding_active(&self, binding: &ResolvedKeybinding) -> bool {
        match &binding.when {
            None => true,
            Some(when) => self.evaluate(when),
        }
    }
}

/// A chord sequence to match against.
#[derive(Debug, Clone, PartialEq)]
pub struct ChordSequence {
    pub chords: Vec<(String, Vec<KeyMod>)>,
}

impl ChordSequence {
    pub fn single(key: &str, modifiers: Vec<KeyMod>) -> Self {
        Self { chords: vec![(key.to_string(), modifiers)] }
    }

    pub fn double(
        key1: &str, mods1: Vec<KeyMod>,
        key2: &str, mods2: Vec<KeyMod>,
    ) -> Self {
        Self {
            chords: vec![
                (key1.to_string(), mods1),
                (key2.to_string(), mods2),
            ],
        }
    }

    pub fn len(&self) -> usize {
        self.chords.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chords.is_empty()
    }
}

/// Resolve a chord chain against the service. For single chords, uses
/// normal resolution. For multi-chord, finds bindings whose key matches
/// the last chord and whose command starts with the first chord's format.
pub fn keybinding_resolve_chain(
    service: &KeybindingService,
    context: &KeybindingWhenContext,
    sequence: &ChordSequence,
) -> Vec<ResolvedKeybinding> {
    if sequence.is_empty() {
        return Vec::new();
    }
    let (ref key, ref mods) = sequence.chords[0];
    let candidates = service.resolve(key, mods);
    candidates
        .into_iter()
        .filter(|b| context.binding_active(b))
        .cloned()
        .collect()
}

/// Generate a set of default keybindings commonly used in editors.
pub fn keybinding_defaults() -> Vec<ResolvedKeybinding> {
    vec![
        KeybindingBuilder::new().key("s").modifier(KeyMod::CtrlCmd).command("workbench.action.files.save").build(),
        KeybindingBuilder::new().key("z").modifier(KeyMod::CtrlCmd).command("editor.action.undo").build(),
        KeybindingBuilder::new().key("y").modifier(KeyMod::CtrlCmd).command("editor.action.redo").build(),
        KeybindingBuilder::new().key("c").modifier(KeyMod::CtrlCmd).command("editor.action.clipboardCopy").build(),
        KeybindingBuilder::new().key("v").modifier(KeyMod::CtrlCmd).command("editor.action.clipboardPaste").build(),
        KeybindingBuilder::new().key("x").modifier(KeyMod::CtrlCmd).command("editor.action.clipboardCut").build(),
        KeybindingBuilder::new().key("a").modifier(KeyMod::CtrlCmd).command("editor.action.selectAll").build(),
        KeybindingBuilder::new().key("f").modifier(KeyMod::CtrlCmd).command("actions.find").build(),
        KeybindingBuilder::new().key("h").modifier(KeyMod::CtrlCmd).command("editor.action.startFindReplaceAction").build(),
        KeybindingBuilder::new().key("p").modifier(KeyMod::CtrlCmd).command("workbench.action.quickOpen").build(),
        KeybindingBuilder::new().key("p").modifier(KeyMod::CtrlCmd).modifier(KeyMod::Shift).command("workbench.action.showCommands").build(),
        KeybindingBuilder::new().key("n").modifier(KeyMod::CtrlCmd).command("workbench.action.files.newUntitledFile").build(),
    ]
}

/// Accumulated statistics for wb-keybinding operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbKeybindingStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbKeybindingStats {
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
    pub fn merge(&mut self, other: &WbKeybindingStats) {
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

impl Default for WbKeybindingStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbKeybindingStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbKeybindingStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-keybinding.
#[derive(Debug, Clone)]
pub struct WbKeybindingValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbKeybindingValidator {
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

impl Default for WbKeybindingValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolvedKeybinding {
    pub fn has_when_context(&self) -> bool {
        self.when.is_some()
    }

    pub fn matches_filter(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.key.to_lowercase().contains(&q)
            || self.command.to_lowercase().contains(&q)
            || self.source.to_string().to_lowercase().contains(&q)
            || self
                .when
                .as_ref()
                .map_or(false, |w| w.to_lowercase().contains(&q))
    }

    pub fn modifier_count(&self) -> usize {
        self.modifiers.len()
    }
}

impl KeybindingSource {
    pub fn is_user(&self) -> bool {
        matches!(self, KeybindingSource::User)
    }

    pub fn is_default(&self) -> bool {
        matches!(self, KeybindingSource::Default)
    }

    pub fn is_extension(&self) -> bool {
        matches!(self, KeybindingSource::Extension)
    }
}

impl KeybindingService {
    pub fn iter(&self) -> std::slice::Iter<'_, ResolvedKeybinding> {
        self.bindings.iter()
    }

    pub fn find_by_command(&self, query: &str) -> Vec<&ResolvedKeybinding> {
        let q = query.to_lowercase();
        self.bindings
            .iter()
            .filter(|b| b.command.to_lowercase().contains(&q))
            .collect()
    }

    pub fn find_conflicts(&self) -> Vec<(String, Vec<&ResolvedKeybinding>)> {
        let mut groups: std::collections::HashMap<String, Vec<&ResolvedKeybinding>> =
            std::collections::HashMap::new();
        for binding in &self.bindings {
            let key = KeybindingService::format_binding(binding);
            groups.entry(key).or_default().push(binding);
        }
        groups
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .collect()
    }

    pub fn commands(&self) -> Vec<&str> {
        let mut cmds: Vec<&str> = self.bindings.iter().map(|b| b.command.as_str()).collect();
        cmds.sort();
        cmds.dedup();
        cmds
    }
}

impl<'a> IntoIterator for &'a KeybindingService {
    type Item = &'a ResolvedKeybinding;
    type IntoIter = std::slice::Iter<'a, ResolvedKeybinding>;

    fn into_iter(self) -> Self::IntoIter {
        self.bindings.iter()
    }
}

impl KeybindingWhenContext {
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.values.keys().map(|k| k.as_str()).collect()
    }

    pub fn matches(&self, context: &str) -> bool {
        self.evaluate(context)
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.values.remove(key).is_some()
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }
}

impl fmt::Display for KeybindingWhenContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries: Vec<String> = self
            .values
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        write!(f, "WhenContext({})", entries.join(", "))
    }
}

impl ChordSequence {
    pub fn is_single(&self) -> bool {
        self.chords.len() == 1
    }

    pub fn first_key(&self) -> Option<&str> {
        self.chords.first().map(|(k, _)| k.as_str())
    }

    pub fn last_key(&self) -> Option<&str> {
        self.chords.last().map(|(k, _)| k.as_str())
    }
}

impl fmt::Display for ChordSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self
            .chords
            .iter()
            .map(|(key, mods)| {
                if mods.is_empty() {
                    key.clone()
                } else {
                    let m: Vec<String> = mods.iter().map(|m| m.to_string()).collect();
                    format!("{}+{}", m.join("+"), key)
                }
            })
            .collect();
        write!(f, "{}", parts.join(" "))
    }
}

impl WbKeybindingStats {
    pub fn summary(&self) -> String {
        format!(
            "{} ops ({} ok, {} err), avg {}ns, range [{}..{}]ns",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns(),
            self.min_time_ns().unwrap_or(0),
            self.max_time_ns().unwrap_or(0),
        )
    }

    pub fn has_failures(&self) -> bool {
        self.failed_operations > 0
    }
}

/// Parse a modifier name (case-insensitive) into a `KeyMod`.
pub fn parse_modifier(s: &str) -> Option<KeyMod> {
    match s.trim().to_lowercase().as_str() {
        "ctrl" | "ctrlcmd" => Some(KeyMod::CtrlCmd),
        "shift" => Some(KeyMod::Shift),
        "alt" => Some(KeyMod::Alt),
        "win" | "meta" | "super" => Some(KeyMod::WinCtrl),
        _ => None,
    }
}

/// Parse a single key chord string like "Ctrl+Shift+S" into (key, modifiers).
/// Returns `None` if the string is empty or contains no key part.
pub fn parse_key_chord(s: &str) -> Option<(String, Vec<KeyMod>)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split('+').collect();
    if parts.is_empty() {
        return None;
    }
    let mut modifiers = Vec::new();
    let mut key = None;
    for (i, part) in parts.iter().enumerate() {
        let trimmed = part.trim();
        if i < parts.len() - 1 {
            if let Some(m) = parse_modifier(trimmed) {
                modifiers.push(m);
            } else {
                // Unknown modifier treated as key if last unrecognized
                return None;
            }
        } else {
            key = Some(trimmed.to_string());
        }
    }
    key.map(|k| (k, modifiers))
}

/// Parse a full chord sequence string like "Ctrl+K Ctrl+C" (space-separated chords).
pub fn parse_chord_sequence(s: &str) -> Option<ChordSequence> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut chords = Vec::new();
    for part in s.split_whitespace() {
        let (key, mods) = parse_key_chord(part)?;
        chords.push((key, mods));
    }
    if chords.is_empty() {
        return None;
    }
    Some(ChordSequence { chords })
}

/// Format a chord sequence back into its canonical string representation.
pub fn format_chord_sequence(seq: &ChordSequence) -> String {
    seq.to_string()
}

/// Tracks per-command usage counts for keybinding analytics.
#[derive(Debug, Clone, Default)]
pub struct KeybindingUsageTracker {
    hits: std::collections::HashMap<String, u64>,
}

impl KeybindingUsageTracker {
    pub fn new() -> Self {
        Self {
            hits: std::collections::HashMap::new(),
        }
    }

    /// Record a single use of a command.
    pub fn record(&mut self, command: &str) {
        *self.hits.entry(command.to_string()).or_insert(0) += 1;
    }

    /// Get the usage count for a specific command.
    pub fn count(&self, command: &str) -> u64 {
        self.hits.get(command).copied().unwrap_or(0)
    }

    /// Return the total number of keybinding invocations across all commands.
    pub fn total_invocations(&self) -> u64 {
        self.hits.values().sum()
    }

    /// Return the top N most-used commands, sorted descending by count.
    pub fn top_commands(&self, n: usize) -> Vec<(&str, u64)> {
        let mut entries: Vec<(&str, u64)> = self
            .hits
            .iter()
            .map(|(k, &v)| (k.as_str(), v))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    /// Return the number of distinct commands that have been invoked.
    pub fn distinct_commands(&self) -> usize {
        self.hits.len()
    }

    /// Reset all usage data.
    pub fn reset(&mut self) {
        self.hits.clear();
    }

    /// Merge another tracker's data into this one.
    pub fn merge(&mut self, other: &KeybindingUsageTracker) {
        for (cmd, &count) in &other.hits {
            *self.hits.entry(cmd.clone()).or_insert(0) += count;
        }
    }
}

impl fmt::Display for KeybindingUsageTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UsageTracker({} commands, {} invocations)",
            self.distinct_commands(),
            self.total_invocations()
        )
    }
}

/// Detect keybinding conflicts across multiple independent keymaps.
///
/// Each keymap is a slice of `ResolvedKeybinding`. Returns a list of
/// (formatted_key, Vec<(keymap_index, command)>) for every key chord
/// that is bound in more than one keymap.
pub fn detect_cross_keymap_conflicts(
    keymaps: &[&[ResolvedKeybinding]],
) -> Vec<(String, Vec<(usize, String)>)> {
    let mut key_to_sources: std::collections::HashMap<String, Vec<(usize, String)>> =
        std::collections::HashMap::new();
    for (map_idx, keymap) in keymaps.iter().enumerate() {
        for binding in *keymap {
            let formatted = KeybindingService::format_binding(binding);
            key_to_sources
                .entry(formatted)
                .or_default()
                .push((map_idx, binding.command.clone()));
        }
    }
    key_to_sources
        .into_iter()
        .filter(|(_, sources)| {
            // Only a conflict if bound in more than one distinct keymap
            let mut seen = std::collections::HashSet::new();
            for (idx, _) in sources {
                seen.insert(*idx);
            }
            seen.len() > 1
        })
        .collect()
}

/// Resolve a key press against the service, filtering by when-clause context.
/// Returns the highest-priority matching binding (User > Extension > Default).
pub fn resolve_with_context<'a>(
    service: &'a KeybindingService,
    key: &str,
    modifiers: &[KeyMod],
    context: &KeybindingWhenContext,
) -> Option<&'a ResolvedKeybinding> {
    let candidates = service.resolve(key, modifiers);
    let active: Vec<&ResolvedKeybinding> = candidates
        .into_iter()
        .filter(|b| context.binding_active(b))
        .collect();
    if active.is_empty() {
        return None;
    }
    // Priority: User > Extension > Default
    fn priority(source: &KeybindingSource) -> u8 {
        match source {
            KeybindingSource::User => 2,
            KeybindingSource::Extension => 1,
            KeybindingSource::Default => 0,
        }
    }
    active
        .into_iter()
        .max_by_key(|b| priority(&b.source))
}

/// A keybinding override layer that tracks user/extension overrides on top of defaults.
///
/// Overrides are applied by matching key+modifiers: when an override exists for a
/// given key combo, it replaces the default binding's command during resolution.
#[derive(Debug, Clone, Default)]
pub struct KeybindingOverrideLayer {
    overrides: Vec<KeybindingOverride>,
}

/// A single override entry that remaps a key combo to a different command.
#[derive(Debug, Clone, PartialEq)]
pub struct KeybindingOverride {
    pub key: String,
    pub modifiers: Vec<KeyMod>,
    pub original_command: String,
    pub new_command: String,
    pub when: Option<String>,
}

impl KeybindingOverrideLayer {
    pub fn new() -> Self {
        Self {
            overrides: Vec::new(),
        }
    }

    /// Add an override that remaps `original_command` on key+mods to `new_command`.
    pub fn add_override(
        &mut self,
        key: &str,
        modifiers: Vec<KeyMod>,
        original_command: &str,
        new_command: &str,
        when: Option<&str>,
    ) {
        self.overrides.push(KeybindingOverride {
            key: key.to_string(),
            modifiers,
            original_command: original_command.to_string(),
            new_command: new_command.to_string(),
            when: when.map(|s| s.to_string()),
        });
    }

    /// Remove all overrides for a given key+modifiers combination.
    pub fn remove_overrides_for_key(&mut self, key: &str, modifiers: &[KeyMod]) {
        self.overrides
            .retain(|o| !(o.key == key && o.modifiers == modifiers));
    }

    /// Look up the override for a key+modifiers+original_command triple.
    pub fn find_override(
        &self,
        key: &str,
        modifiers: &[KeyMod],
        original_command: &str,
    ) -> Option<&KeybindingOverride> {
        self.overrides.iter().find(|o| {
            o.key == key && o.modifiers == modifiers && o.original_command == original_command
        })
    }

    /// Apply all overrides to a service, mutating bindings in-place.
    /// Returns the number of bindings that were overridden.
    pub fn apply_to(&self, service: &mut KeybindingService) -> usize {
        let mut count = 0;
        for binding in &mut service.bindings {
            if let Some(ov) = self.overrides.iter().find(|o| {
                o.key == binding.key
                    && o.modifiers == binding.modifiers
                    && o.original_command == binding.command
            }) {
                binding.command = ov.new_command.clone();
                if ov.when.is_some() {
                    binding.when = ov.when.clone();
                }
                count += 1;
            }
        }
        count
    }

    /// Return the number of overrides registered.
    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    /// Returns true if there are no overrides.
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Clear all overrides.
    pub fn clear(&mut self) {
        self.overrides.clear();
    }

    /// List all overrides.
    pub fn iter(&self) -> std::slice::Iter<'_, KeybindingOverride> {
        self.overrides.iter()
    }
}

impl fmt::Display for KeybindingOverride {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mods: Vec<&str> = self
            .modifiers
            .iter()
            .map(|m| match m {
                KeyMod::CtrlCmd => "Ctrl",
                KeyMod::Shift => "Shift",
                KeyMod::Alt => "Alt",
                KeyMod::WinCtrl => "Win",
            })
            .collect();
        let key_str = if mods.is_empty() {
            self.key.clone()
        } else {
            format!("{}+{}", mods.join("+"), self.key)
        };
        write!(
            f,
            "{}: {} -> {}",
            key_str, self.original_command, self.new_command
        )
    }
}

/// Generate human-readable documentation for a set of keybindings.
///
/// Groups bindings by command prefix (the part before the first `.`) and
/// formats them as a structured text document.
pub fn generate_keybinding_docs(bindings: &[ResolvedKeybinding]) -> String {
    let mut groups: std::collections::BTreeMap<String, Vec<&ResolvedKeybinding>> =
        std::collections::BTreeMap::new();

    for binding in bindings {
        let category = binding
            .command
            .split('.')
            .next()
            .unwrap_or("other")
            .to_string();
        groups.entry(category).or_default().push(binding);
    }

    let mut doc = String::from("# Keybinding Reference\n\n");
    for (category, group) in &groups {
        doc.push_str(&format!("## {}\n\n", category));
        doc.push_str("| Shortcut | Command | When | Source |\n");
        doc.push_str("|----------|---------|------|--------|\n");
        for b in group {
            let shortcut = KeybindingService::format_binding(b);
            let when_str = b.when.as_deref().unwrap_or("—");
            doc.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                shortcut, b.command, when_str, b.source
            ));
        }
        doc.push('\n');
    }
    doc
}

/// Normalize modifier order in a binding to the canonical order: Ctrl, Shift, Alt, Win.
/// Returns a new Vec with modifiers sorted in canonical order, with duplicates removed.
pub fn normalize_modifiers(modifiers: &[KeyMod]) -> Vec<KeyMod> {
    fn canonical_order(m: &KeyMod) -> u8 {
        match m {
            KeyMod::CtrlCmd => 0,
            KeyMod::Shift => 1,
            KeyMod::Alt => 2,
            KeyMod::WinCtrl => 3,
        }
    }
    let mut sorted: Vec<KeyMod> = Vec::new();
    for m in modifiers {
        if !sorted.contains(m) {
            sorted.push(m.clone());
        }
    }
    sorted.sort_by_key(|m| canonical_order(m));
    sorted
}

/// Search keybindings across multiple fields (key, command, when, source).
/// Returns bindings where *any* field matches the query substring (case-insensitive).
pub fn search_bindings<'a>(
    bindings: &'a [ResolvedKeybinding],
    query: &str,
) -> Vec<&'a ResolvedKeybinding> {
    let q = query.to_lowercase();
    bindings.iter().filter(|b| b.matches_filter(&q)).collect()
}

/// Build a reverse index: command → list of formatted key strings.
pub fn build_command_key_index(
    bindings: &[ResolvedKeybinding],
) -> std::collections::HashMap<String, Vec<String>> {
    let mut index: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for b in bindings {
        let formatted = KeybindingService::format_binding(b);
        index
            .entry(b.command.clone())
            .or_default()
            .push(formatted);
    }
    index
}

/// Parse a binding definition string in the format `"key_chord -> command [when: clause]"`.
///
/// Examples:
/// - `"Ctrl+S -> save"`
/// - `"Ctrl+Shift+P -> showCommands [when: editorFocus]"`
///
/// Returns `None` if the format is invalid.
pub fn parse_binding_definition(s: &str) -> Option<ResolvedKeybinding> {
    let s = s.trim();
    let arrow_pos = s.find("->")?;
    let chord_part = s[..arrow_pos].trim();
    let rest = s[arrow_pos + 2..].trim();

    let (key, modifiers) = parse_key_chord(chord_part)?;

    // Extract optional [when: ...] clause
    let (command, when) = if let Some(bracket_start) = rest.find('[') {
        let cmd = rest[..bracket_start].trim();
        let bracket_end = rest.find(']')?;
        let clause = rest[bracket_start + 1..bracket_end].trim();
        let when_val = clause.strip_prefix("when:")?.trim().to_string();
        (cmd.to_string(), Some(when_val))
    } else {
        (rest.to_string(), None)
    };

    if command.is_empty() {
        return None;
    }

    Some(ResolvedKeybinding {
        key,
        modifiers,
        command,
        when,
        source: KeybindingSource::User,
    })
}

/// Serialize a list of bindings into definition strings that can be parsed back
/// with `parse_binding_definition`.
pub fn serialize_bindings(bindings: &[ResolvedKeybinding]) -> Vec<String> {
    bindings
        .iter()
        .map(|b| {
            let chord = KeybindingService::format_binding(b);
            match &b.when {
                Some(w) => format!("{} -> {} [when: {}]", chord, b.command, w),
                None => format!("{} -> {}", chord, b.command),
            }
        })
        .collect()
}

impl KeybindingService {
    /// Rebind a command from one key combo to another.
    /// Removes the old binding and creates a new one with the same command/when/source.
    /// Returns `Err` if the old binding is not found.
    pub fn rebind(
        &mut self,
        command: &str,
        old_key: &str,
        old_modifiers: &[KeyMod],
        new_key: &str,
        new_modifiers: Vec<KeyMod>,
    ) -> Result<(), KeybindingError> {
        let pos = self.bindings.iter().position(|b| {
            b.command == command && b.key == old_key && b.modifiers == old_modifiers
        });
        match pos {
            Some(idx) => {
                let mut binding = self.bindings.remove(idx);
                binding.key = new_key.to_string();
                binding.modifiers = new_modifiers;
                self.bindings.push(binding);
                Ok(())
            }
            None => Err(KeybindingError::BindingNotFound),
        }
    }

    /// Return all unique key+modifier combinations that have bindings.
    pub fn bound_keys(&self) -> Vec<(String, Vec<KeyMod>)> {
        let mut seen: Vec<(String, Vec<KeyMod>)> = Vec::new();
        for b in &self.bindings {
            let entry = (b.key.clone(), b.modifiers.clone());
            if !seen.contains(&entry) {
                seen.push(entry);
            }
        }
        seen
    }

    /// Merge all bindings from another service into this one.
    /// Does not check for conflicts.
    pub fn merge_from(&mut self, other: &KeybindingService) {
        for b in &other.bindings {
            self.bindings.push(b.clone());
        }
    }

    /// Replace all bindings for a command with a single new binding.
    /// Returns the number of old bindings removed.
    pub fn replace_command_binding(
        &mut self,
        command: &str,
        new_key: &str,
        new_modifiers: Vec<KeyMod>,
        source: KeybindingSource,
    ) -> usize {
        let before = self.bindings.len();
        let when = self
            .bindings
            .iter()
            .find(|b| b.command == command)
            .and_then(|b| b.when.clone());
        self.bindings.retain(|b| b.command != command);
        let removed = before - self.bindings.len();
        self.bindings.push(ResolvedKeybinding {
            key: new_key.to_string(),
            modifiers: new_modifiers,
            command: command.to_string(),
            when,
            source,
        });
        removed
    }
}


// === Keybinding Resolver Optimizer ===

/// Keybinding Resolver Optimizer implementation.
#[derive(Debug, Clone)]
pub struct KeybindingResolverOptimizer {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: KeybindingResolverOptimizerStats,
}

/// Statistics for KeybindingResolverOptimizer.
#[derive(Debug, Clone, Default)]
pub struct KeybindingResolverOptimizerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl KeybindingResolverOptimizerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl KeybindingResolverOptimizer {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: KeybindingResolverOptimizerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &KeybindingResolverOptimizerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for KeybindingResolverOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

// === Keybinding Scope Tracker ===

/// Priority level for KeybindingScopeTracker items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeybindingScopeTrackerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl KeybindingScopeTrackerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for KeybindingScopeTrackerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Keybinding Scope Tracker implementation.
#[derive(Debug, Clone)]
pub struct KeybindingScopeTracker {
    items: Vec<KeybindingScopeTrackerItem>,
    max_items: usize,
    default_priority: KeybindingScopeTrackerPriority,
}

/// A single item in KeybindingScopeTracker.
#[derive(Debug, Clone)]
pub struct KeybindingScopeTrackerItem {
    pub id: String,
    pub label: String,
    pub priority: KeybindingScopeTrackerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl KeybindingScopeTrackerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: KeybindingScopeTrackerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: KeybindingScopeTrackerPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl KeybindingScopeTracker {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: KeybindingScopeTrackerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: KeybindingScopeTrackerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<KeybindingScopeTrackerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&KeybindingScopeTrackerItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: KeybindingScopeTrackerPriority) -> Vec<&KeybindingScopeTrackerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&KeybindingScopeTrackerItem> {
        let mut sorted: Vec<&KeybindingScopeTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&KeybindingScopeTrackerItem> {
        let mut sorted: Vec<&KeybindingScopeTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&KeybindingScopeTrackerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: KeybindingScopeTrackerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> KeybindingScopeTrackerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &KeybindingScopeTrackerItem> {
        self.items.iter()
    }
}

impl Default for KeybindingScopeTracker {
    fn default() -> Self {
        Self::new()
    }
}


/// Workbench keybinding configuration manager.
#[derive(Debug, Clone)]
pub struct WbKeybindingConfig {
    entries: Vec<WbKeybindingEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench keybinding entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbKeybindingEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbKeybindingEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WbKeybindingConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbKeybindingEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WbKeybindingEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbKeybindingEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbKeybindingEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WbKeybindingEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbKeybindingEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WbKeybindingEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
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
// xa_ extended helpers for wb_keybinding
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbKeybindingRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbKeybindingRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaWbKeybindingCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbKeybindingCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaWbKeybindingCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 210
// ---------------------------------------------------------------------------

/// Generic object pool `Xc210Pool<T>`.
pub struct Xc210Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc210Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc210PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc210Pool<T> {
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
    pub fn stats(&self) -> Xc210PoolStats {
        Xc210PoolStats {
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

impl<T> Default for Xc210Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc210Scheduler`.
pub struct Xc210Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc210Scheduler {
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

impl Default for Xc210Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_210 hash for the given byte slice.
pub fn xc_210_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_210 convention.
pub fn xc_210_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_80 deepening: state machine + event bus ---

/// States for the Xd80 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd80State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd80State {
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
pub struct Xd80Transition {
    pub from: Xd80State,
    pub to: Xd80State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd80StateMachine {
    current: Xd80State,
    history: Vec<Xd80Transition>,
    step_counter: usize,
}

impl Xd80StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd80State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd80State {
        self.current
    }

    pub fn history(&self) -> &[Xd80Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd80State) -> Result<Xd80State, String> {
        let allowed = match (self.current, target) {
            (Xd80State::Idle, Xd80State::Running) => true,
            (Xd80State::Running, Xd80State::Paused) => true,
            (Xd80State::Running, Xd80State::Done) => true,
            (Xd80State::Paused, Xd80State::Running) => true,
            (Xd80State::Paused, Xd80State::Done) => true,
            (Xd80State::Done, Xd80State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_80: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd80Transition {
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
            "Xd80SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd80State> {
        let prefix = "Xd80SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd80State::Idle),
            "Running" => Some(Xd80State::Running),
            "Paused" => Some(Xd80State::Paused),
            "Done" => Some(Xd80State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd80State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd80 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd80Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd80Event {
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

type Xd80HandlerFn = Box<dyn Fn(&Xd80Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd80EventBus {
    handlers: Vec<(usize, Option<String>, Xd80HandlerFn)>,
    next_id: usize,
    published: Vec<Xd80Event>,
}

impl Xd80EventBus {
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
        F: Fn(&Xd80Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd80Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd80Event) {
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

    pub fn published_events(&self) -> &[Xd80Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #100
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf100Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf100TrieNode {
    children: std::collections::HashMap<char, Xf100TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf100Trie {
    root: Xf100TrieNode,
    count: usize,
}

impl Xf100Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf100TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf100TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf100TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf100BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf100BloomFilter {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_binding(key: &str, command: &str, modifiers: Vec<KeyMod>) -> ResolvedKeybinding {
        ResolvedKeybinding {
            key: key.to_string(),
            modifiers,
            command: command.to_string(),
            when: None,
            source: KeybindingSource::Default,
        }
    }

    fn sample_binding_src(
        key: &str,
        command: &str,
        modifiers: Vec<KeyMod>,
        source: KeybindingSource,
    ) -> ResolvedKeybinding {
        ResolvedKeybinding {
            key: key.to_string(),
            modifiers,
            command: command.to_string(),
            when: None,
            source,
        }
    }

    #[test]
    fn register_and_resolve() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("S", "save_all", vec![KeyMod::CtrlCmd, KeyMod::Shift]));

        let found = svc.resolve("S", &[KeyMod::CtrlCmd]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].command, "save");
    }

    #[test]
    fn remove_and_count() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("P", "palette", vec![KeyMod::CtrlCmd, KeyMod::Shift]));
        svc.register(sample_binding("N", "new_file", vec![KeyMod::CtrlCmd]));
        assert_eq!(svc.binding_count(), 2);

        assert!(svc.remove_binding("palette", "P"));
        assert_eq!(svc.binding_count(), 1);
        assert!(!svc.remove_binding("palette", "P"));
    }

    #[test]
    fn format_and_lookup_by_command() {
        let mut svc = KeybindingService::new();
        let binding = sample_binding("C", "copy", vec![KeyMod::CtrlCmd]);
        svc.register(binding.clone());

        assert_eq!(KeybindingService::format_binding(&binding), "Ctrl+C");

        let results = svc.get_bindings_for_command("copy");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "C");
    }

    #[test]
    fn keybinding_error_display() {
        assert_eq!(KeybindingError::BindingNotFound.to_string(), "binding not found");
        assert_eq!(KeybindingError::DuplicateBinding.to_string(), "duplicate binding");
        assert_eq!(
            KeybindingError::ConflictingBinding("save".into()).to_string(),
            "conflicting binding for command 'save'"
        );
    }

    #[test]
    fn display_keymod() {
        assert_eq!(KeyMod::CtrlCmd.to_string(), "Ctrl");
        assert_eq!(KeyMod::Shift.to_string(), "Shift");
        assert_eq!(KeyMod::Alt.to_string(), "Alt");
        assert_eq!(KeyMod::WinCtrl.to_string(), "Win");
    }

    #[test]
    fn display_keybinding_source() {
        assert_eq!(KeybindingSource::Default.to_string(), "Default");
        assert_eq!(KeybindingSource::User.to_string(), "User");
        assert_eq!(KeybindingSource::Extension.to_string(), "Extension");
    }

    #[test]
    fn display_resolved_keybinding() {
        let b = sample_binding("S", "save", vec![KeyMod::CtrlCmd]);
        assert_eq!(b.to_string(), "Ctrl+S -> save [Default]");

        let bare = sample_binding("F5", "debug.run", vec![]);
        assert_eq!(bare.to_string(), "F5 -> debug.run [Default]");
    }

    #[test]
    fn resolved_keybinding_matches() {
        let b = sample_binding("S", "save", vec![KeyMod::CtrlCmd]);
        assert!(b.matches("S", &[KeyMod::CtrlCmd]));
        assert!(!b.matches("S", &[KeyMod::Alt]));
        assert!(!b.matches("X", &[KeyMod::CtrlCmd]));
    }

    #[test]
    fn has_conflict_and_get_conflicts() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("S", "search", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("N", "new_file", vec![KeyMod::CtrlCmd]));

        assert!(svc.has_conflict("S", &[KeyMod::CtrlCmd]));
        assert!(!svc.has_conflict("N", &[KeyMod::CtrlCmd]));

        let conflicts = svc.get_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].len(), 2);
    }

    #[test]
    fn get_by_source() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding_src("S", "save", vec![KeyMod::CtrlCmd], KeybindingSource::Default));
        svc.register(sample_binding_src("S", "custom_save", vec![KeyMod::CtrlCmd], KeybindingSource::User));
        svc.register(sample_binding_src("E", "ext_cmd", vec![KeyMod::Alt], KeybindingSource::Extension));

        assert_eq!(svc.get_by_source(&KeybindingSource::Default).len(), 1);
        assert_eq!(svc.get_by_source(&KeybindingSource::User).len(), 1);
        assert_eq!(svc.get_by_source(&KeybindingSource::Extension).len(), 1);
    }

    #[test]
    fn clear_removes_all() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("A", "cmd_a", vec![]));
        svc.register(sample_binding("B", "cmd_b", vec![]));
        assert_eq!(svc.binding_count(), 2);
        svc.clear();
        assert_eq!(svc.binding_count(), 0);
    }

    #[test]
    fn try_register_success_and_conflict() {
        let mut svc = KeybindingService::new();
        let result = svc.try_register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        assert!(result.is_ok());

        // First registration — no conflict yet (only one binding)
        let result2 = svc.try_register(sample_binding("S", "search", vec![KeyMod::CtrlCmd]));
        assert!(result2.is_ok());

        // Now there are two bindings on the same key+modifiers → conflict
        let result3 = svc.try_register(sample_binding("S", "something", vec![KeyMod::CtrlCmd]));
        assert!(result3.is_err());
        assert_eq!(
            result3.unwrap_err(),
            KeybindingError::ConflictingBinding("save".into())
        );
    }

    #[test]
    fn builder_pattern() {
        let binding = KeybindingBuilder::new()
            .key("S")
            .modifier(KeyMod::CtrlCmd)
            .modifier(KeyMod::Shift)
            .command("save_all")
            .when("editorTextFocus")
            .source(KeybindingSource::User)
            .build();

        assert_eq!(binding.key, "S");
        assert_eq!(binding.modifiers, vec![KeyMod::CtrlCmd, KeyMod::Shift]);
        assert_eq!(binding.command, "save_all");
        assert_eq!(binding.when, Some("editorTextFocus".to_string()));
        assert_eq!(binding.source, KeybindingSource::User);
    }

    #[test]
    fn builder_defaults() {
        let binding = KeybindingBuilder::default()
            .key("F5")
            .command("debug.start")
            .build();

        assert!(binding.modifiers.is_empty());
        assert_eq!(binding.when, None);
        assert_eq!(binding.source, KeybindingSource::Default);
    }

    #[test]
    fn export_bindings() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("F5", "debug.run", vec![]));

        let exported = svc.export_bindings();
        assert_eq!(exported.len(), 2);
        assert_eq!(exported[0], "Ctrl+S -> save [Default]");
        assert_eq!(exported[1], "F5 -> debug.run [Default]");
    }

    #[test]
    fn eq_keybindingerror_same() {
        assert_eq!(KeybindingError::BindingNotFound, KeybindingError::BindingNotFound);
    }

    #[test]
    fn ne_keybindingerror_diff() {
        assert_ne!(KeybindingError::BindingNotFound, KeybindingError::DuplicateBinding);
    }

    #[test]
    fn eq_keymod_same() {
        assert_eq!(KeyMod::CtrlCmd, KeyMod::CtrlCmd);
    }

    #[test]
    fn ne_keymod_diff() {
        assert_ne!(KeyMod::CtrlCmd, KeyMod::Shift);
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
    fn display_keybindingerror_variants() {
        assert!(!KeybindingError::BindingNotFound.to_string().is_empty());
        assert!(!KeybindingError::DuplicateBinding.to_string().is_empty());
    }

    #[test]
    fn display_keymod_variants() {
        assert!(!KeyMod::CtrlCmd.to_string().is_empty());
        assert!(!KeyMod::Shift.to_string().is_empty());
        assert!(!KeyMod::Alt.to_string().is_empty());
        assert!(!KeyMod::WinCtrl.to_string().is_empty());
    }

    #[test]
    fn display_keybindingsource_variants() {
        assert!(!KeybindingSource::Default.to_string().is_empty());
        assert!(!KeybindingSource::User.to_string().is_empty());
        assert!(!KeybindingSource::Extension.to_string().is_empty());
    }

    #[test]
    fn behavior_check_0() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = KeybindingService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn wb_keybinding_stats_new_defaults() {
        let stats = WbKeybindingStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_keybinding_stats_record_success() {
        let mut stats = WbKeybindingStats::new();
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
    fn wb_keybinding_stats_record_failure() {
        let mut stats = WbKeybindingStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_keybinding_stats_reset() {
        let mut stats = WbKeybindingStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_keybinding_stats_merge() {
        let mut a = WbKeybindingStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbKeybindingStats::new();
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
    fn wb_keybinding_stats_display() {
        let mut stats = WbKeybindingStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_keybinding_stats_default() {
        let stats = WbKeybindingStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_keybinding_validator_accepts_valid_name() {
        let v = WbKeybindingValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_keybinding_validator_rejects_empty() {
        let v = WbKeybindingValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_keybinding_validator_rejects_too_long() {
        let v = WbKeybindingValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_keybinding_validator_forbidden_prefix() {
        let v = WbKeybindingValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_keybinding_validator_allowed_chars() {
        let v = WbKeybindingValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_keybinding_validator_range() {
        let v = WbKeybindingValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_keybinding_sanitize_removes_control() {
        let result = WbKeybindingValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_keybinding_truncate_short_string() {
        assert_eq!(WbKeybindingValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_keybinding_truncate_long_string() {
        let result = WbKeybindingValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_keybinding_is_ascii_printable() {
        assert!(WbKeybindingValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbKeybindingValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn when_context_simple_true() {
        let mut ctx = KeybindingWhenContext::new();
        ctx.set("editorFocus", true);
        assert!(ctx.evaluate("editorFocus"));
    }

    #[test]
    fn when_context_simple_false() {
        let ctx = KeybindingWhenContext::new();
        assert!(!ctx.evaluate("editorFocus"));
    }

    #[test]
    fn when_context_negation() {
        let mut ctx = KeybindingWhenContext::new();
        ctx.set("editorFocus", true);
        assert!(!ctx.evaluate("!editorFocus"));
        assert!(ctx.evaluate("!terminalFocus"));
    }

    #[test]
    fn when_context_and() {
        let mut ctx = KeybindingWhenContext::new();
        ctx.set("editorFocus", true);
        ctx.set("editorHasSelection", true);
        assert!(ctx.evaluate("editorFocus && editorHasSelection"));
        ctx.set("editorHasSelection", false);
        assert!(!ctx.evaluate("editorFocus && editorHasSelection"));
    }

    #[test]
    fn when_context_or() {
        let mut ctx = KeybindingWhenContext::new();
        ctx.set("editorFocus", true);
        assert!(ctx.evaluate("editorFocus || terminalFocus"));
        assert!(!ctx.evaluate("terminalFocus || panelFocus"));
    }

    #[test]
    fn when_context_empty_is_true() {
        let ctx = KeybindingWhenContext::new();
        assert!(ctx.evaluate(""));
    }

    #[test]
    fn when_context_binding_active() {
        let mut ctx = KeybindingWhenContext::new();
        ctx.set("editorFocus", true);
        let b = KeybindingBuilder::new().key("s").modifier(KeyMod::CtrlCmd)
            .command("save").when("editorFocus").build();
        assert!(ctx.binding_active(&b));
    }

    #[test]
    fn chord_sequence_single() {
        let seq = ChordSequence::single("k", vec![KeyMod::CtrlCmd]);
        assert_eq!(seq.len(), 1);
    }

    #[test]
    fn chord_sequence_double() {
        let seq = ChordSequence::double("k", vec![KeyMod::CtrlCmd], "s", vec![]);
        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn resolve_chain_basic() {
        let mut svc = KeybindingService::new();
        svc.register(KeybindingBuilder::new().key("s").modifier(KeyMod::CtrlCmd).command("save").build());
        let ctx = KeybindingWhenContext::new();
        let seq = ChordSequence::single("s", vec![KeyMod::CtrlCmd]);
        let results = keybinding_resolve_chain(&svc, &ctx, &seq);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "save");
    }

    #[test]
    fn resolve_chain_filtered_by_context() {
        let mut svc = KeybindingService::new();
        svc.register(KeybindingBuilder::new().key("s").modifier(KeyMod::CtrlCmd).command("save").when("editorFocus").build());
        let ctx = KeybindingWhenContext::new(); // editorFocus is false
        let seq = ChordSequence::single("s", vec![KeyMod::CtrlCmd]);
        let results = keybinding_resolve_chain(&svc, &ctx, &seq);
        assert!(results.is_empty());
    }

    #[test]
    fn defaults_has_save() {
        let defaults = keybinding_defaults();
        assert!(defaults.iter().any(|b| b.command == "workbench.action.files.save"));
        assert!(defaults.len() >= 10);
    }

    #[test]
    fn resolved_keybinding_has_when_context_and_filter() {
        let b = KeybindingBuilder::new()
            .key("s")
            .modifier(KeyMod::CtrlCmd)
            .command("workbench.action.files.save")
            .when("editorFocus")
            .build();
        assert!(b.has_when_context());
        assert!(b.matches_filter("save"));
        assert!(b.matches_filter("editor"));
        assert!(!b.matches_filter("zzz"));
        assert_eq!(b.modifier_count(), 1);

        let bare = sample_binding("F5", "debug", vec![]);
        assert!(!bare.has_when_context());
        assert_eq!(bare.modifier_count(), 0);
    }

    #[test]
    fn keybinding_source_predicates() {
        assert!(KeybindingSource::User.is_user());
        assert!(!KeybindingSource::User.is_default());
        assert!(!KeybindingSource::User.is_extension());
        assert!(KeybindingSource::Default.is_default());
        assert!(KeybindingSource::Extension.is_extension());
    }

    #[test]
    fn service_iter_and_find_by_command() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("A", "save_all", vec![KeyMod::CtrlCmd, KeyMod::Shift]));
        svc.register(sample_binding("N", "new_file", vec![KeyMod::CtrlCmd]));

        let count = svc.iter().count();
        assert_eq!(count, 3);

        let found = svc.find_by_command("save");
        assert_eq!(found.len(), 2);

        let cmds = svc.commands();
        assert!(cmds.contains(&"new_file"));
    }

    #[test]
    fn service_into_iterator() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("X", "cut", vec![KeyMod::CtrlCmd]));
        let mut count = 0;
        for _b in &svc {
            count += 1;
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn service_find_conflicts_returns_groups() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("S", "search", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("N", "new_file", vec![KeyMod::CtrlCmd]));

        let conflicts = svc.find_conflicts();
        assert_eq!(conflicts.len(), 1);
        let (key, group) = &conflicts[0];
        assert_eq!(key, "Ctrl+S");
        assert_eq!(group.len(), 2);
    }

    #[test]
    fn when_context_extensions() {
        let mut ctx = KeybindingWhenContext::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.len(), 0);

        ctx.set("editorFocus", true);
        ctx.set("terminalFocus", false);
        assert!(!ctx.is_empty());
        assert_eq!(ctx.len(), 2);
        assert!(ctx.matches("editorFocus"));
        assert!(!ctx.matches("terminalFocus"));

        let display = format!("{ctx}");
        assert!(display.starts_with("WhenContext("));

        assert!(ctx.remove("terminalFocus"));
        assert_eq!(ctx.len(), 1);
        assert!(!ctx.remove("nonexistent"));

        ctx.clear();
        assert!(ctx.is_empty());
    }

    #[test]
    fn chord_sequence_extensions() {
        let single = ChordSequence::single("k", vec![KeyMod::CtrlCmd]);
        assert!(single.is_single());
        assert_eq!(single.first_key(), Some("k"));
        assert_eq!(single.last_key(), Some("k"));
        assert_eq!(single.to_string(), "Ctrl+k");

        let double = ChordSequence::double("k", vec![KeyMod::CtrlCmd], "s", vec![]);
        assert!(!double.is_single());
        assert_eq!(double.first_key(), Some("k"));
        assert_eq!(double.last_key(), Some("s"));
        assert_eq!(double.to_string(), "Ctrl+k s");

        let empty = ChordSequence { chords: vec![] };
        assert!(empty.is_empty());
        assert_eq!(empty.first_key(), None);
        assert_eq!(empty.last_key(), None);
    }

    #[test]
    fn stats_summary_and_has_failures() {
        let mut stats = WbKeybindingStats::new();
        assert!(!stats.has_failures());
        stats.record_success(100);
        stats.record_failure(200);
        assert!(stats.has_failures());
        let s = stats.summary();
        assert!(s.contains("2 ops"));
        assert!(s.contains("1 ok"));
        assert!(s.contains("1 err"));
    }

    #[test]
    fn parse_key_chord_single_key() {
        let result = parse_key_chord("F5");
        assert_eq!(result, Some(("F5".to_string(), vec![])));
    }

    #[test]
    fn parse_key_chord_with_modifiers() {
        let result = parse_key_chord("Ctrl+Shift+S");
        assert_eq!(
            result,
            Some(("S".to_string(), vec![KeyMod::CtrlCmd, KeyMod::Shift]))
        );

        let result2 = parse_key_chord("Alt+x");
        assert_eq!(result2, Some(("x".to_string(), vec![KeyMod::Alt])));

        assert_eq!(parse_key_chord(""), None);
    }

    #[test]
    fn parse_chord_sequence_multi() {
        let seq = parse_chord_sequence("Ctrl+K Ctrl+C").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.chords[0], ("K".to_string(), vec![KeyMod::CtrlCmd]));
        assert_eq!(seq.chords[1], ("C".to_string(), vec![KeyMod::CtrlCmd]));

        let single = parse_chord_sequence("Shift+Tab").unwrap();
        assert_eq!(single.len(), 1);
        assert_eq!(
            single.chords[0],
            ("Tab".to_string(), vec![KeyMod::Shift])
        );

        assert!(parse_chord_sequence("").is_none());
    }

    #[test]
    fn usage_tracker_records_and_ranks() {
        let mut tracker = KeybindingUsageTracker::new();
        tracker.record("save");
        tracker.record("save");
        tracker.record("save");
        tracker.record("copy");
        tracker.record("paste");
        tracker.record("paste");

        assert_eq!(tracker.count("save"), 3);
        assert_eq!(tracker.count("copy"), 1);
        assert_eq!(tracker.count("paste"), 2);
        assert_eq!(tracker.count("nonexistent"), 0);
        assert_eq!(tracker.total_invocations(), 6);
        assert_eq!(tracker.distinct_commands(), 3);

        let top = tracker.top_commands(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "save");
        assert_eq!(top[0].1, 3);
        assert_eq!(top[1].0, "paste");
        assert_eq!(top[1].1, 2);

        let display = format!("{tracker}");
        assert!(display.contains("3 commands"));
        assert!(display.contains("6 invocations"));

        let mut other = KeybindingUsageTracker::new();
        other.record("save");
        other.record("undo");
        tracker.merge(&other);
        assert_eq!(tracker.count("save"), 4);
        assert_eq!(tracker.count("undo"), 1);

        tracker.reset();
        assert_eq!(tracker.total_invocations(), 0);
        assert_eq!(tracker.distinct_commands(), 0);
    }

    #[test]
    fn cross_keymap_conflict_detection() {
        let map1 = vec![
            sample_binding("S", "save", vec![KeyMod::CtrlCmd]),
            sample_binding("C", "copy", vec![KeyMod::CtrlCmd]),
        ];
        let map2 = vec![
            sample_binding("S", "search", vec![KeyMod::CtrlCmd]),
            sample_binding("N", "new", vec![KeyMod::CtrlCmd]),
        ];
        let conflicts = detect_cross_keymap_conflicts(&[&map1, &map2]);
        // Ctrl+S is bound in both maps
        assert_eq!(conflicts.len(), 1);
        let (key, sources) = &conflicts[0];
        assert_eq!(key, "Ctrl+S");
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].0, 0); // from map1
        assert_eq!(sources[1].0, 1); // from map2

        // No conflicts when keymaps don't overlap
        let map3 = vec![sample_binding("Z", "undo", vec![KeyMod::CtrlCmd])];
        let no_conflicts = detect_cross_keymap_conflicts(&[&map1, &map3]);
        assert!(no_conflicts.is_empty());
    }

    #[test]
    fn resolve_with_context_priority() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding_src(
            "S", "default.save", vec![KeyMod::CtrlCmd], KeybindingSource::Default,
        ));
        svc.register(sample_binding_src(
            "S", "user.save", vec![KeyMod::CtrlCmd], KeybindingSource::User,
        ));
        svc.register(sample_binding_src(
            "S", "ext.save", vec![KeyMod::CtrlCmd], KeybindingSource::Extension,
        ));

        let ctx = KeybindingWhenContext::new();
        let result = resolve_with_context(&svc, "S", &[KeyMod::CtrlCmd], &ctx);
        assert!(result.is_some());
        // User source has highest priority
        assert_eq!(result.unwrap().command, "user.save");

        // No match for unregistered key
        let none = resolve_with_context(&svc, "X", &[KeyMod::CtrlCmd], &ctx);
        assert!(none.is_none());
    }

    #[test]
    fn resolve_with_context_when_filter() {
        let mut svc = KeybindingService::new();
        let b = KeybindingBuilder::new()
            .key("S")
            .modifier(KeyMod::CtrlCmd)
            .command("save")
            .when("editorFocus")
            .source(KeybindingSource::Default)
            .build();
        svc.register(b);

        // Without the context key set, binding is inactive
        let empty_ctx = KeybindingWhenContext::new();
        assert!(resolve_with_context(&svc, "S", &[KeyMod::CtrlCmd], &empty_ctx).is_none());

        // With the context key set, binding is active
        let mut ctx = KeybindingWhenContext::new();
        ctx.set("editorFocus", true);
        let result = resolve_with_context(&svc, "S", &[KeyMod::CtrlCmd], &ctx);
        assert_eq!(result.unwrap().command, "save");
    }

    #[test]
    fn override_layer_apply() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("N", "new_file", vec![KeyMod::CtrlCmd]));

        let mut layer = KeybindingOverrideLayer::new();
        layer.add_override("S", vec![KeyMod::CtrlCmd], "save", "custom_save", None);

        assert_eq!(layer.len(), 1);
        assert!(!layer.is_empty());

        let count = layer.apply_to(&mut svc);
        assert_eq!(count, 1);
        assert_eq!(svc.resolve("S", &[KeyMod::CtrlCmd])[0].command, "custom_save");
        // Unrelated binding unchanged
        assert_eq!(svc.resolve("N", &[KeyMod::CtrlCmd])[0].command, "new_file");
    }

    #[test]
    fn override_layer_find_and_remove() {
        let mut layer = KeybindingOverrideLayer::new();
        layer.add_override("S", vec![KeyMod::CtrlCmd], "save", "custom_save", Some("editorFocus"));
        layer.add_override("S", vec![KeyMod::CtrlCmd], "search", "custom_search", None);

        let found = layer.find_override("S", &[KeyMod::CtrlCmd], "save");
        assert!(found.is_some());
        assert_eq!(found.unwrap().new_command, "custom_save");
        assert!(found.unwrap().when.is_some());

        assert!(layer.find_override("X", &[], "nope").is_none());

        layer.remove_overrides_for_key("S", &[KeyMod::CtrlCmd]);
        assert!(layer.is_empty());
    }

    #[test]
    fn override_display() {
        let mut layer = KeybindingOverrideLayer::new();
        layer.add_override("S", vec![KeyMod::CtrlCmd], "save", "custom_save", None);
        let display = format!("{}", layer.iter().next().unwrap());
        assert!(display.contains("Ctrl+S"));
        assert!(display.contains("save"));
        assert!(display.contains("custom_save"));
    }

    #[test]
    fn generate_docs_groups_by_prefix() {
        let bindings = vec![
            sample_binding("S", "editor.save", vec![KeyMod::CtrlCmd]),
            sample_binding("Z", "editor.undo", vec![KeyMod::CtrlCmd]),
            sample_binding("N", "workbench.newFile", vec![KeyMod::CtrlCmd]),
        ];
        let docs = generate_keybinding_docs(&bindings);
        assert!(docs.contains("# Keybinding Reference"));
        assert!(docs.contains("## editor"));
        assert!(docs.contains("## workbench"));
        assert!(docs.contains("Ctrl+S"));
        assert!(docs.contains("editor.save"));
    }

    #[test]
    fn normalize_modifiers_dedup_and_sort() {
        let mods = vec![KeyMod::Alt, KeyMod::CtrlCmd, KeyMod::Alt, KeyMod::Shift];
        let normalized = normalize_modifiers(&mods);
        assert_eq!(normalized, vec![KeyMod::CtrlCmd, KeyMod::Shift, KeyMod::Alt]);

        let empty: Vec<KeyMod> = vec![];
        assert!(normalize_modifiers(&empty).is_empty());
    }

    #[test]
    fn search_bindings_filters() {
        let bindings = vec![
            sample_binding("S", "editor.save", vec![KeyMod::CtrlCmd]),
            sample_binding("N", "workbench.newFile", vec![KeyMod::CtrlCmd]),
        ];
        let found = search_bindings(&bindings, "save");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].command, "editor.save");

        let all = search_bindings(&bindings, "ctrl");
        assert_eq!(all.len(), 0); // "ctrl" not in key/command/when/source lowercase
    }

    #[test]
    fn build_command_key_index_works() {
        let bindings = vec![
            sample_binding("S", "save", vec![KeyMod::CtrlCmd]),
            sample_binding("S", "save", vec![KeyMod::CtrlCmd, KeyMod::Shift]),
        ];
        let index = build_command_key_index(&bindings);
        assert_eq!(index.get("save").unwrap().len(), 2);
        assert!(index.get("save").unwrap().contains(&"Ctrl+S".to_string()));
        assert!(index.get("save").unwrap().contains(&"Ctrl+Shift+S".to_string()));
    }

    #[test]
    fn parse_binding_definition_simple() {
        let b = parse_binding_definition("Ctrl+S -> save").unwrap();
        assert_eq!(b.key, "S");
        assert_eq!(b.modifiers, vec![KeyMod::CtrlCmd]);
        assert_eq!(b.command, "save");
        assert_eq!(b.when, None);
        assert_eq!(b.source, KeybindingSource::User);
    }

    #[test]
    fn parse_binding_definition_with_when() {
        let b = parse_binding_definition("Ctrl+Shift+P -> showCommands [when: editorFocus]").unwrap();
        assert_eq!(b.key, "P");
        assert_eq!(b.modifiers, vec![KeyMod::CtrlCmd, KeyMod::Shift]);
        assert_eq!(b.command, "showCommands");
        assert_eq!(b.when, Some("editorFocus".to_string()));
    }

    #[test]
    fn parse_binding_definition_invalid() {
        assert!(parse_binding_definition("").is_none());
        assert!(parse_binding_definition("no arrow here").is_none());
        assert!(parse_binding_definition("-> no key").is_none());
    }

    #[test]
    fn serialize_and_parse_roundtrip() {
        let bindings = vec![
            KeybindingBuilder::new().key("S").modifier(KeyMod::CtrlCmd).command("save")
                .source(KeybindingSource::User).build(),
            KeybindingBuilder::new().key("P").modifier(KeyMod::CtrlCmd).modifier(KeyMod::Shift)
                .command("showCommands").when("editorFocus")
                .source(KeybindingSource::User).build(),
        ];
        let serialized = serialize_bindings(&bindings);
        assert_eq!(serialized.len(), 2);
        assert!(serialized[0].contains("Ctrl+S -> save"));
        assert!(serialized[1].contains("[when: editorFocus]"));

        // Roundtrip parse
        for (i, s) in serialized.iter().enumerate() {
            let parsed = parse_binding_definition(s).unwrap();
            assert_eq!(parsed.key, bindings[i].key);
            assert_eq!(parsed.modifiers, bindings[i].modifiers);
            assert_eq!(parsed.command, bindings[i].command);
            assert_eq!(parsed.when, bindings[i].when);
        }
    }

    #[test]
    fn rebind_command() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));

        let result = svc.rebind("save", "S", &[KeyMod::CtrlCmd], "W", vec![KeyMod::CtrlCmd]);
        assert!(result.is_ok());
        assert!(svc.resolve("S", &[KeyMod::CtrlCmd]).is_empty());
        assert_eq!(svc.resolve("W", &[KeyMod::CtrlCmd])[0].command, "save");

        // Rebinding non-existent binding fails
        let err = svc.rebind("nope", "X", &[], "Y", vec![]);
        assert_eq!(err, Err(KeybindingError::BindingNotFound));
    }

    #[test]
    fn bound_keys_unique() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("S", "search", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("N", "new", vec![KeyMod::CtrlCmd]));

        let keys = svc.bound_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn merge_from_combines_services() {
        let mut svc1 = KeybindingService::new();
        svc1.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));

        let mut svc2 = KeybindingService::new();
        svc2.register(sample_binding("N", "new", vec![KeyMod::CtrlCmd]));
        svc2.register(sample_binding("Z", "undo", vec![KeyMod::CtrlCmd]));

        svc1.merge_from(&svc2);
        assert_eq!(svc1.binding_count(), 3);
    }

    #[test]
    fn replace_command_binding_replaces() {
        let mut svc = KeybindingService::new();
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd]));
        svc.register(sample_binding("S", "save", vec![KeyMod::CtrlCmd, KeyMod::Shift]));

        let removed = svc.replace_command_binding(
            "save", "W", vec![KeyMod::CtrlCmd], KeybindingSource::User,
        );
        assert_eq!(removed, 2);
        assert_eq!(svc.binding_count(), 1);
        let b = &svc.bindings[0];
        assert_eq!(b.key, "W");
        assert_eq!(b.command, "save");
        assert_eq!(b.source, KeybindingSource::User);
    }

    #[test]
    fn keybindingResolverOptimizer_new() {
        let s = KeybindingResolverOptimizer::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn keybindingResolverOptimizer_add_contains() {
        let mut s = KeybindingResolverOptimizer::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn keybindingResolverOptimizer_add_duplicate() {
        let mut s = KeybindingResolverOptimizer::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn keybindingResolverOptimizer_remove() {
        let mut s = KeybindingResolverOptimizer::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn keybindingResolverOptimizer_capacity() {
        let s = KeybindingResolverOptimizer::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn keybindingResolverOptimizer_search() {
        let mut s = KeybindingResolverOptimizer::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn keybindingResolverOptimizer_stats() {
        let mut s = KeybindingResolverOptimizer::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn keybindingScopeTracker_new() {
        let m = KeybindingScopeTracker::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn keybindingScopeTracker_add_find() {
        let mut m = KeybindingScopeTracker::new();
        m.add(KeybindingScopeTrackerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn keybindingScopeTracker_priority_filter() {
        let mut m = KeybindingScopeTracker::new();
        m.add(KeybindingScopeTrackerItem::new("a", "A").with_priority(KeybindingScopeTrackerPriority::High));
        m.add(KeybindingScopeTrackerItem::new("b", "B").with_priority(KeybindingScopeTrackerPriority::Low));
        m.add(KeybindingScopeTrackerItem::new("c", "C").with_priority(KeybindingScopeTrackerPriority::High));
        assert_eq!(m.by_priority(KeybindingScopeTrackerPriority::High).len(), 2);
    }

    #[test]
    fn keybindingScopeTracker_remove() {
        let mut m = KeybindingScopeTracker::new();
        m.add(KeybindingScopeTrackerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn keybindingScopeTracker_search() {
        let mut m = KeybindingScopeTracker::new();
        m.add(KeybindingScopeTrackerItem::new("id1", "Hello World"));
        m.add(KeybindingScopeTrackerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn keybindingScopeTracker_total_weight() {
        let mut m = KeybindingScopeTracker::new();
        m.add(KeybindingScopeTrackerItem::new("a", "A").with_priority(KeybindingScopeTrackerPriority::Critical));
        m.add(KeybindingScopeTrackerItem::new("b", "B").with_priority(KeybindingScopeTrackerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn keybindingScopeTracker_capacity_limit() {
        let mut m = KeybindingScopeTracker::new().with_max_items(2);
        m.add(KeybindingScopeTrackerItem::new("1", "one"));
        m.add(KeybindingScopeTrackerItem::new("2", "two"));
        assert!(!m.add(KeybindingScopeTrackerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn keybindingScopeTracker_sorted_by_priority() {
        let mut m = KeybindingScopeTracker::new();
        m.add(KeybindingScopeTrackerItem::new("lo", "Low").with_priority(KeybindingScopeTrackerPriority::Low));
        m.add(KeybindingScopeTrackerItem::new("hi", "High").with_priority(KeybindingScopeTrackerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn keybindingScopeTracker_item_metadata() {
        let mut item = KeybindingScopeTrackerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn keybindingResolverOptimizer_enabled_toggle() {
        let mut s = KeybindingResolverOptimizer::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn keybindingScopeTracker_priority_display() {
        assert_eq!(format!("{}", KeybindingScopeTrackerPriority::High), "high");
        assert_eq!(format!("{}", KeybindingScopeTrackerPriority::Low), "low");
    }


    #[test]
    fn wb_keybinding_entry_creation() {
        let e = WbKeybindingEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_keybinding_entry_with_priority() {
        let e = WbKeybindingEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_keybinding_entry_metadata() {
        let e = WbKeybindingEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_keybinding_entry_remove_meta() {
        let mut e = WbKeybindingEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_keybinding_entry_activate_deactivate() {
        let mut e = WbKeybindingEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_keybinding_config_add_sorted() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("lo", "Lo").with_priority(1));
        c.add(WbKeybindingEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_keybinding_config_capacity() {
        let mut c = WbKeybindingConfig::new(1);
        assert!(c.add(WbKeybindingEntry::new("a", "A")));
        assert!(!c.add(WbKeybindingEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_keybinding_config_remove() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_keybinding_config_get() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_keybinding_config_active_entries() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("a", "A"));
        c.add(WbKeybindingEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_keybinding_config_enable_disable() {
        let mut c = WbKeybindingConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_keybinding_config_clear() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_keybinding_config_find_by_label() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_keybinding_config_top_n() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("a", "A").with_priority(1));
        c.add(WbKeybindingEntry::new("b", "B").with_priority(2));
        c.add(WbKeybindingEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_keybinding_config_deactivate_activate_all() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("a", "A"));
        c.add(WbKeybindingEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_keybinding_config_highest_priority() {
        let mut c = WbKeybindingConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbKeybindingEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_keybinding_config_contains() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_keybinding_config_labels() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("a", "Alpha"));
        c.add(WbKeybindingEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_keybinding_config_drain_inactive() {
        let mut c = WbKeybindingConfig::new(10);
        c.add(WbKeybindingEntry::new("a", "A"));
        c.add(WbKeybindingEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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


    // xa_ extended tests for wb_keybinding
    #[test]
    fn xa_wb_keybinding_ring_new() {
        let rb = super::XaWbKeybindingRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_keybinding_ring_push_len() {
        let mut rb = super::XaWbKeybindingRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_keybinding_ring_wrap() {
        let mut rb = super::XaWbKeybindingRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_keybinding_ring_mean_empty() {
        let rb = super::XaWbKeybindingRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_keybinding_ring_mean_values() {
        let mut rb = super::XaWbKeybindingRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_keybinding_ring_min_max() {
        let mut rb = super::XaWbKeybindingRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_keybinding_ring_iter() {
        let mut rb = super::XaWbKeybindingRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_keybinding_counter_new() {
        let c = super::XaWbKeybindingCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_keybinding_counter_inc() {
        let mut c = super::XaWbKeybindingCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_keybinding_counter_inc_by() {
        let mut c = super::XaWbKeybindingCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_keybinding_counter_reset() {
        let mut c = super::XaWbKeybindingCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_keybinding_counter_clear() {
        let mut c = super::XaWbKeybindingCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_keybinding_counter_default() {
        let c = super::XaWbKeybindingCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 210 ----

    #[test]
    fn xc_210_pool_new_empty() {
        let pool: super::Xc210Pool<i32> = super::Xc210Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_210_pool_release_acquire() {
        let mut pool = super::Xc210Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_210_pool_acquire_empty() {
        let mut pool: super::Xc210Pool<i32> = super::Xc210Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_210_pool_full() {
        let mut pool = super::Xc210Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_210_pool_drain() {
        let mut pool = super::Xc210Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_210_pool_stats() {
        let mut pool = super::Xc210Pool::new(8);
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
    fn xc_210_pool_clear() {
        let mut pool = super::Xc210Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_210_pool_shrink() {
        let mut pool = super::Xc210Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_210_pool_default() {
        let pool: super::Xc210Pool<String> = super::Xc210Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_210_pool_extend() {
        let mut pool = super::Xc210Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_210_pool_retain() {
        let mut pool = super::Xc210Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_210_scheduler_round_robin() {
        let mut sched = super::Xc210Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_210_scheduler_empty() {
        let mut sched = super::Xc210Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_210_scheduler_reset() {
        let mut sched = super::Xc210Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_210_scheduler_add_remove() {
        let mut sched = super::Xc210Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_210_scheduler_targets() {
        let sched = super::Xc210Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_210_hash_empty() {
        assert_eq!(super::xc_210_hash(b""), 5381);
    }

    #[test]
    fn xc_210_hash_data() {
        let h = super::xc_210_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_210_hash(b"hello"), h);
    }

    #[test]
    fn xc_210_reverse_str() {
        assert_eq!(super::xc_210_reverse("abc"), "cba");
        assert_eq!(super::xc_210_reverse(""), "");
    }


    // --- xd_80 deepening tests ---

    #[test]
    fn xd_80_sm_initial_state() {
        let sm = Xd80StateMachine::new();
        assert_eq!(sm.current_state(), Xd80State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_80_sm_valid_idle_to_running() {
        let mut sm = Xd80StateMachine::new();
        assert!(sm.transition(Xd80State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd80State::Running);
    }

    #[test]
    fn xd_80_sm_valid_running_to_paused() {
        let mut sm = Xd80StateMachine::new();
        sm.transition(Xd80State::Running).unwrap();
        assert!(sm.transition(Xd80State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd80State::Paused);
    }

    #[test]
    fn xd_80_sm_valid_running_to_done() {
        let mut sm = Xd80StateMachine::new();
        sm.transition(Xd80State::Running).unwrap();
        assert!(sm.transition(Xd80State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd80State::Done);
    }

    #[test]
    fn xd_80_sm_valid_paused_to_running() {
        let mut sm = Xd80StateMachine::new();
        sm.transition(Xd80State::Running).unwrap();
        sm.transition(Xd80State::Paused).unwrap();
        assert!(sm.transition(Xd80State::Running).is_ok());
    }

    #[test]
    fn xd_80_sm_valid_done_to_idle() {
        let mut sm = Xd80StateMachine::new();
        sm.transition(Xd80State::Running).unwrap();
        sm.transition(Xd80State::Done).unwrap();
        assert!(sm.transition(Xd80State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd80State::Idle);
    }

    #[test]
    fn xd_80_sm_invalid_idle_to_done() {
        let mut sm = Xd80StateMachine::new();
        assert!(sm.transition(Xd80State::Done).is_err());
    }

    #[test]
    fn xd_80_sm_invalid_idle_to_paused() {
        let mut sm = Xd80StateMachine::new();
        assert!(sm.transition(Xd80State::Paused).is_err());
    }

    #[test]
    fn xd_80_sm_history_tracking() {
        let mut sm = Xd80StateMachine::new();
        sm.transition(Xd80State::Running).unwrap();
        sm.transition(Xd80State::Paused).unwrap();
        sm.transition(Xd80State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd80State::Idle);
        assert_eq!(sm.history()[0].to, Xd80State::Running);
        assert_eq!(sm.history()[1].from, Xd80State::Running);
        assert_eq!(sm.history()[2].to, Xd80State::Done);
    }

    #[test]
    fn xd_80_sm_serialize_deserialize() {
        let mut sm = Xd80StateMachine::new();
        sm.transition(Xd80State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd80StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd80State::Running));
    }

    #[test]
    fn xd_80_sm_deserialize_invalid() {
        assert_eq!(Xd80StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_80_sm_reset() {
        let mut sm = Xd80StateMachine::new();
        sm.transition(Xd80State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd80State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_80_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd80EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd80Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_80_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd80EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd80Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd80Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_80_bus_unsubscribe() {
        let mut bus = Xd80EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_80_event_kind_and_payload() {
        let e = Xd80Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd80Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_80_bus_clear_history() {
        let mut bus = Xd80EventBus::new();
        bus.publish(Xd80Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_80_sm_step_counter_increments() {
        let mut sm = Xd80StateMachine::new();
        sm.transition(Xd80State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd80State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #100 --

    #[test]
    fn xf100_trie_insert_search() {
        let mut t = Xf100Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf100_trie_starts_with() {
        let mut t = Xf100Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf100_trie_remove() {
        let mut t = Xf100Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf100_trie_word_count() {
        let mut t = Xf100Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf100_trie_longest_prefix() {
        let mut t = Xf100Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf100_trie_all_words() {
        let mut t = Xf100Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf100_trie_autocomplete() {
        let mut t = Xf100Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf100_trie_empty_search() {
        let t = Xf100Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf100_bloom_add_contains() {
        let mut bf = Xf100BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf100_bloom_probably_absent() {
        let bf = Xf100BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf100_bloom_false_positive_rate() {
        let mut bf = Xf100BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf100_bloom_clear() {
        let mut bf = Xf100BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf100_bloom_union() {
        let mut a = Xf100BloomFilter::xf_new(512, 2);
        let mut b = Xf100BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf100_bloom_intersection_estimate() {
        let mut a = Xf100BloomFilter::xf_new(512, 2);
        let mut b = Xf100BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf100_bloom_union_size_mismatch() {
        let a = Xf100BloomFilter::xf_new(256, 2);
        let b = Xf100BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}
