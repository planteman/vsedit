//! Extension activation event handling.

use std::collections::{HashMap, HashSet, VecDeque};

/// Activation events that trigger extension loading.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActivationEvent {
    /// Activate on startup (always).
    Star,
    /// Activate on language.
    OnLanguage(String),
    /// Activate on command.
    OnCommand(String),
    /// Activate when a file with pattern is opened.
    OnFileSystem(String),
    /// Activate on view.
    OnView(String),
    /// Activate on URI scheme.
    OnUri(String),
    /// Activate on workspace contains.
    WorkspaceContains(String),
    /// Activate on debug.
    OnDebug,
    /// Activate on authentication.
    OnAuthenticationRequest(String),
    /// Activate on start finished.
    OnStartupFinished,
}

/// Parse activation event strings from package.json.
pub fn parse_activation_event(event: &str) -> Option<ActivationEvent> {
    if event == "*" { return Some(ActivationEvent::Star); }
    if event == "onStartupFinished" { return Some(ActivationEvent::OnStartupFinished); }
    if event == "onDebug" { return Some(ActivationEvent::OnDebug); }
    if let Some(lang) = event.strip_prefix("onLanguage:") {
        return Some(ActivationEvent::OnLanguage(lang.to_string()));
    }
    if let Some(cmd) = event.strip_prefix("onCommand:") {
        return Some(ActivationEvent::OnCommand(cmd.to_string()));
    }
    if let Some(fs) = event.strip_prefix("onFileSystem:") {
        return Some(ActivationEvent::OnFileSystem(fs.to_string()));
    }
    if let Some(view) = event.strip_prefix("onView:") {
        return Some(ActivationEvent::OnView(view.to_string()));
    }
    if let Some(uri) = event.strip_prefix("onUri:") {
        return Some(ActivationEvent::OnUri(uri.to_string()));
    }
    if let Some(glob) = event.strip_prefix("workspaceContains:") {
        return Some(ActivationEvent::WorkspaceContains(glob.to_string()));
    }
    if let Some(provider) = event.strip_prefix("onAuthenticationRequest:") {
        return Some(ActivationEvent::OnAuthenticationRequest(provider.to_string()));
    }
    None
}

/// Check if an activation event matches a trigger.
pub fn matches_trigger(event: &ActivationEvent, trigger: &str, value: &str) -> bool {
    match (event, trigger) {
        (ActivationEvent::Star, _) => true,
        (ActivationEvent::OnLanguage(lang), "onLanguage") => lang == value,
        (ActivationEvent::OnCommand(cmd), "onCommand") => cmd == value,
        (ActivationEvent::OnView(view), "onView") => view == value,
        (ActivationEvent::OnStartupFinished, "onStartupFinished") => true,
        _ => false,
    }
}

/// Checks whether a set of conditions satisfy an activation event.
pub struct ActivationEventMatcher {
    /// Currently open languages.
    pub open_languages: HashSet<String>,
    /// Whether startup has finished.
    pub startup_finished: bool,
    /// Currently open file URI schemes.
    pub open_schemes: HashSet<String>,
    /// Files present in the workspace root.
    pub workspace_files: HashSet<String>,
}

impl ActivationEventMatcher {
    pub fn new() -> Self {
        Self {
            open_languages: HashSet::new(),
            startup_finished: false,
            open_schemes: HashSet::new(),
            workspace_files: HashSet::new(),
        }
    }

    /// Check whether the given activation event's conditions are currently met.
    pub fn should_activate(&self, event: &ActivationEvent) -> bool {
        match event {
            ActivationEvent::Star => true,
            ActivationEvent::OnLanguage(lang) => self.open_languages.contains(lang),
            ActivationEvent::OnStartupFinished => self.startup_finished,
            ActivationEvent::OnFileSystem(scheme) => self.open_schemes.contains(scheme),
            ActivationEvent::WorkspaceContains(pattern) => {
                self.workspace_files.iter().any(|f| f.contains(pattern.as_str()))
            }
            ActivationEvent::OnDebug => false,
            ActivationEvent::OnCommand(_) => false,
            ActivationEvent::OnView(_) => false,
            ActivationEvent::OnUri(_) => false,
            ActivationEvent::OnAuthenticationRequest(_) => false,
        }
    }
}

impl Default for ActivationEventMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks pending extension activations.
pub struct ExtensionActivationQueue {
    /// Extension ID → list of activation events.
    registry: HashMap<String, Vec<ActivationEvent>>,
    /// Extensions already activated.
    activated: HashSet<String>,
    /// Queue of extension IDs pending activation.
    pending: VecDeque<String>,
}

