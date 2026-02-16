//! Model Context Protocol service.

use std::fmt;

/// A tool exposed by an MCP server.
#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Option<String>,
}

impl fmt::Display for McpTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.description)
    }
}

/// A resource exposed by an MCP server.
#[derive(Debug, Clone)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// An MCP server instance.
#[derive(Debug, Clone)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
    pub connected: bool,
}

impl McpServer {
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }
}

impl fmt::Display for McpServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.connected { "connected" } else { "disconnected" };
        write!(f, "{} ({})", self.name, status)
    }
}

/// Service for managing MCP servers and their tools.
pub struct McpService {
    servers: Vec<McpServer>,
}

impl McpService {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    pub fn add_server(&mut self, server: McpServer) {
        self.servers.push(server);
    }

    pub fn remove_server(&mut self, id: &str) -> bool {
        let len = self.servers.len();
        self.servers.retain(|s| s.id != id);
        self.servers.len() < len
    }

    pub fn connect(&mut self, id: &str) {
        if let Some(s) = self.servers.iter_mut().find(|s| s.id == id) {
            s.connected = true;
        }
    }

    pub fn disconnect(&mut self, id: &str) {
        if let Some(s) = self.servers.iter_mut().find(|s| s.id == id) {
            s.connected = false;
        }
    }

    pub fn get_server(&self, id: &str) -> Option<&McpServer> {
        self.servers.iter().find(|s| s.id == id)
    }

    pub fn list_tools(&self) -> Vec<&McpTool> {
        self.servers
            .iter()
            .filter(|s| s.connected)
            .flat_map(|s| &s.tools)
            .collect()
    }

    pub fn list_resources(&self) -> Vec<&McpResource> {
        self.servers
            .iter()
            .filter(|s| s.connected)
            .flat_map(|s| &s.resources)
            .collect()
    }

    pub fn get_server_mut(&mut self, id: &str) -> Option<&mut McpServer> {
        self.servers.iter_mut().find(|s| s.id == id)
    }

    pub fn find_tool(&self, name: &str) -> Option<(&McpServer, &McpTool)> {
        self.servers
            .iter()
            .filter(|s| s.connected)
            .find_map(|s| s.tools.iter().find(|t| t.name == name).map(|t| (s, t)))
    }

    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    pub fn connected_count(&self) -> usize {
        self.servers.iter().filter(|s| s.connected).count()
    }

    pub fn disconnected_count(&self) -> usize {
        self.servers.iter().filter(|s| !s.connected).count()
    }

    pub fn get_all_servers(&self) -> &[McpServer] {
        &self.servers
    }

    pub fn disconnect_all(&mut self) {
        for s in &mut self.servers {
            s.connected = false;
        }
    }
}

impl Default for McpService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_server(id: &str, tools: Vec<McpTool>) -> McpServer {
        McpServer {
            id: id.to_string(),
            name: format!("Server {id}"),
            tools,
            resources: vec![],
            connected: false,
        }
    }

    #[test]
    fn add_and_remove_server() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        assert_eq!(svc.server_count(), 1);
        assert!(svc.remove_server("s1"));
        assert_eq!(svc.server_count(), 0);
        assert!(!svc.remove_server("s1"));
    }

    #[test]
    fn connect_disconnect() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        assert!(!svc.get_server("s1").unwrap().connected);
        svc.connect("s1");
        assert!(svc.get_server("s1").unwrap().connected);
        svc.disconnect("s1");
        assert!(!svc.get_server("s1").unwrap().connected);
    }

    #[test]
    fn list_tools_only_connected() {
        let mut svc = McpService::new();
        let tool = McpTool {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            input_schema: None,
        };
        svc.add_server(make_server("s1", vec![tool]));
        assert!(svc.list_tools().is_empty());
        svc.connect("s1");
        assert_eq!(svc.list_tools().len(), 1);
        assert_eq!(svc.list_tools()[0].name, "read");
    }

    fn make_tool(name: &str) -> McpTool {
        McpTool {
            name: name.to_string(),
            description: format!("{name} description"),
            input_schema: None,
        }
    }

    #[test]
    fn find_tool_across_servers() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![make_tool("alpha")]));
        svc.add_server(make_server("s2", vec![make_tool("beta")]));
        svc.connect("s1");
        svc.connect("s2");
        let (server, tool) = svc.find_tool("beta").unwrap();
        assert_eq!(server.id, "s2");
        assert_eq!(tool.name, "beta");
        assert!(svc.find_tool("gamma").is_none());
    }

    #[test]
    fn find_tool_ignores_disconnected() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![make_tool("alpha")]));
        assert!(svc.find_tool("alpha").is_none());
    }

    #[test]
    fn connected_and_disconnected_counts() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        svc.add_server(make_server("s2", vec![]));
        svc.add_server(make_server("s3", vec![]));
        assert_eq!(svc.connected_count(), 0);
        assert_eq!(svc.disconnected_count(), 3);
        svc.connect("s1");
        svc.connect("s2");
        assert_eq!(svc.connected_count(), 2);
        assert_eq!(svc.disconnected_count(), 1);
    }

    #[test]
    fn disconnect_all_servers() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        svc.add_server(make_server("s2", vec![]));
        svc.connect("s1");
        svc.connect("s2");
        assert_eq!(svc.connected_count(), 2);
        svc.disconnect_all();
        assert_eq!(svc.connected_count(), 0);
        assert_eq!(svc.disconnected_count(), 2);
    }

    #[test]
    fn get_server_mut_modifies() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        let server = svc.get_server_mut("s1").unwrap();
        server.tools.push(make_tool("new_tool"));
        assert_eq!(svc.get_server("s1").unwrap().tool_count(), 1);
        assert!(svc.get_server("s1").unwrap().has_tool("new_tool"));
    }

    #[test]
    fn list_resources_only_connected() {
        let mut svc = McpService::new();
        let mut server = make_server("s1", vec![]);
        server.resources.push(McpResource {
            uri: "file:///test.txt".to_string(),
            name: "test".to_string(),
            description: Some("A test resource".to_string()),
            mime_type: Some("text/plain".to_string()),
        });
        svc.add_server(server);
        assert!(svc.list_resources().is_empty());
        svc.connect("s1");
        assert_eq!(svc.list_resources().len(), 1);
        assert_eq!(svc.list_resources()[0].uri, "file:///test.txt");
    }

    #[test]
    fn server_display_format() {
        let mut server = make_server("s1", vec![]);
        assert_eq!(format!("{server}"), "Server s1 (disconnected)");
        server.connected = true;
        assert_eq!(format!("{server}"), "Server s1 (connected)");
    }

    #[test]
    fn tool_display_format() {
        let tool = make_tool("read_file");
        assert_eq!(format!("{tool}"), "read_file: read_file description");
    }

    #[test]
    fn get_all_servers_returns_slice() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        svc.add_server(make_server("s2", vec![]));
        let all = svc.get_all_servers();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, "s1");
        assert_eq!(all[1].id, "s2");
    }
}
