//! AI chat service.

/// A participant that can respond in the chat.
#[derive(Debug, Clone)]
pub struct ChatParticipant {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

/// A slash command available within a chat participant.
#[derive(Debug, Clone)]
pub struct ChatSlashCommand {
    pub name: String,
    pub description: String,
    pub participant_id: String,
}

/// Service for managing chat participants and commands.
pub struct ChatWorkbenchService {
    participants: Vec<ChatParticipant>,
    commands: Vec<ChatSlashCommand>,
}

impl ChatWorkbenchService {
    pub fn new() -> Self {
        Self {
            participants: Vec::new(),
            commands: Vec::new(),
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
}
