//! AI chat service.

use std::fmt;

/// Errors that can occur within the chat service.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatError {
    /// A participant with this ID already exists.
    DuplicateParticipant(String),
    /// A variable with this name already exists.
    DuplicateVariable(String),
    /// The referenced participant was not found.
    ParticipantNotFound(String),
    /// A required field was empty or invalid.
    ValidationError(String),
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::DuplicateParticipant(id) => {
                write!(f, "participant already registered: {id}")
            }
            ChatError::DuplicateVariable(name) => {
                write!(f, "variable already registered: {name}")
            }
            ChatError::ParticipantNotFound(id) => {
                write!(f, "participant not found: {id}")
            }
            ChatError::ValidationError(msg) => {
                write!(f, "validation error: {msg}")
            }
        }
    }
}

impl std::error::Error for ChatError {}

/// A participant that can respond in the chat.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatParticipant {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

impl ChatParticipant {
    /// Returns the display name of the participant.
    pub fn display_name(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for ChatParticipant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.id)
    }
}

/// Builder for constructing a `ChatParticipant` with validation.
#[derive(Debug, Default)]
pub struct ChatParticipantBuilder {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    is_default: bool,
}

impl ChatParticipantBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn is_default(mut self, val: bool) -> Self {
        self.is_default = val;
        self
    }

    /// Build the participant, returning a `ChatError::ValidationError` if
    /// required fields are missing or empty.
    pub fn build(self) -> Result<ChatParticipant, ChatError> {
        let id = self
            .id
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ChatError::ValidationError("id is required".into()))?;
        let name = self
            .name
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ChatError::ValidationError("name is required".into()))?;
        Ok(ChatParticipant {
            id,
            name,
            description: self.description,
            is_default: self.is_default,
        })
    }
}

/// A slash command available within a chat participant.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatSlashCommand {
    pub name: String,
    pub description: String,
    pub participant_id: String,
}

impl fmt::Display for ChatSlashCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{} ({})", self.name, self.participant_id)
    }
}

/// A variable that can be referenced in chat messages.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatVariable {
    pub name: String,
    pub description: String,
    pub value: String,
}

impl fmt::Display for ChatVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${{{}}}", self.name)
    }
}

/// Service for managing chat participants and commands.
pub struct ChatWorkbenchService {
    participants: Vec<ChatParticipant>,
    commands: Vec<ChatSlashCommand>,
    variables: Vec<ChatVariable>,
}

impl ChatWorkbenchService {
    pub fn new() -> Self {
        Self {
            participants: Vec::new(),
            commands: Vec::new(),
            variables: Vec::new(),
        }
    }

    pub fn register_participant(&mut self, participant: ChatParticipant) {
        self.participants.push(participant);
    }

    pub fn register_command(&mut self, command: ChatSlashCommand) {
        self.commands.push(command);
    }

    pub fn get_participant(&self, id: &str) -> Option<&ChatParticipant> {
        self.participants.iter().find(|p| p.id == id)
    }

    pub fn get_default_participant(&self) -> Option<&ChatParticipant> {
        self.participants.iter().find(|p| p.is_default)
    }

    pub fn get_commands_for(&self, participant_id: &str) -> Vec<&ChatSlashCommand> {
        self.commands
            .iter()
            .filter(|c| c.participant_id == participant_id)
            .collect()
    }

    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    pub fn register_variable(&mut self, variable: ChatVariable) {
        self.variables.push(variable);
    }

    pub fn get_variable(&self, name: &str) -> Option<&ChatVariable> {
        self.variables.iter().find(|v| v.name == name)
    }

    pub fn get_all_variables(&self) -> &[ChatVariable] {
        &self.variables
    }

    pub fn unregister_participant(&mut self, id: &str) -> bool {
        let before = self.participants.len();
        self.participants.retain(|p| p.id != id);
        self.participants.len() < before
    }

    pub fn unregister_command(&mut self, name: &str, participant_id: &str) -> bool {
        let before = self.commands.len();
        self.commands
            .retain(|c| !(c.name == name && c.participant_id == participant_id));
        self.commands.len() < before
    }

    pub fn get_all_commands(&self) -> &[ChatSlashCommand] {
        &self.commands
    }

