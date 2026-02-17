//! Keybinding resolution service.
//!
//! Resolves key presses to commands based on registered keybinding rules and
//! context key evaluation. Equivalent to VS Code's keybinding resolver.
//!
//! # Features
//!
//! - **Chord sequences** with configurable timeout (`Ctrl+K Ctrl+C`)
//! - **When-clause evaluation** via [`vsedit_contextkey::ContextKeyExpr`]
//! - **Source-aware conflict resolution** (User > Extension > Default)
//! - **keybindings.json loading** with removal (`-command`) support
//! - **50+ default keybindings** matching VS Code

use std::fmt;

use vsedit_contextkey::{ContextKeyExpr, IContext};
use vsedit_keybindings::{
    keybinding_matches, parse_keybinding, serialize_keybinding, Keybinding,
    ResolvedKeybinding, SimpleResolvedKeybinding,
};
use vsedit_keycodes::{KeyCode, KeyCodeChord};
use vsedit_platform::Platform;

// ---------------------------------------------------------------------------
// KeybindingWeight
// ---------------------------------------------------------------------------

/// Precedence weight for keybinding rules. Higher weight wins when multiple
/// rules match the same chord sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u32)]
pub enum KeybindingWeight {
    EditorCore = 0,
    EditorContrib = 100,
    WorkbenchContrib = 200,
    BuiltinExtension = 300,
    ExternalExtension = 400,
}

// ---------------------------------------------------------------------------
// KeybindingSource
// ---------------------------------------------------------------------------

/// Origin of a keybinding rule, used for conflict resolution.
///
/// Priority order: User > Extension > Default.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeybindingSource {
    /// Built-in default keybinding.
    Default,
    /// Keybinding contributed by an extension.
    Extension(String),
    /// User-defined keybinding (from keybindings.json).
    User,
}

impl KeybindingSource {
    /// Numeric priority for conflict resolution (higher wins).
    fn priority(&self) -> u32 {
        match self {
            Self::Default => 0,
            Self::Extension(_) => 1,
            Self::User => 2,
        }
    }
}

// ---------------------------------------------------------------------------
// KeybindingRule
// ---------------------------------------------------------------------------

/// A registered keybinding that maps a chord sequence to a command.
#[derive(Debug, Clone)]
pub struct KeybindingRule {
    pub keybinding: Keybinding,
    pub command: String,
    pub args: Option<Vec<String>>,
    pub when: Option<ContextKeyExpr>,
    pub weight: KeybindingWeight,
    pub source: KeybindingSource,
}

// ---------------------------------------------------------------------------
// ResolveResult
// ---------------------------------------------------------------------------

/// The outcome of resolving a chord sequence against the keybinding rules.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveResult {
    /// No rule matches the pressed chords.
    NoMatch,
    /// The pressed chords are a prefix of a multi-chord binding; waiting for
    /// the next chord.
    MoreChordsNeeded,
    /// A matching rule was found.
    CommandMatch {
        command: String,
        args: Option<Vec<String>>,
    },
}

// ---------------------------------------------------------------------------
// ChordState — tracks multi-chord input state
// ---------------------------------------------------------------------------

/// Tracks the state of a multi-chord keybinding sequence.
#[derive(Debug, Clone, PartialEq)]
pub enum ChordState {
    /// No chord sequence in progress.
    None,
    /// First chord pressed, waiting for second chord.
    FirstChord {
        chord: KeyCodeChord,
        /// Timestamp (milliseconds since epoch or monotonic) when the first chord was pressed.
        timestamp_ms: u64,
    },
}

impl ChordState {
    /// Default timeout in milliseconds for chord sequences.
    pub const DEFAULT_TIMEOUT_MS: u64 = 1000;
}

// ---------------------------------------------------------------------------
// KeybindingMatch — result of resolve_key
// ---------------------------------------------------------------------------

/// Result of resolving a single key press through the chord state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum KeybindingMatch {
    /// No keybinding matches.
    NoMatch,
    /// First chord of a multi-chord binding; waiting for second chord.
    PartialMatch,
    /// A keybinding fully matched.
    ExactMatch {
        command: String,
        args: Option<Vec<String>>,
    },
}

// ---------------------------------------------------------------------------
// KeybindingResolver
// ---------------------------------------------------------------------------

/// Resolves key chord sequences to commands.
///
/// Rules are checked in registration order. When multiple rules match, the
/// one with the highest source priority wins, then weight, then last-registered.
///
/// A rule whose `command` starts with `-` acts as a *removal*: it
/// suppresses any earlier rule whose command (without the `-` prefix)
/// matches.
pub struct KeybindingResolver {
    rules: Vec<KeybindingRule>,
    chord_state: ChordState,
    chord_timeout_ms: u64,
}

impl KeybindingResolver {
    /// Create an empty resolver.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            chord_state: ChordState::None,
            chord_timeout_ms: ChordState::DEFAULT_TIMEOUT_MS,
        }
    }

    /// Set the chord timeout in milliseconds.
    pub fn set_chord_timeout(&mut self, timeout_ms: u64) {
        self.chord_timeout_ms = timeout_ms;
    }

    /// Register a keybinding rule.
    pub fn add_rule(&mut self, rule: KeybindingRule) {
        self.rules.push(rule);
    }

    /// Return all registered rules.
    pub fn rules(&self) -> &[KeybindingRule] {
        &self.rules
    }

    /// Get the current chord state.
    pub fn chord_state(&self) -> &ChordState {
        &self.chord_state
    }

    /// Reset the chord state to None.
    pub fn reset_chord_state(&mut self) {
        self.chord_state = ChordState::None;
    }

    /// Resolve a single key press, managing chord state and timeout.
    ///
    /// `now_ms` is the current timestamp in milliseconds (monotonic or epoch).
    pub fn resolve_key(
        &mut self,
        context: &dyn IContext,
        chord: KeyCodeChord,
        now_ms: u64,
    ) -> KeybindingMatch {
        match &self.chord_state {
            ChordState::None => {
                // Try as first chord of a multi-chord binding.
                let pressed = [chord];
                let result = self.resolve(context, &pressed);
                match result {
                    ResolveResult::MoreChordsNeeded => {
                        self.chord_state = ChordState::FirstChord {
                            chord,
                            timestamp_ms: now_ms,
                        };
                        KeybindingMatch::PartialMatch
                    }
                    ResolveResult::CommandMatch { command, args } => {
                        KeybindingMatch::ExactMatch { command, args }
                    }
                    ResolveResult::NoMatch => KeybindingMatch::NoMatch,
                }
            }
            ChordState::FirstChord {
                chord: first_chord,
                timestamp_ms,
            } => {
                let elapsed = now_ms.saturating_sub(*timestamp_ms);
                if elapsed > self.chord_timeout_ms {
                    // Timeout: reset and treat this as a fresh key press.
                    self.chord_state = ChordState::None;
                    return self.resolve_key(context, chord, now_ms);
                }

                // Try completing the two-chord sequence.
                let first = *first_chord;
                self.chord_state = ChordState::None;
                let pressed = [first, chord];
                let result = self.resolve(context, &pressed);
                match result {
                    ResolveResult::CommandMatch { command, args } => {
                        KeybindingMatch::ExactMatch { command, args }
                    }
                    _ => {
                        // Two-chord sequence didn't match; try the second chord alone.
                        let single = [chord];
                        let fallback = self.resolve(context, &single);
                        match fallback {
                            ResolveResult::MoreChordsNeeded => {
                                self.chord_state = ChordState::FirstChord {
                                    chord,
                                    timestamp_ms: now_ms,
                                };
                                KeybindingMatch::PartialMatch
                            }
                            ResolveResult::CommandMatch { command, args } => {
                                KeybindingMatch::ExactMatch { command, args }
                            }
                            ResolveResult::NoMatch => KeybindingMatch::NoMatch,
                        }
                    }
                }
            }
        }
    }

    /// Resolve pressed chords against registered rules and context.
    pub fn resolve(
        &self,
        context: &dyn IContext,
        pressed_chords: &[KeyCodeChord],
    ) -> ResolveResult {
        if pressed_chords.is_empty() {
            return ResolveResult::NoMatch;
        }

        let mut best_match: Option<&KeybindingRule> = None;
        let mut has_prefix_match = false;

        // Collect negated commands so we can suppress them.
        let negated: Vec<&str> = self
            .rules
            .iter()
            .filter(|r| r.command.starts_with('-'))
            .filter(|r| chords_fully_match(&r.keybinding, pressed_chords))
            .filter(|r| when_satisfied(r, context))
            .map(|r| &r.command[1..])
            .collect();

        for rule in &self.rules {
            // Skip negation rules themselves.
            if rule.command.starts_with('-') {
                continue;
            }

            // Skip rules suppressed by a negation.
            if negated.contains(&rule.command.as_str()) {
                continue;
            }

            let binding_len = rule.keybinding.parts.len();
            let pressed_len = pressed_chords.len();

            // Check if all pressed chords match the beginning of this binding.
            let prefix_ok = pressed_chords
                .iter()
                .enumerate()
                .all(|(i, chord)| keybinding_matches(&rule.keybinding, chord, i));

            if !prefix_ok {
                continue;
            }

            if pressed_len < binding_len {
                // Partial match — more chords needed.
                has_prefix_match = true;
            } else if pressed_len == binding_len {
                // Full match — evaluate when clause.
                if !when_satisfied(rule, context) {
                    continue;
                }
                best_match = Some(match best_match {
                    None => rule,
                    Some(prev) => pick_best_rule(prev, rule),
                });
            }
            // pressed_len > binding_len: impossible for a correct match.
        }

        if let Some(rule) = best_match {
            return ResolveResult::CommandMatch {
                command: rule.command.clone(),
                args: rule.args.clone(),
            };
        }

        if has_prefix_match {
            return ResolveResult::MoreChordsNeeded;
        }

        ResolveResult::NoMatch
    }

    /// Return all rules that map to the given command.
    pub fn get_keybindings_for_command(&self, command: &str) -> Vec<&KeybindingRule> {
        self.rules
            .iter()
            .filter(|r| r.command == command)
            .collect()
    }
}

