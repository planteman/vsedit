//! AI chat service.

/// A participant that can respond in the chat.
#[derive(Debug, Clone)]
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

/// A slash command available within a chat participant.
#[derive(Debug, Clone)]
pub struct ChatSlashCommand {
    pub name: String,
    pub description: String,
    pub participant_id: String,
}

/// A variable that can be referenced in chat messages.
#[derive(Debug, Clone)]
pub struct ChatVariable {
    pub name: String,
    pub description: String,
    pub value: String,
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
}