impl ExtensionActivationQueue {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
            activated: HashSet::new(),
            pending: VecDeque::new(),
        }
    }

    /// Register an extension with its activation events.
    pub fn register(&mut self, extension_id: String, events: Vec<ActivationEvent>) {
        self.registry.insert(extension_id, events);
    }

    /// Evaluate all registered extensions against the current matcher state.
    /// Returns newly queued extension IDs.
    pub fn evaluate(&mut self, matcher: &ActivationEventMatcher) -> Vec<String> {
        let mut newly_queued = Vec::new();
        for (ext_id, events) in &self.registry {
            if self.activated.contains(ext_id) {
                continue;
            }
            if events.iter().any(|e| matcher.should_activate(e)) {
                if !self.pending.contains(ext_id) {
                    self.pending.push_back(ext_id.clone());
                    newly_queued.push(ext_id.clone());
                }
            }
        }
        newly_queued
    }

    /// Pop the next extension to activate.
    pub fn pop_pending(&mut self) -> Option<String> {
        if let Some(ext_id) = self.pending.pop_front() {
            self.activated.insert(ext_id.clone());
            Some(ext_id)
        } else {
            None
        }
    }

    pub fn is_activated(&self, extension_id: &str) -> bool {
        self.activated.contains(extension_id)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for ExtensionActivationQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_events() {
        assert_eq!(parse_activation_event("*"), Some(ActivationEvent::Star));
        assert_eq!(
            parse_activation_event("onLanguage:rust"),
            Some(ActivationEvent::OnLanguage("rust".into()))
        );
        assert_eq!(
            parse_activation_event("onCommand:workbench.action.files.save"),
            Some(ActivationEvent::OnCommand("workbench.action.files.save".into()))
        );
        assert_eq!(parse_activation_event("onStartupFinished"), Some(ActivationEvent::OnStartupFinished));
    }

    #[test]
    fn trigger_matching() {
        let event = ActivationEvent::OnLanguage("rust".into());
        assert!(matches_trigger(&event, "onLanguage", "rust"));
        assert!(!matches_trigger(&event, "onLanguage", "python"));
    }

    #[test]
    fn star_matches_everything() {
        let event = ActivationEvent::Star;
        assert!(matches_trigger(&event, "onLanguage", "anything"));
    }

    #[test]
    fn parse_all_event_types() {
        assert_eq!(parse_activation_event("onDebug"), Some(ActivationEvent::OnDebug));
        assert_eq!(
            parse_activation_event("onFileSystem:ftp"),
            Some(ActivationEvent::OnFileSystem("ftp".into()))
        );
        assert_eq!(
            parse_activation_event("onView:explorer"),
            Some(ActivationEvent::OnView("explorer".into()))
        );
        assert_eq!(
            parse_activation_event("onUri:vscode"),
            Some(ActivationEvent::OnUri("vscode".into()))
        );
        assert_eq!(
            parse_activation_event("workspaceContains:*.rs"),
            Some(ActivationEvent::WorkspaceContains("*.rs".into()))
        );
        assert_eq!(
            parse_activation_event("onAuthenticationRequest:github"),
            Some(ActivationEvent::OnAuthenticationRequest("github".into()))
        );
        assert_eq!(parse_activation_event("unknownEvent"), None);
    }

    #[test]
    fn matcher_language() {
        let mut m = ActivationEventMatcher::new();
        let event = ActivationEvent::OnLanguage("rust".into());
        assert!(!m.should_activate(&event));
        m.open_languages.insert("rust".into());
        assert!(m.should_activate(&event));
    }

    #[test]
    fn matcher_startup_finished() {
        let mut m = ActivationEventMatcher::new();
        let event = ActivationEvent::OnStartupFinished;
        assert!(!m.should_activate(&event));
        m.startup_finished = true;
        assert!(m.should_activate(&event));
    }

    #[test]
    fn activation_queue_basic() {
        let mut queue = ExtensionActivationQueue::new();
        queue.register("ext-a".into(), vec![ActivationEvent::Star]);
        queue.register(
            "ext-b".into(),
            vec![ActivationEvent::OnLanguage("rust".into())],
        );

        let matcher = ActivationEventMatcher::new();
        let queued = queue.evaluate(&matcher);
        // Star should activate immediately
        assert!(queued.contains(&"ext-a".to_string()));
        assert!(!queued.contains(&"ext-b".to_string()));

        let popped = queue.pop_pending().unwrap();
        assert_eq!(popped, "ext-a");
        assert!(queue.is_activated("ext-a"));
        assert!(!queue.is_activated("ext-b"));
    }

    #[test]
    fn activation_queue_no_double_activation() {
        let mut queue = ExtensionActivationQueue::new();
        queue.register("ext-a".into(), vec![ActivationEvent::Star]);

        let matcher = ActivationEventMatcher::new();
        queue.evaluate(&matcher);
        queue.pop_pending();

        // Re-evaluate should not re-queue
        let queued = queue.evaluate(&matcher);
        assert!(queued.is_empty());
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn matcher_workspace_contains() {
        let mut m = ActivationEventMatcher::new();
        m.workspace_files.insert("Cargo.toml".into());
        m.workspace_files.insert("src/main.rs".into());

        let event = ActivationEvent::WorkspaceContains("Cargo".into());
        assert!(m.should_activate(&event));

        let no_match = ActivationEvent::WorkspaceContains("package.json".into());
        assert!(!m.should_activate(&no_match));
    }
}