/// Pick the winning rule when two rules both match.
///
/// Priority: source priority > weight > registration order (last wins).
fn pick_best_rule<'a>(prev: &'a KeybindingRule, candidate: &'a KeybindingRule) -> &'a KeybindingRule {
    let prev_src = prev.source.priority();
    let cand_src = candidate.source.priority();
    if cand_src > prev_src {
        return candidate;
    }
    if cand_src < prev_src {
        return prev;
    }
    // Same source priority — compare weight, then last-registered wins.
    if candidate.weight >= prev.weight {
        candidate
    } else {
        prev
    }
}

impl Default for KeybindingResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// KeybindingRule — additional methods
// ---------------------------------------------------------------------------

impl KeybindingRule {
    /// Returns `true` if this rule is a removal/negation rule (command starts
    /// with `-`).
    pub fn is_removal(&self) -> bool {
        self.command.starts_with('-')
    }

    /// For removal rules, returns the command name being removed (without the
    /// leading `-`). Returns `None` for normal rules.
    pub fn removal_target(&self) -> Option<&str> {
        if self.is_removal() {
            Some(&self.command[1..])
        } else {
            None
        }
    }

    /// Returns true if this rule has a when-clause guard.
    pub fn is_conditional(&self) -> bool {
        self.when.is_some()
    }

    /// Returns the number of chords in the keybinding (1 for single-chord,
    /// 2 for two-chord sequences like Ctrl+K Ctrl+C).
    pub fn chord_count(&self) -> usize {
        self.keybinding.parts.len()
    }

    /// Returns a human-readable label for this rule's keybinding on the given
    /// platform.
    pub fn label(&self, platform: Platform) -> String {
        let resolved = SimpleResolvedKeybinding::new(
            self.keybinding.clone(),
            platform,
        );
        resolved.get_label()
    }

    /// Serialize the keybinding to a canonical dispatch string
    /// (e.g. `"ctrl+k ctrl+c"`).
    pub fn serialize_key(&self) -> String {
        serialize_keybinding(&self.keybinding)
    }
}

// ---------------------------------------------------------------------------
// KeybindingSource — additional methods
// ---------------------------------------------------------------------------

impl KeybindingSource {
    /// Returns the extension ID if this source is an extension, `None`
    /// otherwise.
    pub fn extension_id(&self) -> Option<&str> {
        match self {
            Self::Extension(id) => Some(id),
            _ => None,
        }
    }

    /// Returns `true` when this source represents a user-defined keybinding.
    pub fn is_user(&self) -> bool {
        matches!(self, Self::User)
    }

