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

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// KeybindingScope
// ---------------------------------------------------------------------------

/// Scope-based resolution for keybindings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeybindingScope {
    Global,
    Editor,
    Panel,
    Dialog,
    Custom(String),
}

impl KeybindingScope {
    pub fn parent_scope(&self) -> Option<KeybindingScope> {
        match self {
            KeybindingScope::Global => None,
            KeybindingScope::Editor => Some(KeybindingScope::Global),
            KeybindingScope::Panel => Some(KeybindingScope::Global),
            KeybindingScope::Dialog => Some(KeybindingScope::Global),
            KeybindingScope::Custom(_) => Some(KeybindingScope::Global),
        }
    }

    pub fn is_descendant_of(&self, ancestor: &KeybindingScope) -> bool {
        if self == ancestor {
            return true;
        }
        match self.parent_scope() {
            Some(parent) => parent.is_descendant_of(ancestor),
            None => false,
        }
    }

    pub fn matches_or_inherits(&self, target: &KeybindingScope) -> bool {
        self == target || self.is_descendant_of(target)
    }

    pub fn depth(&self) -> u32 {
        match self {
            KeybindingScope::Global => 0,
            _ => 1,
        }
    }
}

impl fmt::Display for KeybindingScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeybindingScope::Global => write!(f, "global"),
            KeybindingScope::Editor => write!(f, "editor"),
            KeybindingScope::Panel => write!(f, "panel"),
            KeybindingScope::Dialog => write!(f, "dialog"),
            KeybindingScope::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

// ---------------------------------------------------------------------------
// KeybindingOverrideTracker
// ---------------------------------------------------------------------------

/// Tracks user overrides vs default keybindings.
pub struct KeybindingOverrideTracker {
    overrides: std::collections::HashMap<String, (String, String)>,
}

impl KeybindingOverrideTracker {
    pub fn new() -> Self {
        Self {
            overrides: std::collections::HashMap::new(),
        }
    }

    /// Register an override: command -> (original_chord, new_chord).
    pub fn add_override(&mut self, command: &str, original: &str, new_chord: &str) {
        self.overrides.insert(
            command.to_string(),
            (original.to_string(), new_chord.to_string()),
        );
    }

    pub fn is_overridden(&self, command: &str) -> bool {
        self.overrides.contains_key(command)
    }

    pub fn original_binding(&self, command: &str) -> Option<&str> {
        self.overrides.get(command).map(|(orig, _)| orig.as_str())
    }

    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    pub fn reset_to_default(&mut self, command: &str) -> bool {
        self.overrides.remove(command).is_some()
    }

