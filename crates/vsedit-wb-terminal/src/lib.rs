//! Terminal management.

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TerminalShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Custom(String),
}

impl fmt::Display for TerminalShellType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bash => write!(f, "bash"),
            Self::Zsh => write!(f, "zsh"),
            Self::Fish => write!(f, "fish"),
            Self::PowerShell => write!(f, "powershell"),
            Self::Cmd => write!(f, "cmd"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerminalDimensions {
    pub columns: u32,
    pub rows: u32,
}

impl TerminalDimensions {
    /// Returns the total cell area (columns × rows).
    pub fn area(&self) -> u32 {
        self.columns * self.rows
    }

    /// Resize with minimum constraints (columns ≥ 1, rows ≥ 1).
    pub fn resize(&mut self, columns: u32, rows: u32) {
        self.columns = columns.max(1);
        self.rows = rows.max(1);
    }
}

impl fmt::Display for TerminalDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.columns, self.rows)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CursorStyle {
    Block,
    Underline,
    Line,
}

impl fmt::Display for CursorStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Block => write!(f, "block"),
            Self::Underline => write!(f, "underline"),
            Self::Line => write!(f, "line"),
        }
    }
}

/// Errors returned by terminal operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalError {
    InstanceNotFound(u32),
    NoActiveInstance,
    InvalidDimensions { columns: u32, rows: u32 },
}

impl fmt::Display for TerminalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstanceNotFound(id) => write!(f, "terminal instance {id} not found"),
            Self::NoActiveInstance => write!(f, "no active terminal instance"),
            Self::InvalidDimensions { columns, rows } => {
                write!(f, "invalid dimensions: {columns}x{rows}")
            }
        }
    }
}

/// A single terminal instance.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalInstance {
    pub id: u32,
    pub title: String,
    pub shell_type: TerminalShellType,
    pub dimensions: TerminalDimensions,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct TerminalWorkbenchConfig {
    pub default_shell: TerminalShellType,
    pub font_size: u32,
    pub font_family: String,
    pub cursor_style: CursorStyle,
    pub scrollback: u32,
}

impl Default for TerminalWorkbenchConfig {
    fn default() -> Self {
        Self {
            default_shell: TerminalShellType::Bash,
            font_size: 14,
            font_family: "monospace".into(),
            cursor_style: CursorStyle::Block,
            scrollback: 1000,
        }
    }
}

/// Builder for `TerminalWorkbenchConfig`.
#[derive(Debug)]
pub struct TerminalWorkbenchConfigBuilder {
    config: TerminalWorkbenchConfig,
}

impl TerminalWorkbenchConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: TerminalWorkbenchConfig::default(),
        }
    }

    pub fn default_shell(mut self, shell: TerminalShellType) -> Self {
        self.config.default_shell = shell;
        self
    }

    pub fn font_size(mut self, size: u32) -> Self {
        self.config.font_size = size;
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.config.font_family = family.into();
        self
    }

    pub fn cursor_style(mut self, style: CursorStyle) -> Self {
        self.config.cursor_style = style;
        self
    }

    pub fn scrollback(mut self, lines: u32) -> Self {
        self.config.scrollback = lines;
        self
    }

    pub fn build(self) -> TerminalWorkbenchConfig {
        self.config
    }
}

impl Default for TerminalWorkbenchConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for terminal workbench functionality.
pub struct TerminalWorkbenchService {
    config: TerminalWorkbenchConfig,
    instance_count: u32,
    instances: HashMap<u32, TerminalInstance>,
    active_instance_id: Option<u32>,
}

impl TerminalWorkbenchService {
    pub fn new() -> Self {
        Self {
            config: TerminalWorkbenchConfig::default(),
            instance_count: 0,
            instances: HashMap::new(),
            active_instance_id: None,
        }
    }