    pub fn find_command(&self, name: &str) -> Option<&ChatSlashCommand> {
        self.commands.iter().find(|c| c.name == name)
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Register a participant, rejecting duplicates.
    pub fn try_register_participant(
        &mut self,
        participant: ChatParticipant,
    ) -> Result<(), ChatError> {
        if self.participants.iter().any(|p| p.id == participant.id) {
            return Err(ChatError::DuplicateParticipant(participant.id));
        }
        self.participants.push(participant);
        Ok(())
    }

    /// Register a variable, rejecting duplicates.
    pub fn try_register_variable(
        &mut self,
        variable: ChatVariable,
    ) -> Result<(), ChatError> {
        if self.variables.iter().any(|v| v.name == variable.name) {
            return Err(ChatError::DuplicateVariable(variable.name));
        }
        self.variables.push(variable);
        Ok(())
    }

    /// Get all participant IDs as a collected vector.
    pub fn participant_ids(&self) -> Vec<&str> {
        self.participants.iter().map(|p| p.id.as_str()).collect()
    }

    /// Resolve `${variable}` placeholders in a template string using
    /// registered variables. Unknown variables are left as-is.
    pub fn resolve_variables(&self, template: &str) -> String {
        let mut result = template.to_string();
        for var in &self.variables {
            let placeholder = format!("${{{}}}", var.name);
            result = result.replace(&placeholder, &var.value);
        }
        result
    }

    /// Returns true when `name` looks like a valid slash-command reference
    /// (non-empty, ASCII alphanumeric or hyphen, no leading hyphen).
    pub fn is_valid_command_name(name: &str) -> bool {
        !name.is_empty()
            && !name.starts_with('-')
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    }

    /// Unregister a variable by name, returning whether it was present.
    pub fn unregister_variable(&mut self, name: &str) -> bool {
        let before = self.variables.len();
        self.variables.retain(|v| v.name != name);
        self.variables.len() < before
    }

    /// Clear every registration (participants, commands, variables).
    pub fn clear(&mut self) {
        self.participants.clear();
        self.commands.clear();
        self.variables.clear();
    }
}

impl Default for ChatWorkbenchService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_participant(id: &str, is_default: bool) -> ChatParticipant {
        ChatParticipant {
            id: id.to_string(),
            name: format!("Participant {id}"),
            description: None,
            is_default,
        }
    }

    fn make_command(name: &str, participant_id: &str) -> ChatSlashCommand {
        ChatSlashCommand {
            name: name.to_string(),
            description: format!("Command {name}"),
            participant_id: participant_id.to_string(),
        }
    }

