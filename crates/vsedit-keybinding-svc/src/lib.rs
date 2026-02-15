//! Keybinding resolution service.
//!
//! Resolves key presses to commands based on registered keybinding rules and
//! context key evaluation. Equivalent to VS Code's keybinding resolver.

use vsedit_contextkey::{ContextKeyExpr, IContext};
use vsedit_keybindings::{keybinding_matches, Keybinding};
use vsedit_keycodes::{KeyCode, KeyCodeChord};

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
// KeybindingResolver
// ---------------------------------------------------------------------------

/// Resolves key chord sequences to commands.
///
/// Rules are checked in registration order. When multiple rules match, the
/// one with the highest [`KeybindingWeight`] wins. Among rules with equal
/// weight, the last registered rule wins (later overrides earlier).
///
/// A rule whose `command` starts with `-` acts as a *removal*: it
/// suppresses any earlier rule whose command (without the `-` prefix)
/// matches.
pub struct KeybindingResolver {
    rules: Vec<KeybindingRule>,
}

impl KeybindingResolver {
    /// Create an empty resolver.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Register a keybinding rule.
    pub fn add_rule(&mut self, rule: KeybindingRule) {
        self.rules.push(rule);
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
                    Some(prev) => {
                        if rule.weight >= prev.weight {
                            rule
                        } else {
                            prev
                        }
                    }
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
// Default keybindings
// ---------------------------------------------------------------------------

/// Register the core set of default editor keybindings on a resolver.
pub fn register_default_keybindings(resolver: &mut KeybindingResolver) {
    let defaults: &[(&[KeyCodeChord], &str)] = &[
        // Clipboard
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyC)],
            "editor.action.clipboardCopyAction",
        ),
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyV)],
            "editor.action.clipboardPasteAction",
        ),
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyX)],
            "editor.action.clipboardCutAction",
        ),
        // Undo / Redo
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyZ)],
            "undo",
        ),
        (
            &[KeyCodeChord::new(true, true, false, false, KeyCode::KeyZ)],
            "redo",
        ),
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyY)],
            "redo",
        ),
        // File
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)],
            "workbench.action.files.save",
        ),
        // Quick open / command palette
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyP)],
            "workbench.action.quickOpen",
        ),
        (
            &[KeyCodeChord::new(true, true, false, false, KeyCode::KeyP)],
            "workbench.action.showCommands",
        ),
        // Find / Replace
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyF)],
            "actions.find",
        ),
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyH)],
            "editor.action.startFindReplaceAction",
        ),
        // Go to line
        (
            &[KeyCodeChord::new(true, false, false, false, KeyCode::KeyG)],
            "workbench.action.gotoLine",
        ),
        // F1 → command palette
        (
            &[KeyCodeChord::just(KeyCode::F1)],
            "workbench.action.showCommands",
        ),
    ];

    for (chords, command) in defaults {
        let keybinding = Keybinding {
            parts: chords.to_vec(),
        };
        resolver.add_rule(KeybindingRule {
            keybinding,
            command: command.to_string(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });
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

    // -- Single-chord matching --

    #[test]
    fn single_chord_match() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "workbench.action.files.save".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        let result = resolver.resolve(&ctx, &pressed);
        assert_eq!(
            result,
            ResolveResult::CommandMatch {
                command: "workbench.action.files.save".into(),
                args: None,
            }
        );
    }

    #[test]
    fn single_chord_no_match() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "save".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

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
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::two_chords(
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ),
            command: "editor.action.addCommentLine".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

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
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::two_chords(
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ),
            command: "editor.action.addCommentLine".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

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
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::two_chords(
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyK),
                KeyCodeChord::new(true, false, false, false, KeyCode::KeyC),
            ),
            command: "editor.action.addCommentLine".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

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
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyD,
            )),
            command: "editor.action.deleteLines".into(),
            args: None,
            when: Some(ContextKeyExpr::parse("editorTextFocus").unwrap()),
            weight: KeybindingWeight::EditorCore,
        });

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
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyD,
            )),
            command: "editor.action.deleteLines".into(),
            args: None,
            when: Some(ContextKeyExpr::parse("editorTextFocus").unwrap()),
            weight: KeybindingWeight::EditorCore,
        });

        let ctx = TestContext::new(); // editorTextFocus is NOT set
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyD)];
        assert_eq!(resolver.resolve(&ctx, &pressed), ResolveResult::NoMatch);
    }

    #[test]
    fn when_clause_fallback_to_unconditional() {
        let mut resolver = KeybindingResolver::new();
        // Conditional rule that won't match
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyD,
            )),
            command: "editor.action.deleteLines".into(),
            args: None,
            when: Some(ContextKeyExpr::parse("editorTextFocus").unwrap()),
            weight: KeybindingWeight::EditorCore,
        });
        // Unconditional fallback
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyD,
            )),
            command: "fallback.action".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

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
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "low.priority".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "high.priority".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::ExternalExtension,
        });

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
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "first".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "second".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

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
        // Normal binding
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "workbench.action.files.save".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });
        // Negation rule
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "-workbench.action.files.save".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        assert_eq!(resolver.resolve(&ctx, &pressed), ResolveResult::NoMatch);
    }

    #[test]
    fn negation_only_removes_specific_command() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "workbench.action.files.save".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "other.save".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });
        // Only negate the first command
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "-workbench.action.files.save".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

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
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::just(KeyCode::F5)),
            command: "workbench.action.debug.start".into(),
            args: Some(vec!["noDebug".into()]),
            when: None,
            weight: KeybindingWeight::EditorCore,
        });

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
        assert_eq!(bindings.len(), 2); // Ctrl+Shift+Z and Ctrl+Y
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

        // Ctrl+C → copy
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyC)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "editor.action.clipboardCopyAction".into(),
                args: None,
            }
        );

        // Ctrl+Z → undo
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyZ)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "undo".into(),
                args: None,
            }
        );

        // Ctrl+Shift+P → show commands
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
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "workbench.action.files.save".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
        });
        // Negation only when inDebugMode
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(KeyCodeChord::new(
                true, false, false, false, KeyCode::KeyS,
            )),
            command: "-workbench.action.files.save".into(),
            args: None,
            when: Some(ContextKeyExpr::parse("inDebugMode").unwrap()),
            weight: KeybindingWeight::EditorCore,
        });

        // Without debug mode: binding works
        let ctx = TestContext::new();
        let pressed = [KeyCodeChord::new(true, false, false, false, KeyCode::KeyS)];
        assert_eq!(
            resolver.resolve(&ctx, &pressed),
            ResolveResult::CommandMatch {
                command: "workbench.action.files.save".into(),
                args: None,
            }
        );

        // With debug mode: binding is negated
        let mut ctx = TestContext::new();
        ctx.set("inDebugMode", ContextKeyValue::Bool(true));
        assert_eq!(resolver.resolve(&ctx, &pressed), ResolveResult::NoMatch);
    }
}