    pub fn get_config(&self) -> &TerminalWorkbenchConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: TerminalWorkbenchConfig) {
        self.config = config;
    }

    pub fn default_dimensions() -> TerminalDimensions {
        TerminalDimensions {
            columns: 80,
            rows: 24,
        }
    }

    pub fn create_instance(&mut self) -> u32 {
        self.instance_count += 1;
        let id = self.instance_count;
        let instance = TerminalInstance {
            id,
            title: format!("Terminal {id}"),
            shell_type: self.config.default_shell.clone(),
            dimensions: Self::default_dimensions(),
            active: false,
        };
        self.instances.insert(id, instance);
        id
    }

    pub fn close_instance(&mut self, id: u32) {
        if self.instances.remove(&id).is_some() {
            if self.active_instance_id == Some(id) {
                self.active_instance_id = None;
            }
        }
    }

    pub fn active_instance_count(&self) -> u32 {
        self.instances.len() as u32
    }

    /// Returns a reference to the instance with the given id.
    pub fn get_instance(&self, id: u32) -> Result<&TerminalInstance, TerminalError> {
        self.instances
            .get(&id)
            .ok_or(TerminalError::InstanceNotFound(id))
    }

    /// Renames an existing instance.
    pub fn rename_instance(&mut self, id: u32, title: impl Into<String>) -> Result<(), TerminalError> {
        self.instances
            .get_mut(&id)
            .ok_or(TerminalError::InstanceNotFound(id))?
            .title = title.into();
        Ok(())
    }

    /// Sets the active instance by id.
    pub fn set_active_instance(&mut self, id: u32) -> Result<(), TerminalError> {
        if !self.instances.contains_key(&id) {
            return Err(TerminalError::InstanceNotFound(id));
        }
        // Deactivate previous
        if let Some(prev) = self.active_instance_id {
            if let Some(inst) = self.instances.get_mut(&prev) {
                inst.active = false;
            }
        }
        self.active_instance_id = Some(id);
        if let Some(inst) = self.instances.get_mut(&id) {
            inst.active = true;
        }
        Ok(())
    }

    /// Returns the active instance id, if any.
    pub fn get_active_instance_id(&self) -> Result<u32, TerminalError> {
        self.active_instance_id.ok_or(TerminalError::NoActiveInstance)
    }
}

