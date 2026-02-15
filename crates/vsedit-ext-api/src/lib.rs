//! Extension API surface (vscode.* namespace bridging).

/// VS Code API namespace identifiers.
pub mod namespaces {
    pub const COMMANDS: &str = "commands";
    pub const WINDOW: &str = "window";
    pub const WORKSPACE: &str = "workspace";
    pub const LANGUAGES: &str = "languages";
    pub const DEBUG: &str = "debug";
    pub const EXTENSIONS: &str = "extensions";
    pub const ENV: &str = "env";
    pub const TASKS: &str = "tasks";
    pub const SCM: &str = "scm";
    pub const COMMENTS: &str = "comments";
    pub const AUTHENTICATION: &str = "authentication";
    pub const NOTEBOOKS: &str = "notebooks";
    pub const TESTS: &str = "tests";
    pub const CHAT: &str = "chat";
    pub const LM: &str = "lm";
}

/// All supported namespaces.
pub fn all_namespaces() -> Vec<&'static str> {
    vec![
        namespaces::COMMANDS, namespaces::WINDOW, namespaces::WORKSPACE,
        namespaces::LANGUAGES, namespaces::DEBUG, namespaces::EXTENSIONS,
        namespaces::ENV, namespaces::TASKS, namespaces::SCM,
        namespaces::COMMENTS, namespaces::AUTHENTICATION,
        namespaces::NOTEBOOKS, namespaces::TESTS, namespaces::CHAT,
        namespaces::LM,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_count() {
        assert_eq!(all_namespaces().len(), 15);
    }
}
