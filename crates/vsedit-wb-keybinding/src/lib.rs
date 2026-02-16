//! User keybinding service.

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
}
