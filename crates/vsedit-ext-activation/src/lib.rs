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

// ---------------------------------------------------------------------------
// Additional activation utilities
// ---------------------------------------------------------------------------

impl std::fmt::Display for ActivationEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Star => write!(f, "*"),
            Self::OnLanguage(l) => write!(f, "onLanguage:{l}"),
            Self::OnCommand(c) => write!(f, "onCommand:{c}"),
            Self::OnFileSystem(s) => write!(f, "onFileSystem:{s}"),
            Self::OnView(v) => write!(f, "onView:{v}"),
            Self::OnUri(u) => write!(f, "onUri:{u}"),
            Self::WorkspaceContains(g) => write!(f, "workspaceContains:{g}"),
            Self::OnDebug => write!(f, "onDebug"),
            Self::OnAuthenticationRequest(p) => write!(f, "onAuthenticationRequest:{p}"),
            Self::OnStartupFinished => write!(f, "onStartupFinished"),
        }
    }
}

/// Serialize an activation event back to the string form used in package.json.
pub fn activation_event_to_string(event: &ActivationEvent) -> String {
    format!("{event}")
}

/// Parse a list of activation event strings, skipping any unrecognized ones.
pub fn parse_activation_events(events: &[&str]) -> Vec<ActivationEvent> {
    events.iter().filter_map(|e| parse_activation_event(e)).collect()
}

/// Validate that an activation event string is well-formed.
pub fn validate_activation_event(event: &str) -> Result<ActivationEvent, String> {
    parse_activation_event(event).ok_or_else(|| format!("unknown activation event: {event}"))
}

/// An activation dependency graph: extensions can depend on other extensions being activated first.
#[derive(Debug, Clone, Default)]
pub struct ActivationDependencyGraph {
    /// Extension ID → set of extension IDs it depends on.
    deps: HashMap<String, HashSet<String>>,
}

impl ActivationDependencyGraph {
    pub fn new() -> Self {
        Self { deps: HashMap::new() }
    }

    /// Add a dependency: `ext_id` depends on `depends_on` being activated first.
    pub fn add_dependency(&mut self, ext_id: impl Into<String>, depends_on: impl Into<String>) {
        self.deps.entry(ext_id.into()).or_default().insert(depends_on.into());
    }

    /// Get the set of dependencies for an extension.
    pub fn dependencies_of(&self, ext_id: &str) -> HashSet<String> {
        self.deps.get(ext_id).cloned().unwrap_or_default()
    }

    /// Check whether all dependencies of `ext_id` are in the `activated` set.
    pub fn can_activate(&self, ext_id: &str, activated: &HashSet<String>) -> bool {
        match self.deps.get(ext_id) {
            None => true,
            Some(deps) => deps.iter().all(|d| activated.contains(d)),
        }
    }

    /// Return all extensions that have no unsatisfied dependencies given the activated set.
    pub fn ready_to_activate(&self, all_ids: &[String], activated: &HashSet<String>) -> Vec<String> {
        all_ids
            .iter()
            .filter(|id| !activated.contains(id.as_str()) && self.can_activate(id, activated))
            .cloned()
            .collect()
    }

    /// Produce a topological ordering of all extensions, or return an error if there is a cycle.
    pub fn topological_sort(&self, all_ids: &[String]) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for id in all_ids {
            in_degree.entry(id.clone()).or_insert(0);
        }
        for (id, deps) in &self.deps {
            for dep in deps {
                if all_ids.contains(dep) {
                    *in_degree.entry(id.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(id, _)| id.clone())
            .collect();
        queue.make_contiguous().sort();

        let mut result = Vec::new();
        while let Some(id) = queue.pop_front() {
            result.push(id.clone());
            for (ext_id, deps) in &self.deps {
                if deps.contains(&id) {
                    if let Some(deg) = in_degree.get_mut(ext_id) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(ext_id.clone());
                        }
                    }
                }
            }
        }

        if result.len() == all_ids.len() {
            Ok(result)
        } else {
            Err("cycle detected in activation dependencies".to_string())
        }
    }

    /// Number of registered dependency relationships.
    pub fn total_edges(&self) -> usize {
        self.deps.values().map(|s| s.len()).sum()
    }
}

impl ActivationEventMatcher {
    /// Register that a language file was opened.
    pub fn open_language(&mut self, lang: impl Into<String>) {
        self.open_languages.insert(lang.into());
    }

    /// Register that a URI scheme is available.
    pub fn add_scheme(&mut self, scheme: impl Into<String>) {
        self.open_schemes.insert(scheme.into());
    }

    /// Register that a file exists in the workspace.
    pub fn add_workspace_file(&mut self, file: impl Into<String>) {
        self.workspace_files.insert(file.into());
    }

    /// Mark startup as finished.
    pub fn finish_startup(&mut self) {
        self.startup_finished = true;
    }

    /// Collect all activation events that currently match.
    pub fn matching_events(&self, events: &[ActivationEvent]) -> Vec<ActivationEvent> {
        events.iter().filter(|e| self.should_activate(e)).cloned().collect()
    }
}

impl ExtensionActivationQueue {
    /// Register multiple extensions at once.
    pub fn register_many(&mut self, entries: Vec<(String, Vec<ActivationEvent>)>) {
        for (id, events) in entries {
            self.register(id, events);
        }
    }

    /// Drain all pending activations, returning them in order.
    pub fn drain_pending(&mut self) -> Vec<String> {
        let mut result = Vec::new();
        while let Some(id) = self.pop_pending() {
            result.push(id);
        }
        result
    }