impl Default for TerminalWorkbenchService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_management() {
        let mut svc = TerminalWorkbenchService::new();
        assert_eq!(svc.active_instance_count(), 0);
        let id1 = svc.create_instance();
        let _id2 = svc.create_instance();
        assert_eq!(svc.active_instance_count(), 2);
        svc.close_instance(id1);
        assert_eq!(svc.active_instance_count(), 1);
    }

    #[test]
    fn default_config() {
        let svc = TerminalWorkbenchService::new();
        let cfg = svc.get_config();
        assert_eq!(cfg.default_shell, TerminalShellType::Bash);
        assert_eq!(cfg.font_size, 14);
        assert_eq!(cfg.cursor_style, CursorStyle::Block);
    }

    #[test]
    fn update_config() {
        let mut svc = TerminalWorkbenchService::new();
        let cfg = TerminalWorkbenchConfig {
            default_shell: TerminalShellType::Zsh,
            font_size: 16,
            font_family: "Fira Code".into(),
            cursor_style: CursorStyle::Line,
            scrollback: 5000,
        };
        svc.update_config(cfg);
        assert_eq!(svc.get_config().default_shell, TerminalShellType::Zsh);
        assert_eq!(svc.get_config().scrollback, 5000);
    }

    #[test]
    fn default_dimensions() {
        let dims = TerminalWorkbenchService::default_dimensions();
        assert_eq!(dims.columns, 80);
        assert_eq!(dims.rows, 24);
    }

    #[test]
    fn display_shell_type() {
        assert_eq!(TerminalShellType::Bash.to_string(), "bash");
        assert_eq!(TerminalShellType::Zsh.to_string(), "zsh");
        assert_eq!(TerminalShellType::Fish.to_string(), "fish");
        assert_eq!(TerminalShellType::PowerShell.to_string(), "powershell");
        assert_eq!(TerminalShellType::Cmd.to_string(), "cmd");
        assert_eq!(TerminalShellType::Custom("/bin/sh".into()).to_string(), "/bin/sh");
    }

    #[test]
    fn display_cursor_style() {
        assert_eq!(CursorStyle::Block.to_string(), "block");
        assert_eq!(CursorStyle::Underline.to_string(), "underline");
        assert_eq!(CursorStyle::Line.to_string(), "line");
    }

    #[test]
    fn display_dimensions() {
        let dims = TerminalDimensions { columns: 120, rows: 40 };
        assert_eq!(dims.to_string(), "120x40");
    }

    #[test]
    fn dimensions_area() {
        let dims = TerminalDimensions { columns: 80, rows: 24 };
        assert_eq!(dims.area(), 1920);
    }

    #[test]
    fn dimensions_resize() {
        let mut dims = TerminalDimensions { columns: 80, rows: 24 };
        dims.resize(120, 40);
        assert_eq!(dims.columns, 120);
        assert_eq!(dims.rows, 40);
        dims.resize(0, 0);
        assert_eq!(dims.columns, 1);
        assert_eq!(dims.rows, 1);
    }

    #[test]
    fn terminal_error_display() {
        assert_eq!(
            TerminalError::InstanceNotFound(42).to_string(),
            "terminal instance 42 not found"
        );
        assert_eq!(
            TerminalError::NoActiveInstance.to_string(),
            "no active terminal instance"
        );
        assert_eq!(
            TerminalError::InvalidDimensions { columns: 0, rows: 0 }.to_string(),
            "invalid dimensions: 0x0"
        );
    }

    #[test]
    fn get_instance() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        let inst = svc.get_instance(id).unwrap();
        assert_eq!(inst.id, id);
        assert_eq!(inst.title, "Terminal 1");
        assert_eq!(inst.shell_type, TerminalShellType::Bash);
        assert_eq!(svc.get_instance(999), Err(TerminalError::InstanceNotFound(999)));
    }

    #[test]
    fn rename_instance() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        svc.rename_instance(id, "Dev Server").unwrap();
        assert_eq!(svc.get_instance(id).unwrap().title, "Dev Server");
        assert_eq!(
            svc.rename_instance(999, "nope"),
            Err(TerminalError::InstanceNotFound(999))
        );
    }

    #[test]
    fn active_instance_tracking() {
        let mut svc = TerminalWorkbenchService::new();
        assert_eq!(svc.get_active_instance_id(), Err(TerminalError::NoActiveInstance));
        let id1 = svc.create_instance();
        let id2 = svc.create_instance();
        svc.set_active_instance(id1).unwrap();
        assert_eq!(svc.get_active_instance_id(), Ok(id1));
        assert!(svc.get_instance(id1).unwrap().active);
        svc.set_active_instance(id2).unwrap();
        assert_eq!(svc.get_active_instance_id(), Ok(id2));
        assert!(!svc.get_instance(id1).unwrap().active);
        assert!(svc.get_instance(id2).unwrap().active);
        assert_eq!(
            svc.set_active_instance(999),
            Err(TerminalError::InstanceNotFound(999))
        );
    }

    #[test]
    fn close_active_instance_clears_active() {
        let mut svc = TerminalWorkbenchService::new();
        let id = svc.create_instance();
        svc.set_active_instance(id).unwrap();
        svc.close_instance(id);
        assert_eq!(svc.get_active_instance_id(), Err(TerminalError::NoActiveInstance));
        assert_eq!(svc.active_instance_count(), 0);
    }

    #[test]
    fn config_builder() {
        let cfg = TerminalWorkbenchConfigBuilder::new()
            .default_shell(TerminalShellType::Fish)
            .font_size(18)
            .font_family("JetBrains Mono")
            .cursor_style(CursorStyle::Underline)
            .scrollback(2000)
            .build();
        assert_eq!(cfg.default_shell, TerminalShellType::Fish);
        assert_eq!(cfg.font_size, 18);
        assert_eq!(cfg.font_family, "JetBrains Mono");
        assert_eq!(cfg.cursor_style, CursorStyle::Underline);
        assert_eq!(cfg.scrollback, 2000);
    }
}
