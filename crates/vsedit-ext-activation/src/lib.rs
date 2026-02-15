//! Extension activation event handling.

/// Activation events that trigger extension loading.
#[derive(Debug, Clone, PartialEq, Eq)]
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
}