    /// Returns a display string for UI purposes.
    pub fn display_name(&self) -> String {
        match self {
            Self::Default => "Default".to_string(),
            Self::Extension(id) => format!("Extension ({id})"),
            Self::User => "User".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// ResolveResult — additional methods
// ---------------------------------------------------------------------------

impl ResolveResult {
    /// Returns `true` when the result is a successful command match.
    pub fn is_match(&self) -> bool {
        matches!(self, Self::CommandMatch { .. })
    }

    /// Extract the matched command name, if any.
    pub fn command(&self) -> Option<&str> {
        match self {
            Self::CommandMatch { command, .. } => Some(command),
            _ => None,
        }
    }

    /// Extract the matched command args, if any.
    pub fn args(&self) -> Option<&[String]> {
        match self {
            Self::CommandMatch { args: Some(a), .. } => Some(a),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// KeybindingResolver — additional query / bulk methods
// ---------------------------------------------------------------------------

impl KeybindingResolver {
    /// Return the number of registered rules (including removals).
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Remove all rules whose command matches the given string exactly.
    pub fn remove_rules_for_command(&mut self, command: &str) {
        self.rules.retain(|r| r.command != command);
    }

    /// Remove all rules originating from a given extension ID.
    pub fn remove_rules_from_extension(&mut self, ext_id: &str) {
        self.rules.retain(|r| match &r.source {
            KeybindingSource::Extension(id) => id != ext_id,
            _ => true,
        });
    }

    /// Return all unique command IDs (excluding removal rules).
    pub fn command_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .rules
            .iter()
            .filter(|r| !r.is_removal())
            .map(|r| r.command.clone())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Bulk-add rules from a JSON keybindings string.  Convenience wrapper
    /// around [`load_keybindings_json`].
    pub fn load_json(&mut self, json: &str, platform: Platform) -> Result<usize, String> {
        let rules = load_keybindings_json(json, platform)?;
        let count = rules.len();
        for rule in rules {
            self.add_rule(rule);
        }
        Ok(count)
    }

    /// Returns true if any registered rule maps to the given command.
    pub fn has_command(&self, command: &str) -> bool {
        self.rules.iter().any(|r| r.command == command)
    }

    /// Returns all rules contributed by the given extension.
    pub fn rules_from_extension(&self, ext_id: &str) -> Vec<&KeybindingRule> {
        self.rules
            .iter()
            .filter(|r| r.source.extension_id() == Some(ext_id))
            .collect()
    }
}

/// Check if a rule's `when` clause is satisfied (or absent).
fn when_satisfied(rule: &KeybindingRule, context: &dyn IContext) -> bool {
    match &rule.when {
        None => true,
        Some(expr) => expr.evaluate(context),
    }
}

/// Count the specificity of a when-clause (number of terms).
fn when_specificity(rule: &KeybindingRule) -> usize {
    match &rule.when {
        None => 0,
        Some(expr) => count_terms(expr),
    }
}

fn count_terms(expr: &ContextKeyExpr) -> usize {
    match expr {
        ContextKeyExpr::And(v) => v.iter().map(count_terms).sum(),
        ContextKeyExpr::Or(v) => v.iter().map(count_terms).sum(),
        ContextKeyExpr::Not(inner) => count_terms(inner),
        ContextKeyExpr::True | ContextKeyExpr::False => 0,
        _ => 1,
    }
}

/// Check if all parts of a keybinding exactly match the pressed chords.
fn chords_fully_match(binding: &Keybinding, pressed: &[KeyCodeChord]) -> bool {
    binding.parts.len() == pressed.len()
        && pressed
            .iter()
            .enumerate()
            .all(|(i, chord)| keybinding_matches(binding, chord, i))
}

// ---------------------------------------------------------------------------
// IKeybindingService
// ---------------------------------------------------------------------------

/// Service trait for keybinding resolution, suitable for DI registration.
pub trait IKeybindingService: Send + Sync {
    /// Resolve pressed chords to a command.
    fn resolve(
        &self,
        context: &dyn IContext,
        chords: &[KeyCodeChord],
    ) -> ResolveResult;

    /// Register a keybinding rule.
    fn add_rule(&self, rule: KeybindingRule);

    /// Return all rules that map to the given command.
    fn get_keybindings_for_command(&self, command: &str) -> Vec<KeybindingRule>;
}

// ---------------------------------------------------------------------------
// keybindings.json loading
// ---------------------------------------------------------------------------

/// A single entry from a VS Code `keybindings.json` file.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct KeybindingJsonEntry {
    pub key: String,
    pub command: String,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
}

/// Load keybinding rules from a JSON string in VS Code `keybindings.json` format.
///
/// Each entry is `{ "key": "ctrl+s", "command": "...", "when": "..." }`.
/// A command prefixed with `-` is a removal rule.
pub fn load_keybindings_json(
    json: &str,
    platform: Platform,
) -> Result<Vec<KeybindingRule>, String> {
    let entries: Vec<KeybindingJsonEntry> =
        serde_json::from_str(json).map_err(|e| format!("invalid keybindings JSON: {e}"))?;

    let mut rules = Vec::with_capacity(entries.len());
    for entry in entries {
        let keybinding = parse_keybinding(&entry.key, platform)
            .ok_or_else(|| format!("cannot parse key: {:?}", entry.key))?;

        let when = match &entry.when {
            Some(w) if !w.is_empty() => {
                Some(ContextKeyExpr::parse(w).map_err(|e| format!("bad when-clause: {e}"))?)
            }
            _ => None,
        };

        rules.push(KeybindingRule {
            keybinding,
            command: entry.command,
            args: entry.args,
            when,
            weight: KeybindingWeight::ExternalExtension,
            source: KeybindingSource::User,
        });
    }

    Ok(rules)
}

// ---------------------------------------------------------------------------
// Default keybindings
// ---------------------------------------------------------------------------

/// Helper to build a default keybinding rule with an optional when-clause.
fn default_rule(
    chords: &[KeyCodeChord],
    command: &str,
    when: Option<&str>,
) -> KeybindingRule {
    KeybindingRule {
        keybinding: Keybinding {
            parts: chords.to_vec(),
        },
        command: command.to_string(),
        args: None,
        when: when.and_then(|w| ContextKeyExpr::parse(w).ok()),
        weight: KeybindingWeight::EditorCore,
        source: KeybindingSource::Default,
    }
}

/// Register the core set of default editor keybindings on a resolver.
///
/// Registers 50+ keybindings matching VS Code's default set.
pub fn register_default_keybindings(resolver: &mut KeybindingResolver) {
    use KeyCode::*;

    let ctrl = |kc: KeyCode| KeyCodeChord::new(true, false, false, false, kc);
    let ctrl_shift = |kc: KeyCode| KeyCodeChord::new(true, true, false, false, kc);
    let alt = |kc: KeyCode| KeyCodeChord::new(false, false, true, false, kc);
    let alt_shift = |kc: KeyCode| KeyCodeChord::new(false, true, true, false, kc);
    let just = |kc: KeyCode| KeyCodeChord::just(kc);

    let defaults: Vec<KeybindingRule> = vec![
        // ── Clipboard ──
        default_rule(&[ctrl(KeyC)], "editor.action.clipboardCopyAction", None),
        default_rule(&[ctrl(KeyV)], "editor.action.clipboardPasteAction", None),
        default_rule(&[ctrl(KeyX)], "editor.action.clipboardCutAction", None),
        // ── Undo / Redo ──
        default_rule(&[ctrl(KeyZ)], "undo", None),
        default_rule(&[ctrl_shift(KeyZ)], "redo", None),
        default_rule(&[ctrl(KeyY)], "redo", None),
        // ── File ──
        default_rule(&[ctrl(KeyS)], "workbench.action.files.save", None),
        default_rule(&[ctrl_shift(KeyS)], "workbench.action.files.saveAs", None),
        default_rule(&[ctrl(KeyN)], "workbench.action.files.newUntitledFile", None),
        default_rule(&[ctrl(KeyO)], "workbench.action.files.openFile", None),
        // ── Quick open / command palette ──
        default_rule(&[ctrl(KeyP)], "workbench.action.quickOpen", None),
        default_rule(&[ctrl_shift(KeyP)], "workbench.action.showCommands", None),
        default_rule(&[just(F1)], "workbench.action.showCommands", None),
        // ── Find / Replace ──
        default_rule(&[ctrl(KeyF)], "actions.find", Some("editorFocus")),
        default_rule(&[ctrl(KeyH)], "editor.action.startFindReplaceAction", Some("editorFocus")),
        default_rule(&[just(F3)], "editor.action.nextMatchFindAction", Some("editorFocus")),
        default_rule(&[KeyCodeChord::new(false, true, false, false, F3)], "editor.action.previousMatchFindAction", Some("editorFocus")),
        // ── Go to line ──
        default_rule(&[ctrl(KeyG)], "workbench.action.gotoLine", None),
        // ── Terminal ──
        default_rule(&[ctrl(Backquote)], "workbench.action.terminal.toggleTerminal", None),
        // ── Sidebar ──
        default_rule(&[ctrl(KeyB)], "workbench.action.toggleSidebarVisibility", None),
        // ── Editor tabs ──
        default_rule(&[ctrl(KeyW)], "workbench.action.closeActiveEditor", None),
        default_rule(&[ctrl(Tab)], "workbench.action.nextEditor", None),
        default_rule(&[ctrl_shift(Tab)], "workbench.action.previousEditor", None),
        // ── Comment ──
        default_rule(&[ctrl(Slash)], "editor.action.commentLine", Some("editorTextFocus")),
        default_rule(
            &[ctrl(KeyK), ctrl(KeyC)],
            "editor.action.addCommentLine",
            Some("editorTextFocus"),
        ),
        default_rule(
            &[ctrl(KeyK), ctrl(KeyU)],
            "editor.action.removeCommentLine",
            Some("editorTextFocus"),
        ),
        // ── Indentation ──
        default_rule(&[ctrl(BracketRight)], "editor.action.indentLines", Some("editorTextFocus")),
        default_rule(&[ctrl(BracketLeft)], "editor.action.outdentLines", Some("editorTextFocus")),
        // ── Line operations ──
        default_rule(&[alt(UpArrow)], "editor.action.moveLinesUpAction", Some("editorTextFocus")),
        default_rule(&[alt(DownArrow)], "editor.action.moveLinesDownAction", Some("editorTextFocus")),
        default_rule(&[alt_shift(UpArrow)], "editor.action.copyLinesUpAction", Some("editorTextFocus")),
        default_rule(&[alt_shift(DownArrow)], "editor.action.copyLinesDownAction", Some("editorTextFocus")),
        default_rule(&[ctrl_shift(KeyK)], "editor.action.deleteLines", Some("editorTextFocus")),
        default_rule(&[ctrl(Enter)], "editor.action.insertLineAfter", Some("editorTextFocus")),
        default_rule(&[ctrl_shift(Enter)], "editor.action.insertLineBefore", Some("editorTextFocus")),
        // ── Multi-cursor / selection ──
        default_rule(&[ctrl(KeyD)], "editor.action.addSelectionToNextFindMatch", Some("editorFocus")),
        default_rule(&[ctrl_shift(KeyL)], "editor.action.selectHighlights", Some("editorFocus")),
        default_rule(&[ctrl(KeyA)], "editor.action.selectAll", None),
        default_rule(&[ctrl(KeyL)], "expandLineSelection", Some("editorTextFocus")),
        // ── Navigation ──
        default_rule(&[just(F12)], "editor.action.revealDefinition", Some("editorHasDefinitionProvider && editorTextFocus")),
        default_rule(&[alt(F12)], "editor.action.peekDefinition", Some("editorHasDefinitionProvider && editorTextFocus")),
        default_rule(&[KeyCodeChord::new(false, true, false, false, F12)], "editor.action.goToReferences", Some("editorHasReferenceProvider && editorTextFocus")),
        default_rule(&[just(F2)], "editor.action.rename", Some("editorHasRenameProvider && editorTextFocus")),
        default_rule(&[ctrl_shift(KeyO)], "workbench.action.gotoSymbol", None),
        default_rule(&[ctrl(KeyT)], "workbench.action.showAllSymbols", None),
        // ── View / Layout ──
        default_rule(&[ctrl_shift(KeyE)], "workbench.view.explorer", None),
        default_rule(&[ctrl_shift(KeyF)], "workbench.view.search", None),
        default_rule(&[ctrl_shift(KeyG)], "workbench.view.scm", None),
        default_rule(&[ctrl_shift(KeyD)], "workbench.view.debug", None),
        default_rule(&[ctrl_shift(KeyX)], "workbench.view.extensions", None),
        // ── Zoom ──
        default_rule(&[ctrl(Equal)], "workbench.action.zoomIn", None),
        default_rule(&[ctrl(Minus)], "workbench.action.zoomOut", None),
        default_rule(&[ctrl(Digit0)], "workbench.action.zoomReset", None),
        // ── Debug ──
        default_rule(&[just(F5)], "workbench.action.debug.start", Some("!inDebugMode")),
        default_rule(&[just(F5)], "workbench.action.debug.continue", Some("inDebugMode")),
        default_rule(&[KeyCodeChord::new(false, true, false, false, F5)], "workbench.action.debug.stop", Some("inDebugMode")),
        default_rule(&[just(F9)], "editor.debug.action.toggleBreakpoint", Some("editorTextFocus")),
        default_rule(&[just(F10)], "workbench.action.debug.stepOver", Some("inDebugMode")),
        default_rule(&[just(F11)], "workbench.action.debug.stepInto", Some("inDebugMode")),
        default_rule(&[KeyCodeChord::new(false, true, false, false, F11)], "workbench.action.debug.stepOut", Some("inDebugMode")),
        // ── Format ──
        default_rule(&[ctrl_shift(KeyI)], "editor.action.formatDocument", Some("editorTextFocus && editorHasDocumentFormattingProvider")),
        // ── Fold ──
        default_rule(
            &[ctrl(KeyK), ctrl(Digit0)],
            "editor.unfoldAll",
            Some("editorTextFocus"),
        ),
        default_rule(
            &[ctrl(KeyK), ctrl(KeyJ)],
            "editor.unfoldAll",
            Some("editorTextFocus"),
        ),
        // Ctrl+K Ctrl+W → Close all editors
        default_rule(
            &[ctrl(KeyK), ctrl(KeyW)],
            "workbench.action.closeAllEditors",
            None,
        ),
    ];

    for rule in defaults {
        resolver.add_rule(rule);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// KeybindingExporter – serialise bindings to JSON format
// ---------------------------------------------------------------------------

/// Exports keybinding configurations to a structured JSON-like format.
#[derive(Debug, Clone)]
pub struct KeybindingExporter {
    pretty: bool,
    include_defaults: bool,
    include_extensions: bool,
}

impl KeybindingExporter {
    pub fn new() -> Self {
        Self {
            pretty: true,
            include_defaults: false,
            include_extensions: true,
        }
    }

    pub fn pretty(mut self, yes: bool) -> Self {
        self.pretty = yes;
        self
    }

    pub fn include_defaults(mut self, yes: bool) -> Self {
        self.include_defaults = yes;
        self
    }

    pub fn include_extensions(mut self, yes: bool) -> Self {
        self.include_extensions = yes;
        self
    }

    /// Export rules from a resolver to a JSON string.
    pub fn export(&self, resolver: &KeybindingResolver) -> String {
        let mut entries = Vec::new();
        for rule in resolver.rules() {
            let dominated = match &rule.source {
                KeybindingSource::Default if !self.include_defaults => true,
                KeybindingSource::Extension(_) if !self.include_extensions => true,
                _ => false,
            };
            if dominated {
                continue;
            }
            let key_str = serialize_keybinding(&rule.keybinding);
            let when_str = rule
                .when
                .as_ref()
                .map(|w| format!("{w:?}"))
                .unwrap_or_default();
            entries.push(format!(
                "  {{ \"key\": \"{}\", \"command\": \"{}\", \"when\": \"{}\" }}",
                key_str, rule.command, when_str
            ));
        }
        if self.pretty {
            format!("[\n{}\n]", entries.join(",\n"))
        } else {
            format!("[{}]", entries.join(","))
        }
    }

    /// Count exportable rules.
    pub fn exportable_count(&self, resolver: &KeybindingResolver) -> usize {
        resolver
            .rules()
            .iter()
            .filter(|r| match &r.source {
                KeybindingSource::Default => self.include_defaults,
                KeybindingSource::Extension(_) => self.include_extensions,
                KeybindingSource::User => true,
            })
            .count()
    }
}

impl Default for KeybindingExporter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// KeybindingImporter – parse and validate keybinding entries
// ---------------------------------------------------------------------------

/// Validation error when importing keybindings.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportError {
    EmptyKey,
    EmptyCommand,
    InvalidKeySequence(String),
    DuplicateEntry(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::EmptyKey => write!(f, "key field is empty"),
            ImportError::EmptyCommand => write!(f, "command field is empty"),
            ImportError::InvalidKeySequence(s) => write!(f, "invalid key: {s}"),
            ImportError::DuplicateEntry(s) => write!(f, "duplicate: {s}"),
        }
    }
}

/// An entry parsed from a keybindings file.
#[derive(Debug, Clone)]
pub struct ImportedKeybinding {
    pub key: String,
    pub command: String,
    pub when: Option<String>,
    pub is_removal: bool,
}

/// Importer that validates and deduplicates keybinding entries.
#[derive(Debug)]
pub struct KeybindingImporter {
    strict: bool,
    seen_keys: std::collections::HashSet<String>,
}

impl KeybindingImporter {
    pub fn new(strict: bool) -> Self {
        Self {
            strict,
            seen_keys: std::collections::HashSet::new(),
        }
    }

    pub fn validate_entry(
        &mut self,
        key: &str,
        command: &str,
        when: Option<&str>,
    ) -> Result<ImportedKeybinding, ImportError> {
        if key.is_empty() {
            return Err(ImportError::EmptyKey);
        }
        if command.is_empty() {
            return Err(ImportError::EmptyCommand);
        }
        let is_removal = command.starts_with('-');
        let canonical = format!("{}::{}", key, command);
        if self.strict && self.seen_keys.contains(&canonical) {
            return Err(ImportError::DuplicateEntry(canonical));
        }
        self.seen_keys.insert(canonical);
        Ok(ImportedKeybinding {
            key: key.to_string(),
            command: command.to_string(),
            when: when.map(|s| s.to_string()),
            is_removal,
        })
    }

    pub fn imported_count(&self) -> usize {
        self.seen_keys.len()
    }

    pub fn reset(&mut self) {
        self.seen_keys.clear();
    }
}

// ---------------------------------------------------------------------------
// KeybindingDiff – compare two sets of keybindings
// ---------------------------------------------------------------------------

/// Describes differences between keybinding configurations.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffEntry {
    Added(String, String),
    Removed(String, String),
    Changed { key: String, old_cmd: String, new_cmd: String },
}

/// Compares two keybinding maps and returns the differences.
pub fn diff_keybindings(
    old: &[(String, String)],
    new: &[(String, String)],
) -> Vec<DiffEntry> {
    let old_map: std::collections::HashMap<&str, &str> =
        old.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let new_map: std::collections::HashMap<&str, &str> =
        new.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let mut result = Vec::new();
    for (k, v) in &new_map {
        match old_map.get(k) {
            None => result.push(DiffEntry::Added(k.to_string(), v.to_string())),
            Some(old_v) if old_v != v => result.push(DiffEntry::Changed {
                key: k.to_string(),
                old_cmd: old_v.to_string(),
                new_cmd: v.to_string(),
            }),
            _ => {}
        }
    }
    for (k, v) in &old_map {
        if !new_map.contains_key(k) {
            result.push(DiffEntry::Removed(k.to_string(), v.to_string()));
        }
    }
    result
}

// ---------------------------------------------------------------------------
// MergeStrategy – merge keybinding configs
// ---------------------------------------------------------------------------

/// Strategy for merging keybinding configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// User bindings override everything.
    UserWins,
    /// Extension bindings override defaults but not user.
    ExtensionThenDefault,
    /// Keep first occurrence only.
    FirstWins,
}

/// Merge two keybinding lists according to a strategy.
pub fn merge_keybindings(
    base: &[(String, String)],
    overlay: &[(String, String)],
    strategy: MergeStrategy,
) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let (first, second) = match strategy {
        MergeStrategy::UserWins | MergeStrategy::ExtensionThenDefault => (overlay, base),
        MergeStrategy::FirstWins => (base, overlay),
    };

    for (k, v) in first {
        if seen.insert(k.clone()) {
            result.push((k.clone(), v.clone()));
        }
    }
    for (k, v) in second {
        if seen.insert(k.clone()) {
            result.push((k.clone(), v.clone()));
        }
    }
    result
}

/// Count duplicate keys in a keybinding list.
pub fn count_duplicates(bindings: &[(String, String)]) -> usize {
    let mut seen = std::collections::HashSet::new();
    let mut dups = 0;
    for (k, _) in bindings {
        if !seen.insert(k) {
            dups += 1;
        }
    }
    dups
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vsedit_contextkey::ContextKeyValue;

    /// Simple in-memory context for testing.
    struct TestContext {
        values: HashMap<String, ContextKeyValue>,
    }

    impl TestContext {
        fn new() -> Self {
            Self {
                values: HashMap::new(),
            }
        }

        fn set(&mut self, key: &str, value: ContextKeyValue) {
            self.values.insert(key.to_string(), value);
        }
    }

    impl IContext for TestContext {
        fn get_value(&self, key: &str) -> Option<&ContextKeyValue> {
            self.values.get(key)
        }
    }

    fn rule(chords: &[KeyCodeChord], command: &str) -> KeybindingRule {
        KeybindingRule {
            keybinding: Keybinding { parts: chords.to_vec() },
            command: command.into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
            source: KeybindingSource::Default,
        }
    }

    fn rule_with_source(chords: &[KeyCodeChord], command: &str, source: KeybindingSource) -> KeybindingRule {
        KeybindingRule {
            keybinding: Keybinding { parts: chords.to_vec() },
            command: command.into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
            source,
        }
    }

    fn rule_when(chords: &[KeyCodeChord], command: &str, when: &str) -> KeybindingRule {
        KeybindingRule {
            keybinding: Keybinding { parts: chords.to_vec() },
            command: command.into(),
            args: None,
            when: Some(ContextKeyExpr::parse(when).unwrap()),
            weight: KeybindingWeight::EditorCore,
            source: KeybindingSource::Default,
        }
    }

    fn rule_weight(chords: &[KeyCodeChord], command: &str, weight: KeybindingWeight) -> KeybindingRule {
        KeybindingRule {
            keybinding: Keybinding { parts: chords.to_vec() },
            command: command.into(),
            args: None,
            when: None,
            weight,
            source: KeybindingSource::Default,
        }
    }

    // -- Single-chord matching --

    #[test]
    fn single_chord_match() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "workbench.action.files.save",
        ));

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.files.save".into(),
                args: None,
            }
        );
    }

    #[test]
    fn single_chord_no_match() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "save",
        ));

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyA)];
        assert_eq!(resolver.resolve(&ctx, &pressed), ResolveResult::NoMatch);
    }

    #[test]
    fn f1_matches_show_commands() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::just(KeyCode::F1)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.showCommands".into(),
                args: None,
            }
        );
    }

    // -- Multi-chord matching --

    #[test]
    fn multi_chord_more_chords_needed() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ],
            "editor.action.addCommentLine",
        ));

        let ctx = TestContext::new();
        let first = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyK)];
        assert_eq!(
            resolver.resolve(&ctx, &first),
            ResolveResult::MoreChordsNeeded
        );
    }

    #[test]
    fn multi_chord_full_match() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ],
            "editor.action.addCommentLine",
        ));

        let ctx = TestContext::new();
        let both = [
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        ];
        assert_eq!(
            resolver.resolve(&ctx, &both),
            ResolveResult::CommandMatch {
                command: "editor.action.addCommentLine".into(),
                args: None,
            }
        );
    }

    #[test]
    fn multi_chord_wrong_second_chord() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ],
            "editor.action.addCommentLine",
        ));

        let ctx = TestContext::new();
        let wrong = [
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyX),
        ];
        assert_eq!(resolver.resolve(&ctx, &wrong), ResolveResult::NoMatch);
    }

    // -- Context-based resolution --

    #[test]
    fn when_clause_satisfied() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule_when(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyD)],
            "editor.action.deleteLines",
            "editorTextFocus",
        ));

        let mut ctx = TestContext::new();
        ctx.set("editorTextFocus", ContextKeyValue::Bool(true));

        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyD)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "editor.action.deleteLines".into(),
                args: None,
            }
        );
    }

    #[test]
    fn when_clause_not_satisfied() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule_when(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyD)],
            "editor.action.deleteLines",
            "editorTextFocus",
        ));

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyD)];
        assert_eq!(resolver.resolve(&ctx, &pressed), ResolveResult::NoMatch);
    }

    #[test]
    fn when_clause_fallback_to_unconditional() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule_when(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyD)],
            "editor.action.deleteLines",
            "editorTextFocus",
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyD)],
            "fallback.action",
        ));

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyD)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "fallback.action".into(),
                args: None,
            }
        );
    }

    // -- Weight precedence --

    #[test]
    fn higher_weight_wins() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule_weight(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "low.priority",
            KeybindingWeight::EditorCore,
        ));
        resolver.add_rule(rule_weight(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "high.priority",
            KeybindingWeight::ExternalExtension,
        ));

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "high.priority".into(),
                args: None,
            }
        );
    }

    #[test]
    fn equal_weight_last_wins() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "first",
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "second",
        ));

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "second".into(),
                args: None,
            }
        );
    }

    // -- Negation rules --

    #[test]
    fn negation_removes_binding() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "workbench.action.files.save",
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "-workbench.action.files.save",
        ));

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        assert_eq!(resolver.resolve(&ctx, &pressed), ResolveResult::NoMatch);
    }

    #[test]
    fn negation_only_removes_specific_command() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "workbench.action.files.save",
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "other.save",
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "-workbench.action.files.save",
        ));

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "other.save".into(),
                args: None,
            }
        );
    }

    // -- Args forwarding --

    #[test]
    fn args_forwarded() {
        let mut resolver = KeybindingResolver::new();
        let mut r = rule(&[KeyCodeChord::just(KeyCode::F5)], "workbench.action.debug.start");
        r.args = Some(vec!["noDebug".into()]);
        resolver.add_rule(r);

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::just(KeyCode::F5)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.debug.start".into(),
                args: Some(vec!["noDebug".into()]),
            }
        );
    }

    // -- Empty chords --

    #[test]
    fn empty_chords_no_match() {
        let resolver = KeybindingResolver::new();
        let ctx = TestContext::new();
        assert_eq!(resolver.resolve(&ctx, &[]), ResolveResult::NoMatch);
    }

    // -- get_keybindings_for_command --

    #[test]
    fn get_keybindings_for_command() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);

        let bindings = resolver.get_keybindings_for_command("redo");
        assert_eq!(bindings.len(), 2);
    }

    #[test]
    fn get_keybindings_for_unknown_command() {
        let resolver = KeybindingResolver::new();
        let bindings = resolver.get_keybindings_for_command("nonexistent");
        assert!(bindings.is_empty());
    }

    // -- Default keybindings --

    #[test]
    fn default_keybindings_registered() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);

        let ctx = TestContext::new();

        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyC)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "editor.action.clipboardCopyAction".into(),
                args: None,
            }
        );

        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyZ)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "undo".into(),
                args: None,
            }
        );

        let pressed = [KeyCodeChord::new(true, true, false, false, KeyCode::KeyP)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.showCommands".into(),
                args: None,
            }
        );
    }

    // -- Weight ordering --

    #[test]
    fn weight_ordering() {
        assert!(KeybindingWeight::EditorCore < KeybindingWeight::EditorContrib);
        assert!(KeybindingWeight::EditorContrib < KeybindingWeight::WorkbenchContrib);
        assert!(KeybindingWeight::WorkbenchContrib < KeybindingWeight::BuiltinExtension);
        assert!(KeybindingWeight::BuiltinExtension < KeybindingWeight::ExternalExtension);
    }

    // -- Conditional negation --

    #[test]
    fn conditional_negation_only_applies_when_context_matches() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "workbench.action.files.save",
        ));
        resolver.add_rule(rule_when(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "-workbench.action.files.save",
            "inDebugMode",
        ));

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.files.save".into(),
                args: None,
            }
        );

        let mut ctx2 = TestContext::new();
        ctx2.set("inDebugMode", ContextKeyValue::Bool(true));
        assert_eq!(resolver.resolve(&ctx2, &pressed), ResolveResult::NoMatch);
    }

    // =====================================================================
    // Chord state machine tests
    // =====================================================================

    #[test]
    fn chord_state_partial_then_complete() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ],
            "editor.action.addCommentLine",
        ));

        let ctx = TestContext::new();
        let r1 = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            1000,
        );
        assert_eq!(r1, KeybindingMatch::PartialMatch);
        assert!(matches!(resolver.chord_state(), ChordState::FirstChord { .. }));

        let r2 = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            1500,
        );
        assert_eq!(
            r2,
            KeybindingMatch::ExactMatch {
                command: "editor.action.addCommentLine".into(),
                args: None,
            }
        );
        assert_eq!(resolver.chord_state(), &ChordState::None);
    }

    #[test]
    fn chord_state_timeout_resets() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ],
            "editor.action.addCommentLine",
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "save",
        ));

        let ctx = TestContext::new();
        let r1 = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            1000,
        );
        assert_eq!(r1, KeybindingMatch::PartialMatch);

        // Wait > 1000ms, press Ctrl+S — timeout, Ctrl+S matches standalone
        let r2 = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyS),
            2500,
        );
        assert_eq!(
            r2,
            KeybindingMatch::ExactMatch {
                command: "save".into(),
                args: None,
            }
        );
    }

    #[test]
    fn chord_state_wrong_second_chord_falls_through() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ],
            "comment",
        ));

        let ctx = TestContext::new();
        let r1 = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            100,
        );
        assert_eq!(r1, KeybindingMatch::PartialMatch);

        let r2 = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyX),
            200,
        );
        assert_eq!(r2, KeybindingMatch::NoMatch);
        assert_eq!(resolver.chord_state(), &ChordState::None);
    }

    #[test]
    fn chord_state_single_chord_immediate_match() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "save",
        ));

        let ctx = TestContext::new();
        let r = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyS),
            100,
        );
        assert_eq!(
            r,
            KeybindingMatch::ExactMatch {
                command: "save".into(),
                args: None,
            }
        );
        assert_eq!(resolver.chord_state(), &ChordState::None);
    }

    #[test]
    fn chord_state_no_match_at_all() {
        let mut resolver = KeybindingResolver::new();
        let ctx = TestContext::new();
        let r = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyQ),
            100,
        );
        assert_eq!(r, KeybindingMatch::NoMatch);
    }

    #[test]
    fn custom_chord_timeout() {
        let mut resolver = KeybindingResolver::new();
        resolver.set_chord_timeout(500);

        resolver.add_rule(rule(
            &[
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ],
            "comment",
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyC)],
            "copy",
        ));

        let ctx = TestContext::new();
        let r1 = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            100,
        );
        assert_eq!(r1, KeybindingMatch::PartialMatch);

        // Timeout at 500ms, press at 700 → falls through to Ctrl+C = copy
        let r2 = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            700,
        );
        assert_eq!(
            r2,
            KeybindingMatch::ExactMatch {
                command: "copy".into(),
                args: None,
            }
        );
    }

    // =====================================================================
    // Source-based conflict resolution tests
    // =====================================================================

    #[test]
    fn user_source_overrides_default() {
        let mut resolver = KeybindingResolver::new();
        let ctrl_s = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        resolver.add_rule(rule_with_source(&ctrl_s, "default.save", KeybindingSource::Default));
        resolver.add_rule(rule_with_source(&ctrl_s, "user.save", KeybindingSource::User));

        let ctx = TestContext::new();
        assert_eq!(
            resolver.resolve(&ctx, &ctrl_s),
            ResolveResult::CommandMatch {
                command: "user.save".into(),
                args: None,
            }
        );
    }

    #[test]
    fn extension_overrides_default() {
        let mut resolver = KeybindingResolver::new();
        let ctrl_s = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        resolver.add_rule(rule_with_source(&ctrl_s, "default.save", KeybindingSource::Default));
        resolver.add_rule(rule_with_source(
            &ctrl_s,
            "ext.save",
            KeybindingSource::Extension("myext".into()),
        ));

        let ctx = TestContext::new();
        assert_eq!(
            resolver.resolve(&ctx, &ctrl_s),
            ResolveResult::CommandMatch {
                command: "ext.save".into(),
                args: None,
            }
        );
    }

    #[test]
    fn user_overrides_extension() {
        let mut resolver = KeybindingResolver::new();
        let ctrl_s = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        resolver.add_rule(rule_with_source(
            &ctrl_s,
            "ext.save",
            KeybindingSource::Extension("myext".into()),
        ));
        resolver.add_rule(rule_with_source(&ctrl_s, "user.save", KeybindingSource::User));

        let ctx = TestContext::new();
        assert_eq!(
            resolver.resolve(&ctx, &ctrl_s),
            ResolveResult::CommandMatch {
                command: "user.save".into(),
                args: None,
            }
        );
    }

    #[test]
    fn source_priority_ordering() {
        assert!(KeybindingSource::Default.priority() < KeybindingSource::Extension("x".into()).priority());
        assert!(
            KeybindingSource::Extension("x".into()).priority() < KeybindingSource::User.priority()
        );
    }

    #[test]
    fn all_three_sources_user_wins() {
        let mut resolver = KeybindingResolver::new();
        let ctrl_s = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        resolver.add_rule(rule_with_source(&ctrl_s, "default.cmd", KeybindingSource::Default));
        resolver.add_rule(rule_with_source(
            &ctrl_s,
            "ext.cmd",
            KeybindingSource::Extension("ext1".into()),
        ));
        resolver.add_rule(rule_with_source(&ctrl_s, "user.cmd", KeybindingSource::User));

        let ctx = TestContext::new();
        assert_eq!(
            resolver.resolve(&ctx, &ctrl_s),
            ResolveResult::CommandMatch {
                command: "user.cmd".into(),
                args: None,
            }
        );
    }

    // =====================================================================
    // keybindings.json loading tests
    // =====================================================================

    #[test]
    fn load_json_basic() {
        let json = r#"[
            { "key": "ctrl+s", "command": "workbench.action.files.save" },
            { "key": "ctrl+shift+p", "command": "workbench.action.showCommands" }
        ]"#;

        let rules = load_keybindings_json(json, Platform::Linux).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].command, "workbench.action.files.save");
        assert_eq!(rules[1].command, "workbench.action.showCommands");
        assert_eq!(rules[0].source, KeybindingSource::User);
    }

    #[test]
    fn load_json_with_when() {
        let json = r#"[
            { "key": "ctrl+d", "command": "editor.action.deleteLines", "when": "editorTextFocus" }
        ]"#;

        let rules = load_keybindings_json(json, Platform::Linux).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].when.is_some());
    }

    #[test]
    fn load_json_removal_with_dash() {
        let json = r#"[
            { "key": "ctrl+s", "command": "-workbench.action.files.save" }
        ]"#;

        let rules = load_keybindings_json(json, Platform::Linux).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].command.starts_with('-'));
    }

    #[test]
    fn load_json_removal_suppresses_default() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);

        let json = r#"[
            { "key": "ctrl+s", "command": "-workbench.action.files.save" }
        ]"#;
        let user_rules = load_keybindings_json(json, Platform::Linux).unwrap();
        for r in user_rules {
            resolver.add_rule(r);
        }

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        assert_eq!(resolver.resolve(&ctx, &pressed), ResolveResult::NoMatch);
    }

    #[test]
    fn load_json_two_chord_binding() {
        let json = r#"[
            { "key": "ctrl+k ctrl+c", "command": "editor.action.addCommentLine" }
        ]"#;

        let rules = load_keybindings_json(json, Platform::Linux).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].keybinding.parts.len(), 2);
    }

    #[test]
    fn load_json_invalid_key_returns_error() {
        let json = r#"[
            { "key": "ctrl+not_a_key", "command": "something" }
        ]"#;
        assert!(load_keybindings_json(json, Platform::Linux).is_err());
    }

    #[test]
    fn load_json_invalid_json_returns_error() {
        assert!(load_keybindings_json("not json", Platform::Linux).is_err());
    }

    #[test]
    fn load_json_empty_array() {
        let rules = load_keybindings_json("[]", Platform::Linux).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn load_json_with_args() {
        let json = r#"[
            { "key": "ctrl+s", "command": "save", "args": ["--force"] }
        ]"#;
        let rules = load_keybindings_json(json, Platform::Linux).unwrap();
        assert_eq!(rules[0].args, Some(vec!["--force".to_string()]));
    }

    // =====================================================================
    // Default keybindings coverage (50+)
    // =====================================================================

    #[test]
    fn default_keybindings_at_least_50() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        assert!(
            resolver.rules().len() >= 50,
            "expected >= 50 default keybindings, got {}",
            resolver.rules().len()
        );
    }

    #[test]
    fn default_ctrl_w_closes_editor() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyW)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.closeActiveEditor".into(),
                args: None,
            }
        );
    }

    #[test]
    fn default_ctrl_b_toggles_sidebar() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyB)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.toggleSidebarVisibility".into(),
                args: None,
            }
        );
    }

    #[test]
    fn default_ctrl_backtick_toggles_terminal() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::Backquote)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.terminal.toggleTerminal".into(),
                args: None,
            }
        );
    }

    #[test]
    fn default_comment_chords_with_context() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);

        let mut ctx = TestContext::new();
        ctx.set("editorTextFocus", ContextKeyValue::Bool(true));

        let both = [
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
        ];
        assert_eq!(
            resolver.resolve(&ctx, &both),
            ResolveResult::CommandMatch {
                command: "editor.action.addCommentLine".into(),
                args: None,
            }
        );

        let both_u = [
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyU),
        ];
        assert_eq!(
            resolver.resolve(&ctx, &both_u),
            ResolveResult::CommandMatch {
                command: "editor.action.removeCommentLine".into(),
                args: None,
            }
        );
    }

    #[test]
    fn default_f12_needs_context() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::just(KeyCode::F12)];
        assert_eq!(resolver.resolve(&ctx, &pressed), ResolveResult::NoMatch);

        let mut ctx2 = TestContext::new();
        ctx2.set("editorHasDefinitionProvider", ContextKeyValue::Bool(true));
        ctx2.set("editorTextFocus", ContextKeyValue::Bool(true));
        assert_eq!(
            resolver.resolve(&ctx2, &pressed),
            ResolveResult::CommandMatch {
                command: "editor.action.revealDefinition".into(),
                args: None,
            }
        );
    }

    #[test]
    fn default_alt_up_moves_line() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        let mut ctx = TestContext::new();
        ctx.set("editorTextFocus", ContextKeyValue::Bool(true));

        let pressed = [KeyCodeChord::new(false, false, true, false, KeyCode::UpArrow)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "editor.action.moveLinesUpAction".into(),
                args: None,
            }
        );
    }

    #[test]
    fn default_ctrl_slash_comment_line() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        let mut ctx = TestContext::new();
        ctx.set("editorTextFocus", ContextKeyValue::Bool(true));

        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::Slash)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "editor.action.commentLine".into(),
                args: None,
            }
        );
    }

    #[test]
    fn default_ctrl_d_add_selection() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        let mut ctx = TestContext::new();
        ctx.set("editorFocus", ContextKeyValue::Bool(true));

        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyD)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "editor.action.addSelectionToNextFindMatch".into(),
                args: None,
            }
        );
    }

    #[test]
    fn default_f2_rename() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        let mut ctx = TestContext::new();
        ctx.set("editorHasRenameProvider", ContextKeyValue::Bool(true));
        ctx.set("editorTextFocus", ContextKeyValue::Bool(true));

        let pressed = [KeyCodeChord::just(KeyCode::F2)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "editor.action.rename".into(),
                args: None,
            }
        );
    }

    #[test]
    fn default_ctrl_tab_next_editor() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        let ctx = TestContext::new();

        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::Tab)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.nextEditor".into(),
                args: None,
            }
        );
    }

    // =====================================================================
    // when_specificity helper
    // =====================================================================

    #[test]
    fn when_specificity_counts_terms() {
        let r0 = rule(&[KeyCodeChord::just(KeyCode::F5)], "cmd");
        assert_eq!(when_specificity(&r0), 0);

        let r1 = rule_when(&[KeyCodeChord::just(KeyCode::F5)], "cmd", "editorFocus");
        assert_eq!(when_specificity(&r1), 1);

        let r2 = rule_when(
            &[KeyCodeChord::just(KeyCode::F5)],
            "cmd",
            "editorFocus && !inDebugMode",
        );
        assert_eq!(when_specificity(&r2), 2);
    }

    // =====================================================================
    // ChordState enum
    // =====================================================================

    #[test]
    fn chord_state_default_timeout() {
        assert_eq!(ChordState::DEFAULT_TIMEOUT_MS, 1000);
    }

    #[test]
    fn chord_state_none_variant() {
        let state = ChordState::None;
        assert_eq!(state, ChordState::None);
    }

    #[test]
    fn chord_state_first_chord_variant() {
        let state = ChordState::FirstChord {
            chord: KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            timestamp_ms: 500,
        };
        assert!(matches!(state, ChordState::FirstChord { .. }));
    }

    // =====================================================================
    // reset_chord_state
    // =====================================================================

    #[test]
    fn reset_chord_state_clears() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ],
            "comment",
        ));

        let ctx = TestContext::new();
        let _ = resolver.resolve_key(
            &ctx,
            KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
            100,
        );
        assert!(matches!(resolver.chord_state(), ChordState::FirstChord { .. }));

        resolver.reset_chord_state();
        assert_eq!(resolver.chord_state(), &ChordState::None);
    }

    // =====================================================================
    // KeybindingRule methods
    // =====================================================================

    #[test]
    fn rule_is_removal() {
        let normal = rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "save",
        );
        assert!(!normal.is_removal());
        assert!(normal.removal_target().is_none());

        let removal = rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "-save",
        );
        assert!(removal.is_removal());
        assert_eq!(removal.removal_target(), Some("save"));
    }

    #[test]
    fn rule_is_conditional() {
        let uncond = rule(
            &[KeyCodeChord::just(KeyCode::F5)],
            "debug.start",
        );
        assert!(!uncond.is_conditional());

        let cond = rule_when(
            &[KeyCodeChord::just(KeyCode::F5)],
            "debug.start",
            "inDebugMode",
        );
        assert!(cond.is_conditional());
    }

    #[test]
    fn rule_chord_count() {
        let single = rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "save",
        );
        assert_eq!(single.chord_count(), 1);

        let multi = rule(
            &[
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ],
            "comment",
        );
        assert_eq!(multi.chord_count(), 2);
    }

    #[test]
    fn rule_serialize_key() {
        let r = rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "save",
        );
        let serialized = r.serialize_key();
        assert!(!serialized.is_empty());
        // Should contain 'ctrl' and 's' (case-insensitive)
        let lower = serialized.to_lowercase();
        assert!(lower.contains("ctrl"), "expected ctrl in '{lower}'");
    }

    #[test]
    fn rule_label_not_empty() {
        let r = rule(
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "save",
        );
        let label = r.label(Platform::Linux);
        assert!(!label.is_empty());
    }

    // =====================================================================
    // KeybindingSource methods
    // =====================================================================

    #[test]
    fn source_extension_id() {
        assert_eq!(
            KeybindingSource::Extension("rust-analyzer".into()).extension_id(),
            Some("rust-analyzer"),
        );
        assert_eq!(KeybindingSource::Default.extension_id(), None);
        assert_eq!(KeybindingSource::User.extension_id(), None);
    }

    #[test]
    fn source_is_user() {
        assert!(KeybindingSource::User.is_user());
        assert!(!KeybindingSource::Default.is_user());
        assert!(!KeybindingSource::Extension("x".into()).is_user());
    }

    #[test]
    fn source_display_name() {
        assert_eq!(KeybindingSource::Default.display_name(), "Default");
        assert_eq!(KeybindingSource::User.display_name(), "User");
        assert_eq!(
            KeybindingSource::Extension("myext".into()).display_name(),
            "Extension (myext)",
        );
    }

    // =====================================================================
    // ResolveResult methods
    // =====================================================================

    #[test]
    fn resolve_result_accessors() {
        let no_match = ResolveResult::NoMatch;
        assert!(!no_match.is_match());
        assert!(no_match.command().is_none());
        assert!(no_match.args().is_none());

        let more = ResolveResult::MoreChordsNeeded;
        assert!(!more.is_match());
        assert!(more.command().is_none());

        let matched = ResolveResult::CommandMatch {
            command: "save".into(),
            args: Some(vec!["--force".into()]),
        };
        assert!(matched.is_match());
        assert_eq!(matched.command(), Some("save"));
        assert_eq!(matched.args(), Some(&["--force".to_string()][..]));
    }

    #[test]
    fn resolve_result_no_args() {
        let matched = ResolveResult::CommandMatch {
            command: "save".into(),
            args: None,
        };
        assert!(matched.is_match());
        assert!(matched.args().is_none());
    }

    // =====================================================================
    // KeybindingResolver — additional methods
    // =====================================================================

    #[test]
    fn resolver_rule_count() {
        let mut resolver = KeybindingResolver::new();
        assert_eq!(resolver.rule_count(), 0);

        resolver.add_rule(rule(
            &[KeyCodeChord::just(KeyCode::F1)],
            "cmd1",
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::just(KeyCode::F2)],
            "cmd2",
        ));
        assert_eq!(resolver.rule_count(), 2);
    }

    #[test]
    fn resolver_remove_rules_for_command() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(&[KeyCodeChord::just(KeyCode::F1)], "save"));
        resolver.add_rule(rule(&[KeyCodeChord::just(KeyCode::F2)], "save"));
        resolver.add_rule(rule(&[KeyCodeChord::just(KeyCode::F3)], "copy"));
        assert_eq!(resolver.rule_count(), 3);

        resolver.remove_rules_for_command("save");
        assert_eq!(resolver.rule_count(), 1);
        assert_eq!(resolver.rules()[0].command, "copy");
    }

    #[test]
    fn resolver_remove_rules_from_extension() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule_with_source(
            &[KeyCodeChord::just(KeyCode::F1)],
            "ext.cmd",
            KeybindingSource::Extension("ext-a".into()),
        ));
        resolver.add_rule(rule_with_source(
            &[KeyCodeChord::just(KeyCode::F2)],
            "ext.cmd2",
            KeybindingSource::Extension("ext-b".into()),
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::just(KeyCode::F3)],
            "default.cmd",
        ));
        assert_eq!(resolver.rule_count(), 3);

        resolver.remove_rules_from_extension("ext-a");
        assert_eq!(resolver.rule_count(), 2);
        assert!(resolver.rules_from_extension("ext-a").is_empty());
        assert_eq!(resolver.rules_from_extension("ext-b").len(), 1);
    }

    #[test]
    fn resolver_command_ids() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(&[KeyCodeChord::just(KeyCode::F1)], "zzz.cmd"));
        resolver.add_rule(rule(&[KeyCodeChord::just(KeyCode::F2)], "aaa.cmd"));
        resolver.add_rule(rule(&[KeyCodeChord::just(KeyCode::F3)], "aaa.cmd")); // dup
        resolver.add_rule(rule(&[KeyCodeChord::just(KeyCode::F4)], "-zzz.removed"));

        let ids = resolver.command_ids();
        assert_eq!(ids, vec!["aaa.cmd", "zzz.cmd"]);
    }

    #[test]
    fn resolver_has_command() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(&[KeyCodeChord::just(KeyCode::F1)], "save"));
        assert!(resolver.has_command("save"));
        assert!(!resolver.has_command("copy"));
    }

    #[test]
    fn resolver_load_json() {
        let mut resolver = KeybindingResolver::new();
        let json = r#"[
            { "key": "ctrl+s", "command": "save" },
            { "key": "ctrl+z", "command": "undo" }
        ]"#;
        let count = resolver.load_json(json, Platform::Linux).unwrap();
        assert_eq!(count, 2);
        assert_eq!(resolver.rule_count(), 2);
        assert!(resolver.has_command("save"));
        assert!(resolver.has_command("undo"));
    }

    #[test]
    fn resolver_load_json_error() {
        let mut resolver = KeybindingResolver::new();
        let result = resolver.load_json("not json", Platform::Linux);
        assert!(result.is_err());
        assert_eq!(resolver.rule_count(), 0);
    }

    #[test]
    fn resolver_rules_from_extension() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule_with_source(
            &[KeyCodeChord::just(KeyCode::F1)],
            "ext.cmd1",
            KeybindingSource::Extension("my-ext".into()),
        ));
        resolver.add_rule(rule_with_source(
            &[KeyCodeChord::just(KeyCode::F2)],
            "ext.cmd2",
            KeybindingSource::Extension("my-ext".into()),
        ));
        resolver.add_rule(rule(
            &[KeyCodeChord::just(KeyCode::F3)],
            "default.cmd",
        ));

        let ext_rules = resolver.rules_from_extension("my-ext");
        assert_eq!(ext_rules.len(), 2);
        assert!(resolver.rules_from_extension("other").is_empty());
    }

    // =====================================================================
    // Default resolver builder integration
    // =====================================================================

    #[test]
    fn default_resolver_command_ids_are_sorted_and_deduped() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        let ids = resolver.command_ids();
        // Must be sorted
        for w in ids.windows(2) {
            assert!(w[0] <= w[1], "not sorted: {} > {}", w[0], w[1]);
        }
        // No duplicates
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn default_resolver_has_expected_commands() {
        let mut resolver = KeybindingResolver::new();
        register_default_keybindings(&mut resolver);
        assert!(resolver.has_command("undo"));
        assert!(resolver.has_command("redo"));
        assert!(resolver.has_command("workbench.action.files.save"));
        assert!(resolver.has_command("workbench.action.showCommands"));
        assert!(!resolver.has_command("nonexistent.command"));
    }

    // =================================================================
    // KeybindingExporter tests
    // =================================================================

    #[test]
    fn exporter_default_excludes_defaults() {
        let exporter = KeybindingExporter::new();
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::just(KeyCode::F1)],
            "cmd.default",
        ));
        assert_eq!(exporter.exportable_count(&resolver), 0);
    }

    #[test]
    fn exporter_include_defaults_works() {
        let exporter = KeybindingExporter::new().include_defaults(true);
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::just(KeyCode::F1)],
            "cmd.default",
        ));
        assert!(exporter.exportable_count(&resolver) > 0);
    }

    #[test]
    fn exporter_pretty_output_has_newlines() {
        let exporter = KeybindingExporter::new().include_defaults(true);
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::just(KeyCode::F1)],
            "cmd.default",
        ));
        let json = exporter.export(&resolver);
        assert!(json.contains('\n'));
    }

    #[test]
    fn exporter_compact_output() {
        let exporter = KeybindingExporter::new()
            .include_defaults(true)
            .pretty(false);
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(rule(
            &[KeyCodeChord::just(KeyCode::F1)],
            "cmd.default",
        ));
        let json = exporter.export(&resolver);
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
    }

    // =================================================================
    // KeybindingImporter tests
    // =================================================================

    #[test]
    fn importer_validates_empty_key() {
        let mut imp = KeybindingImporter::new(true);
        assert_eq!(imp.validate_entry("", "cmd", None).unwrap_err(), ImportError::EmptyKey);
    }

    #[test]
    fn importer_validates_empty_command() {
        let mut imp = KeybindingImporter::new(true);
        assert_eq!(imp.validate_entry("ctrl+a", "", None).unwrap_err(), ImportError::EmptyCommand);
    }

    #[test]
    fn importer_detects_removal() {
        let mut imp = KeybindingImporter::new(false);
        let entry = imp.validate_entry("ctrl+a", "-editor.action.cut", None).unwrap();
        assert!(entry.is_removal);
    }

    #[test]
    fn importer_strict_detects_duplicate() {
        let mut imp = KeybindingImporter::new(true);
        imp.validate_entry("ctrl+a", "cmd1", None).unwrap();
        assert!(matches!(imp.validate_entry("ctrl+a", "cmd1", None), Err(ImportError::DuplicateEntry(_))));
    }

    #[test]
    fn importer_reset_clears_state() {
        let mut imp = KeybindingImporter::new(true);
        imp.validate_entry("ctrl+a", "cmd1", None).unwrap();
        imp.reset();
        assert_eq!(imp.imported_count(), 0);
    }

    // =================================================================
    // KeybindingDiff tests
    // =================================================================

    #[test]
    fn diff_detects_added() {
        let old = vec![];
        let new = vec![("k1".into(), "cmd1".into())];
        let d = diff_keybindings(&old, &new);
        assert!(matches!(&d[0], DiffEntry::Added(..)));
    }

    #[test]
    fn diff_detects_removed() {
        let old = vec![("k1".into(), "cmd1".into())];
        let d = diff_keybindings(&old, &[]);
        assert!(matches!(&d[0], DiffEntry::Removed(..)));
    }

    #[test]
    fn diff_detects_changed() {
        let old = vec![("k1".into(), "old".into())];
        let new = vec![("k1".into(), "new".into())];
        let d = diff_keybindings(&old, &new);
        assert!(matches!(&d[0], DiffEntry::Changed { .. }));
    }

    #[test]
    fn diff_identical_is_empty() {
        let a = vec![("k1".into(), "cmd1".into())];
        assert!(diff_keybindings(&a, &a).is_empty());
    }

    // =================================================================
    // MergeStrategy tests
    // =================================================================

    #[test]
    fn merge_user_wins() {
        let base = vec![("k1".into(), "default".into())];
        let overlay = vec![("k1".into(), "user".into())];
        let merged = merge_keybindings(&base, &overlay, MergeStrategy::UserWins);
        assert_eq!(merged[0].1, "user");
    }

    #[test]
    fn merge_first_wins() {
        let base = vec![("k1".into(), "base".into())];
        let overlay = vec![("k1".into(), "overlay".into())];
        let merged = merge_keybindings(&base, &overlay, MergeStrategy::FirstWins);
        assert_eq!(merged[0].1, "base");
    }

    #[test]
    fn merge_disjoint() {
        let base = vec![("k1".into(), "a".into())];
        let overlay = vec![("k2".into(), "b".into())];
        let merged = merge_keybindings(&base, &overlay, MergeStrategy::UserWins);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn count_duplicates_works() {
        let b = vec![("k1".into(), "a".into()), ("k2".into(), "b".into()), ("k1".into(), "c".into())];
        assert_eq!(count_duplicates(&b), 1);
    }

    #[test]
    fn import_error_display() {
        assert_eq!(format!("{}", ImportError::EmptyKey), "key field is empty");
        assert!(format!("{}", ImportError::InvalidKeySequence("bad".into())).contains("bad"));
    }

}