    /// Number of activated extensions.
    pub fn activated_count(&self) -> usize {
        self.activated.len()
    }

    /// Number of registered extensions.
    pub fn registered_count(&self) -> usize {
        self.registry.len()
    }

    /// Get the activation events registered for an extension.
    pub fn events_for(&self, ext_id: &str) -> Option<&Vec<ActivationEvent>> {
        self.registry.get(ext_id)
    }

    /// Reset the queue: clear activated and pending, but keep the registry.
    pub fn reset(&mut self) {
        self.activated.clear();
        self.pending.clear();
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

    #[test]
    fn activation_event_display_roundtrip() {
        let events = vec![
            ActivationEvent::Star,
            ActivationEvent::OnLanguage("rust".into()),
            ActivationEvent::OnCommand("save".into()),
            ActivationEvent::OnFileSystem("ftp".into()),
            ActivationEvent::OnView("explorer".into()),
            ActivationEvent::OnUri("vscode".into()),
            ActivationEvent::WorkspaceContains("*.rs".into()),
            ActivationEvent::OnDebug,
            ActivationEvent::OnAuthenticationRequest("github".into()),
            ActivationEvent::OnStartupFinished,
        ];
        for event in &events {
            let s = activation_event_to_string(event);
            let parsed = parse_activation_event(&s).unwrap();
            assert_eq!(&parsed, event);
        }
    }

    #[test]
    fn parse_activation_events_batch() {
        let input = vec!["onLanguage:rust", "bad", "onCommand:run", "*"];
        let result = parse_activation_events(&input);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn validate_activation_event_ok() {
        assert!(validate_activation_event("onLanguage:rust").is_ok());
    }

    #[test]
    fn validate_activation_event_err() {
        let err = validate_activation_event("nonsense").unwrap_err();
        assert!(err.contains("unknown"));
    }

    #[test]
    fn dependency_graph_basic() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("ext-b", "ext-a");
        let activated: HashSet<String> = HashSet::new();
        assert!(!g.can_activate("ext-b", &activated));
        let mut activated2: HashSet<String> = HashSet::new();
        activated2.insert("ext-a".into());
        assert!(g.can_activate("ext-b", &activated2));
    }

    #[test]
    fn dependency_graph_topological_sort() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("c", "b");
        g.add_dependency("b", "a");
        let ids = vec!["a".into(), "b".into(), "c".into()];
        let sorted = g.topological_sort(&ids).unwrap();
        assert_eq!(sorted, vec!["a", "b", "c"]);
    }

    #[test]
    fn dependency_graph_ready_to_activate() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("b", "a");
        g.add_dependency("c", "a");
        let ids = vec!["a".into(), "b".into(), "c".into()];
        let activated = HashSet::new();
        let ready = g.ready_to_activate(&ids, &activated);
        assert_eq!(ready, vec!["a".to_string()]);
    }

    #[test]
    fn dependency_graph_total_edges() {
        let mut g = ActivationDependencyGraph::new();
        g.add_dependency("b", "a");
        g.add_dependency("c", "a");
        g.add_dependency("c", "b");
        assert_eq!(g.total_edges(), 3);
    }

    #[test]
    fn matcher_convenience_methods() {
        let mut m = ActivationEventMatcher::new();
        m.open_language("python");
        m.add_scheme("file");
        m.add_workspace_file("Makefile");
        m.finish_startup();
        assert!(m.should_activate(&ActivationEvent::OnLanguage("python".into())));
        assert!(m.should_activate(&ActivationEvent::OnFileSystem("file".into())));
        assert!(m.should_activate(&ActivationEvent::WorkspaceContains("Makefile".into())));
        assert!(m.should_activate(&ActivationEvent::OnStartupFinished));
    }

    #[test]
    fn matcher_matching_events_filter() {
        let mut m = ActivationEventMatcher::new();
        m.open_language("rust");
        let events = vec![
            ActivationEvent::OnLanguage("rust".into()),
            ActivationEvent::OnLanguage("python".into()),
            ActivationEvent::Star,
        ];
        let matched = m.matching_events(&events);
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn queue_register_many() {
        let mut q = ExtensionActivationQueue::new();
        q.register_many(vec![
            ("a".into(), vec![ActivationEvent::Star]),
            ("b".into(), vec![ActivationEvent::OnDebug]),
        ]);
        assert_eq!(q.registered_count(), 2);
    }

    #[test]
    fn queue_drain_pending() {
        let mut q = ExtensionActivationQueue::new();
        q.register("a".into(), vec![ActivationEvent::Star]);
        q.register("b".into(), vec![ActivationEvent::Star]);
        let m = ActivationEventMatcher::new();
        q.evaluate(&m);
        let drained = q.drain_pending();
        assert_eq!(drained.len(), 2);
        assert_eq!(q.activated_count(), 2);
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn queue_reset() {
        let mut q = ExtensionActivationQueue::new();
        q.register("a".into(), vec![ActivationEvent::Star]);
        let m = ActivationEventMatcher::new();
        q.evaluate(&m);
        q.drain_pending();
        assert_eq!(q.activated_count(), 1);
        q.reset();
        assert_eq!(q.activated_count(), 0);
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn queue_events_for() {
        let mut q = ExtensionActivationQueue::new();
        q.register("x".into(), vec![ActivationEvent::OnDebug]);
        let events = q.events_for("x").unwrap();
        assert_eq!(events.len(), 1);
        assert!(q.events_for("y").is_none());
    }
}