    pub fn list_overrides(&self) -> Vec<(&str, &str, &str)> {
        self.overrides
            .iter()
            .map(|(cmd, (orig, new))| (cmd.as_str(), orig.as_str(), new.as_str()))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// WhenClauseEvaluator
// ---------------------------------------------------------------------------

/// Simple boolean expression evaluator for when clauses.
/// Supports: identifiers, `&&`, `||`, `!`, parentheses.
pub struct WhenClauseEvaluator;

impl WhenClauseEvaluator {
    /// Evaluate a simple when-clause expression against a context map.
    /// Supports: `key`, `!key`, `key1 && key2`, `key1 || key2`.
    pub fn evaluate(expr: &str, context: &std::collections::HashMap<String, bool>) -> bool {
        let expr = expr.trim();
        if expr.is_empty() {
            return true;
        }

        // Split on || first (lowest precedence)
        if let Some(pos) = Self::find_operator(expr, "||") {
            let left = &expr[..pos];
            let right = &expr[pos + 2..];
            return Self::evaluate(left, context) || Self::evaluate(right, context);
        }

        // Split on && (higher precedence)
        if let Some(pos) = Self::find_operator(expr, "&&") {
            let left = &expr[..pos];
            let right = &expr[pos + 2..];
            return Self::evaluate(left, context) && Self::evaluate(right, context);
        }

        // Handle negation
        let expr = expr.trim();
        if let Some(rest) = expr.strip_prefix('!') {
            return !Self::evaluate(rest.trim(), context);
        }

        // Lookup key in context
        context.get(expr.trim()).copied().unwrap_or(false)
    }

    fn find_operator(expr: &str, op: &str) -> Option<usize> {
        let bytes = expr.as_bytes();
        let op_bytes = op.as_bytes();
        let op_len = op_bytes.len();
        if bytes.len() < op_len {
            return None;
        }
        for i in (0..=bytes.len() - op_len).rev() {
            if &bytes[i..i + op_len] == op_bytes {
                return Some(i);
            }
        }
        None
    }

    /// Parse and list the variable names referenced in an expression.
    pub fn referenced_keys(expr: &str) -> Vec<String> {
        expr.split(|c: char| c == '&' || c == '|' || c == '!' || c == '(' || c == ')')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}


/// Configuration manager for keybinding_svc functionality.
pub struct KeybindingSvcConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl KeybindingSvcConfig {
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

    pub fn merge(&mut self, other: &KeybindingSvcConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for keybinding_svc operations.
pub struct KeybindingSvcRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl KeybindingSvcRateTracker {
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

/// Validation result collector for keybinding_svc.
pub struct KeybindingSvcValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl KeybindingSvcValidator {
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

    pub fn merge(&mut self, other: &KeybindingSvcValidator) {
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
// xa_ extended helpers for keybinding_svc
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaKeybindingSvcRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaKeybindingSvcRingBuf {
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
pub struct XaKeybindingSvcCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaKeybindingSvcCounter {
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

impl Default for XaKeybindingSvcCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 102
// ---------------------------------------------------------------------------

/// Generic object pool `Xc102Pool<T>`.
pub struct Xc102Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc102Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc102PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc102Pool<T> {
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
    pub fn stats(&self) -> Xc102PoolStats {
        Xc102PoolStats {
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

impl<T> Default for Xc102Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc102Scheduler`.
pub struct Xc102Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc102Scheduler {
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

impl Default for Xc102Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_102 hash for the given byte slice.
pub fn xc_102_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_102 convention.
pub fn xc_102_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_53 deepening: state machine + event bus ---

/// States for the Xd53 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd53State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd53State {
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
pub struct Xd53Transition {
    pub from: Xd53State,
    pub to: Xd53State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd53StateMachine {
    current: Xd53State,
    history: Vec<Xd53Transition>,
    step_counter: usize,
}

impl Xd53StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd53State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd53State {
        self.current
    }

    pub fn history(&self) -> &[Xd53Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd53State) -> Result<Xd53State, String> {
        let allowed = match (self.current, target) {
            (Xd53State::Idle, Xd53State::Running) => true,
            (Xd53State::Running, Xd53State::Paused) => true,
            (Xd53State::Running, Xd53State::Done) => true,
            (Xd53State::Paused, Xd53State::Running) => true,
            (Xd53State::Paused, Xd53State::Done) => true,
            (Xd53State::Done, Xd53State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_53: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd53Transition {
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
            "Xd53SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd53State> {
        let prefix = "Xd53SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd53State::Idle),
            "Running" => Some(Xd53State::Running),
            "Paused" => Some(Xd53State::Paused),
            "Done" => Some(Xd53State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd53State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd53 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd53Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd53Event {
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

type Xd53HandlerFn = Box<dyn Fn(&Xd53Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd53EventBus {
    handlers: Vec<(usize, Option<String>, Xd53HandlerFn)>,
    next_id: usize,
    published: Vec<Xd53Event>,
}

impl Xd53EventBus {
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
        F: Fn(&Xd53Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd53Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd53Event) {
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

    pub fn published_events(&self) -> &[Xd53Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #51
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf51Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf51TrieNode {
    children: std::collections::HashMap<char, Xf51TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf51Trie {
    root: Xf51TrieNode,
    count: usize,
}

impl Xf51Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf51TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf51TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf51TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf51BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf51BloomFilter {
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

    // -- KeybindingScope tests --

    #[test]
    fn scope_parent() {
        assert_eq!(KeybindingScope::Editor.parent_scope(), Some(KeybindingScope::Global));
        assert_eq!(KeybindingScope::Global.parent_scope(), None);
    }

    #[test]
    fn scope_descendant() {
        assert!(KeybindingScope::Editor.is_descendant_of(&KeybindingScope::Global));
        assert!(!KeybindingScope::Global.is_descendant_of(&KeybindingScope::Editor));
    }

    #[test]
    fn scope_matches_or_inherits() {
        assert!(KeybindingScope::Panel.matches_or_inherits(&KeybindingScope::Global));
        assert!(KeybindingScope::Panel.matches_or_inherits(&KeybindingScope::Panel));
    }

    #[test]
    fn scope_display() {
        assert_eq!(format!("{}", KeybindingScope::Editor), "editor");
        assert_eq!(format!("{}", KeybindingScope::Custom("x".into())), "custom:x");
    }

    #[test]
    fn scope_depth() {
        assert_eq!(KeybindingScope::Global.depth(), 0);
        assert_eq!(KeybindingScope::Dialog.depth(), 1);
    }

    // -- KeybindingOverrideTracker tests --

    #[test]
    fn override_tracker_add_and_check() {
        let mut tracker = KeybindingOverrideTracker::new();
        tracker.add_override("save", "Ctrl+S", "Ctrl+Shift+S");
        assert!(tracker.is_overridden("save"));
        assert!(!tracker.is_overridden("undo"));
    }

    #[test]
    fn override_tracker_original_binding() {
        let mut tracker = KeybindingOverrideTracker::new();
        tracker.add_override("save", "Ctrl+S", "Ctrl+Shift+S");
        assert_eq!(tracker.original_binding("save"), Some("Ctrl+S"));
    }

    #[test]
    fn override_tracker_count() {
        let mut tracker = KeybindingOverrideTracker::new();
        tracker.add_override("save", "Ctrl+S", "Ctrl+Shift+S");
        tracker.add_override("undo", "Ctrl+Z", "Ctrl+Shift+Z");
        assert_eq!(tracker.override_count(), 2);
    }

    #[test]
    fn override_tracker_reset() {
        let mut tracker = KeybindingOverrideTracker::new();
        tracker.add_override("save", "Ctrl+S", "Ctrl+Shift+S");
        assert!(tracker.reset_to_default("save"));
        assert!(!tracker.is_overridden("save"));
    }

    #[test]
    fn override_tracker_list() {
        let mut tracker = KeybindingOverrideTracker::new();
        tracker.add_override("save", "Ctrl+S", "Ctrl+Shift+S");
        let list = tracker.list_overrides();
        assert_eq!(list.len(), 1);
    }

    // -- WhenClauseEvaluator tests --

    #[test]
    fn when_simple_true() {
        let mut ctx = HashMap::new();
        ctx.insert("editorFocus".to_string(), true);
        assert!(WhenClauseEvaluator::evaluate("editorFocus", &ctx));
    }

    #[test]
    fn when_simple_false() {
        let ctx = HashMap::new();
        assert!(!WhenClauseEvaluator::evaluate("editorFocus", &ctx));
    }

    #[test]
    fn when_negation() {
        let mut ctx = HashMap::new();
        ctx.insert("editorReadonly".to_string(), false);
        assert!(WhenClauseEvaluator::evaluate("!editorReadonly", &ctx));
    }

    #[test]
    fn when_and() {
        let mut ctx = HashMap::new();
        ctx.insert("a".to_string(), true);
        ctx.insert("b".to_string(), true);
        assert!(WhenClauseEvaluator::evaluate("a && b", &ctx));
    }

    #[test]
    fn when_or() {
        let mut ctx = HashMap::new();
        ctx.insert("a".to_string(), false);
        ctx.insert("b".to_string(), true);
        assert!(WhenClauseEvaluator::evaluate("a || b", &ctx));
    }

    #[test]
    fn when_empty() {
        let ctx = HashMap::new();
        assert!(WhenClauseEvaluator::evaluate("", &ctx));
    }

    #[test]
    fn when_referenced_keys() {
        let keys = WhenClauseEvaluator::referenced_keys("a && !b || c");
        assert!(keys.contains(&"a".to_string()));
        assert!(keys.contains(&"b".to_string()));
        assert!(keys.contains(&"c".to_string()));
    }


    #[test]
    fn keybinding_svc_config_new() {
        let cfg = KeybindingSvcConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn keybinding_svc_config_set_get() {
        let mut cfg = KeybindingSvcConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn keybinding_svc_config_remove() {
        let mut cfg = KeybindingSvcConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn keybinding_svc_config_keys_sorted() {
        let mut cfg = KeybindingSvcConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn keybinding_svc_config_bump_version() {
        let mut cfg = KeybindingSvcConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn keybinding_svc_config_clear() {
        let mut cfg = KeybindingSvcConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn keybinding_svc_config_merge() {
        let mut cfg1 = KeybindingSvcConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = KeybindingSvcConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn keybinding_svc_config_disable() {
        let mut cfg = KeybindingSvcConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn keybinding_svc_rate_tracker_empty() {
        let rt = KeybindingSvcRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn keybinding_svc_rate_tracker_record() {
        let mut rt = KeybindingSvcRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn keybinding_svc_rate_tracker_prune() {
        let mut rt = KeybindingSvcRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn keybinding_svc_validator_valid() {
        let v = KeybindingSvcValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn keybinding_svc_validator_errors() {
        let mut v = KeybindingSvcValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn keybinding_svc_validator_clear() {
        let mut v = KeybindingSvcValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn keybinding_svc_validator_merge() {
        let mut v1 = KeybindingSvcValidator::new();
        v1.add_error("e1");
        let mut v2 = KeybindingSvcValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn keybinding_svc_rate_tracker_clear() {
        let mut rt = KeybindingSvcRateTracker::new(1000);
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


    // xa_ extended tests for keybinding_svc
    #[test]
    fn xa_keybinding_svc_ring_new() {
        let rb = super::XaKeybindingSvcRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_keybinding_svc_ring_push_len() {
        let mut rb = super::XaKeybindingSvcRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_keybinding_svc_ring_wrap() {
        let mut rb = super::XaKeybindingSvcRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_keybinding_svc_ring_mean_empty() {
        let rb = super::XaKeybindingSvcRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_keybinding_svc_ring_mean_values() {
        let mut rb = super::XaKeybindingSvcRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_keybinding_svc_ring_min_max() {
        let mut rb = super::XaKeybindingSvcRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_keybinding_svc_ring_iter() {
        let mut rb = super::XaKeybindingSvcRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_keybinding_svc_counter_new() {
        let c = super::XaKeybindingSvcCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_keybinding_svc_counter_inc() {
        let mut c = super::XaKeybindingSvcCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_keybinding_svc_counter_inc_by() {
        let mut c = super::XaKeybindingSvcCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_keybinding_svc_counter_reset() {
        let mut c = super::XaKeybindingSvcCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_keybinding_svc_counter_clear() {
        let mut c = super::XaKeybindingSvcCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_keybinding_svc_counter_default() {
        let c = super::XaKeybindingSvcCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 102 ----

    #[test]
    fn xc_102_pool_new_empty() {
        let pool: super::Xc102Pool<i32> = super::Xc102Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_102_pool_release_acquire() {
        let mut pool = super::Xc102Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_102_pool_acquire_empty() {
        let mut pool: super::Xc102Pool<i32> = super::Xc102Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_102_pool_full() {
        let mut pool = super::Xc102Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_102_pool_drain() {
        let mut pool = super::Xc102Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_102_pool_stats() {
        let mut pool = super::Xc102Pool::new(8);
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
    fn xc_102_pool_clear() {
        let mut pool = super::Xc102Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_102_pool_shrink() {
        let mut pool = super::Xc102Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_102_pool_default() {
        let pool: super::Xc102Pool<String> = super::Xc102Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_102_pool_extend() {
        let mut pool = super::Xc102Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_102_pool_retain() {
        let mut pool = super::Xc102Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_102_scheduler_round_robin() {
        let mut sched = super::Xc102Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_102_scheduler_empty() {
        let mut sched = super::Xc102Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_102_scheduler_reset() {
        let mut sched = super::Xc102Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_102_scheduler_add_remove() {
        let mut sched = super::Xc102Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_102_scheduler_targets() {
        let sched = super::Xc102Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_102_hash_empty() {
        assert_eq!(super::xc_102_hash(b""), 5381);
    }

    #[test]
    fn xc_102_hash_data() {
        let h = super::xc_102_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_102_hash(b"hello"), h);
    }

    #[test]
    fn xc_102_reverse_str() {
        assert_eq!(super::xc_102_reverse("abc"), "cba");
        assert_eq!(super::xc_102_reverse(""), "");
    }


    // --- xd_53 deepening tests ---

    #[test]
    fn xd_53_sm_initial_state() {
        let sm = Xd53StateMachine::new();
        assert_eq!(sm.current_state(), Xd53State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_53_sm_valid_idle_to_running() {
        let mut sm = Xd53StateMachine::new();
        assert!(sm.transition(Xd53State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd53State::Running);
    }

    #[test]
    fn xd_53_sm_valid_running_to_paused() {
        let mut sm = Xd53StateMachine::new();
        sm.transition(Xd53State::Running).unwrap();
        assert!(sm.transition(Xd53State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd53State::Paused);
    }

    #[test]
    fn xd_53_sm_valid_running_to_done() {
        let mut sm = Xd53StateMachine::new();
        sm.transition(Xd53State::Running).unwrap();
        assert!(sm.transition(Xd53State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd53State::Done);
    }

    #[test]
    fn xd_53_sm_valid_paused_to_running() {
        let mut sm = Xd53StateMachine::new();
        sm.transition(Xd53State::Running).unwrap();
        sm.transition(Xd53State::Paused).unwrap();
        assert!(sm.transition(Xd53State::Running).is_ok());
    }

    #[test]
    fn xd_53_sm_valid_done_to_idle() {
        let mut sm = Xd53StateMachine::new();
        sm.transition(Xd53State::Running).unwrap();
        sm.transition(Xd53State::Done).unwrap();
        assert!(sm.transition(Xd53State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd53State::Idle);
    }

    #[test]
    fn xd_53_sm_invalid_idle_to_done() {
        let mut sm = Xd53StateMachine::new();
        assert!(sm.transition(Xd53State::Done).is_err());
    }

    #[test]
    fn xd_53_sm_invalid_idle_to_paused() {
        let mut sm = Xd53StateMachine::new();
        assert!(sm.transition(Xd53State::Paused).is_err());
    }

    #[test]
    fn xd_53_sm_history_tracking() {
        let mut sm = Xd53StateMachine::new();
        sm.transition(Xd53State::Running).unwrap();
        sm.transition(Xd53State::Paused).unwrap();
        sm.transition(Xd53State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd53State::Idle);
        assert_eq!(sm.history()[0].to, Xd53State::Running);
        assert_eq!(sm.history()[1].from, Xd53State::Running);
        assert_eq!(sm.history()[2].to, Xd53State::Done);
    }

    #[test]
    fn xd_53_sm_serialize_deserialize() {
        let mut sm = Xd53StateMachine::new();
        sm.transition(Xd53State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd53StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd53State::Running));
    }

    #[test]
    fn xd_53_sm_deserialize_invalid() {
        assert_eq!(Xd53StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_53_sm_reset() {
        let mut sm = Xd53StateMachine::new();
        sm.transition(Xd53State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd53State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_53_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd53EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd53Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_53_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd53EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd53Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd53Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_53_bus_unsubscribe() {
        let mut bus = Xd53EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_53_event_kind_and_payload() {
        let e = Xd53Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd53Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_53_bus_clear_history() {
        let mut bus = Xd53EventBus::new();
        bus.publish(Xd53Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_53_sm_step_counter_increments() {
        let mut sm = Xd53StateMachine::new();
        sm.transition(Xd53State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd53State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #51 --

    #[test]
    fn xf51_trie_insert_search() {
        let mut t = Xf51Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf51_trie_starts_with() {
        let mut t = Xf51Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf51_trie_remove() {
        let mut t = Xf51Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf51_trie_word_count() {
        let mut t = Xf51Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf51_trie_longest_prefix() {
        let mut t = Xf51Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf51_trie_all_words() {
        let mut t = Xf51Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf51_trie_autocomplete() {
        let mut t = Xf51Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf51_trie_empty_search() {
        let t = Xf51Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf51_bloom_add_contains() {
        let mut bf = Xf51BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf51_bloom_probably_absent() {
        let bf = Xf51BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf51_bloom_false_positive_rate() {
        let mut bf = Xf51BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf51_bloom_clear() {
        let mut bf = Xf51BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf51_bloom_union() {
        let mut a = Xf51BloomFilter::xf_new(512, 2);
        let mut b = Xf51BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf51_bloom_intersection_estimate() {
        let mut a = Xf51BloomFilter::xf_new(512, 2);
        let mut b = Xf51BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf51_bloom_union_size_mismatch() {
        let a = Xf51BloomFilter::xf_new(256, 2);
        let b = Xf51BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}
