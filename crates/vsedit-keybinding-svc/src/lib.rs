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

use vsedit_contextkey::{ContextKeyExpr, IContext};
use vsedit_keybindings::{keybinding_matches, parse_keybinding, Keybinding};
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
    ];

    for rule in defaults {
        resolver.add_rule(rule);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
}