    #[test]
    fn register_and_query_participants() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_participant(make_participant("workspace", false));
        assert_eq!(svc.participant_count(), 2);
        assert!(svc.get_participant("copilot").is_some());
        assert!(svc.get_participant("missing").is_none());
    }

    #[test]
    fn get_default_participant() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_participant(make_participant("workspace", false));
        let default = svc.get_default_participant().unwrap();
        assert_eq!(default.id, "copilot");
    }

    #[test]
    fn commands_for_participant() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_command(make_command("explain", "copilot"));
        svc.register_command(make_command("fix", "copilot"));
        svc.register_command(make_command("test", "workspace"));
        assert_eq!(svc.get_commands_for("copilot").len(), 2);
        assert_eq!(svc.get_commands_for("workspace").len(), 1);
    }

    fn make_variable(name: &str, value: &str) -> ChatVariable {
        ChatVariable {
            name: name.to_string(),
            description: format!("Variable {name}"),
            value: value.to_string(),
        }
    }

    #[test]
    fn register_and_query_variables() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_variable(make_variable("file", "main.rs"));
        svc.register_variable(make_variable("selection", "fn main()"));
        assert_eq!(svc.get_all_variables().len(), 2);
        let v = svc.get_variable("file").unwrap();
        assert_eq!(v.value, "main.rs");
        assert!(svc.get_variable("missing").is_none());
    }

    #[test]
    fn unregister_participant_removes_entry() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_participant(make_participant("workspace", false));
        assert!(svc.unregister_participant("copilot"));
        assert_eq!(svc.participant_count(), 1);
        assert!(svc.get_participant("copilot").is_none());
        assert!(!svc.unregister_participant("copilot"));
    }

    #[test]
    fn unregister_command_removes_entry() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_command(make_command("explain", "copilot"));
        svc.register_command(make_command("fix", "copilot"));
        assert!(svc.unregister_command("explain", "copilot"));
        assert_eq!(svc.command_count(), 1);
        assert!(!svc.unregister_command("explain", "copilot"));
    }

    #[test]
    fn find_command_and_get_all() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_command(make_command("explain", "copilot"));
        svc.register_command(make_command("fix", "copilot"));
        assert_eq!(svc.get_all_commands().len(), 2);
        let cmd = svc.find_command("fix").unwrap();
        assert_eq!(cmd.participant_id, "copilot");
        assert!(svc.find_command("missing").is_none());
    }

    #[test]
    fn participant_display_name() {
        let p = make_participant("copilot", true);
        assert_eq!(p.display_name(), "Participant copilot");
    }

    #[test]
    fn participant_display_trait() {
        let p = make_participant("copilot", true);
        assert_eq!(format!("{p}"), "Participant copilot (copilot)");
    }

    #[test]
    fn slash_command_display() {
        let cmd = make_command("explain", "copilot");
        assert_eq!(format!("{cmd}"), "/explain (copilot)");
    }

    #[test]
    fn variable_display() {
        let v = make_variable("file", "main.rs");
        assert_eq!(format!("{v}"), "${file}");
    }

    #[test]
    fn builder_creates_participant() {
        let p = ChatParticipantBuilder::new()
            .id("copilot")
            .name("Copilot")
            .description("AI pair programmer")
            .is_default(true)
            .build()
            .unwrap();
        assert_eq!(p.id, "copilot");
        assert_eq!(p.name, "Copilot");
        assert_eq!(p.description.as_deref(), Some("AI pair programmer"));
        assert!(p.is_default);
    }

    #[test]
    fn builder_rejects_missing_id() {
        let res = ChatParticipantBuilder::new().name("Copilot").build();
        assert_eq!(
            res,
            Err(ChatError::ValidationError("id is required".into()))
        );
    }

    #[test]
    fn builder_rejects_empty_name() {
        let res = ChatParticipantBuilder::new().id("x").name("").build();
        assert_eq!(
            res,
            Err(ChatError::ValidationError("name is required".into()))
        );
    }

    #[test]
    fn try_register_duplicate_participant() {
        let mut svc = ChatWorkbenchService::new();
        svc.try_register_participant(make_participant("copilot", true))
            .unwrap();
        let err = svc
            .try_register_participant(make_participant("copilot", false))
            .unwrap_err();
        assert_eq!(err, ChatError::DuplicateParticipant("copilot".into()));
    }

    #[test]
    fn try_register_duplicate_variable() {
        let mut svc = ChatWorkbenchService::new();
        svc.try_register_variable(make_variable("file", "a.rs"))
            .unwrap();
        let err = svc
            .try_register_variable(make_variable("file", "b.rs"))
            .unwrap_err();
        assert_eq!(err, ChatError::DuplicateVariable("file".into()));
    }

    #[test]
    fn resolve_variables_in_template() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_variable(make_variable("file", "main.rs"));
        svc.register_variable(make_variable("lang", "Rust"));
        let result = svc.resolve_variables("Open ${file} in ${lang}, ${unknown} stays");
        assert_eq!(result, "Open main.rs in Rust, ${unknown} stays");
    }

    #[test]
    fn is_valid_command_name_checks() {
        assert!(ChatWorkbenchService::is_valid_command_name("explain"));
        assert!(ChatWorkbenchService::is_valid_command_name("my-cmd"));
        assert!(!ChatWorkbenchService::is_valid_command_name(""));
        assert!(!ChatWorkbenchService::is_valid_command_name("-bad"));
        assert!(!ChatWorkbenchService::is_valid_command_name("no spaces"));
    }

    #[test]
    fn participant_ids_returns_all() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("a", false));
        svc.register_participant(make_participant("b", false));
        let ids = svc.participant_ids();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn clear_removes_everything() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_command(make_command("fix", "copilot"));
        svc.register_variable(make_variable("file", "x.rs"));
        svc.clear();
        assert_eq!(svc.participant_count(), 0);
        assert_eq!(svc.command_count(), 0);
        assert_eq!(svc.get_all_variables().len(), 0);
    }

    #[test]
    fn unregister_variable() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_variable(make_variable("file", "a.rs"));
        assert!(svc.unregister_variable("file"));
        assert!(!svc.unregister_variable("file"));
        assert_eq!(svc.get_all_variables().len(), 0);
    }

    #[test]
    fn chat_error_display() {
        let e = ChatError::ParticipantNotFound("abc".into());
        assert_eq!(e.to_string(), "participant not found: abc");
        let e2 = ChatError::ValidationError("bad".into());
        assert_eq!(e2.to_string(), "validation error: bad");
    }

    #[test]
    fn participant_equality() {
        let a = make_participant("copilot", true);
        let b = make_participant("copilot", true);
        assert_eq!(a, b);
        let c = make_participant("other", true);
        assert_ne!(a, c);
    }
}
