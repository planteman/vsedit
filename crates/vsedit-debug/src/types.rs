//! Core debug types: stack frames, threads, variables, scopes.

use serde::{Deserialize, Serialize};

/// A stack frame returned by the debug adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StackFrame {
    pub id: u64,
    pub name: String,
    pub source_path: Option<String>,
    pub source_name: Option<String>,
    pub line: u32,
    pub column: u32,
}

impl StackFrame {
    pub fn new(id: u64, name: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            id,
            name: name.into(),
            source_path: None,
            source_name: None,
            line,
            column,
        }
    }

    pub fn with_source(mut self, path: impl Into<String>) -> Self {
        let p: String = path.into();
        self.source_name = p.rsplit('/').next().map(|s| s.to_string());
        self.source_path = Some(p);
        self
    }

    /// Parse from a DAP response body entry.
    pub fn from_dap(value: &serde_json::Value) -> Option<Self> {
        Some(Self {
            id: value.get("id")?.as_u64()?,
            name: value.get("name")?.as_str()?.to_string(),
            source_path: value
                .get("source")
                .and_then(|s| s.get("path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            source_name: value
                .get("source")
                .and_then(|s| s.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            line: value.get("line")?.as_u64()? as u32,
            column: value.get("column").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        })
    }
}

/// The reason a thread stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoppedReason {
    Step,
    Breakpoint,
    Exception,
    Pause,
    Entry,
    Goto,
    FunctionBreakpoint,
    DataBreakpoint,
    InstructionBreakpoint,
    Other(String),
}

impl StoppedReason {
    pub fn from_str(s: &str) -> Self {
        match s {
            "step" => StoppedReason::Step,
            "breakpoint" => StoppedReason::Breakpoint,
            "exception" => StoppedReason::Exception,
            "pause" => StoppedReason::Pause,
            "entry" => StoppedReason::Entry,
            "goto" => StoppedReason::Goto,
            "function breakpoint" => StoppedReason::FunctionBreakpoint,
            "data breakpoint" => StoppedReason::DataBreakpoint,
            "instruction breakpoint" => StoppedReason::InstructionBreakpoint,
            other => StoppedReason::Other(other.to_string()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            StoppedReason::Step => "step",
            StoppedReason::Breakpoint => "breakpoint",
            StoppedReason::Exception => "exception",
            StoppedReason::Pause => "pause",
            StoppedReason::Entry => "entry",
            StoppedReason::Goto => "goto",
            StoppedReason::FunctionBreakpoint => "function breakpoint",
            StoppedReason::DataBreakpoint => "data breakpoint",
            StoppedReason::InstructionBreakpoint => "instruction breakpoint",
            StoppedReason::Other(s) => s,
        }
    }
}

/// A thread reported by the debug adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub id: u64,
    pub name: String,
    pub stopped_reason: Option<StoppedReason>,
}

impl Thread {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            stopped_reason: None,
        }
    }

    pub fn with_stopped_reason(mut self, reason: StoppedReason) -> Self {
        self.stopped_reason = Some(reason);
        self
    }

    /// Parse from a DAP response body entry.
    pub fn from_dap(value: &serde_json::Value) -> Option<Self> {
        Some(Self {
            id: value.get("id")?.as_u64()?,
            name: value.get("name")?.as_str()?.to_string(),
            stopped_reason: None,
        })
    }
}

/// A variable returned by the debug adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub type_name: Option<String>,
    pub variables_reference: u64,
    pub named_variables: Option<u32>,
    pub indexed_variables: Option<u32>,
}

impl Variable {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            type_name: None,
            variables_reference: 0,
            named_variables: None,
            indexed_variables: None,
        }
    }

    pub fn with_type(mut self, type_name: impl Into<String>) -> Self {
        self.type_name = Some(type_name.into());
        self
    }

    pub fn with_children_ref(mut self, reference: u64) -> Self {
        self.variables_reference = reference;
        self
    }

    /// Returns true if this variable has child variables that can be expanded.
    pub fn has_children(&self) -> bool {
        self.variables_reference > 0
    }

    /// Parse from a DAP response body entry.
    pub fn from_dap(value: &serde_json::Value) -> Option<Self> {
        Some(Self {
            name: value.get("name")?.as_str()?.to_string(),
            value: value.get("value")?.as_str()?.to_string(),
            type_name: value
                .get("type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            variables_reference: value
                .get("variablesReference")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            named_variables: value
                .get("namedVariables")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            indexed_variables: value
                .get("indexedVariables")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        })
    }
}

/// A scope that contains variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scope {
    pub name: String,
    pub variables_reference: u64,
    pub expensive: bool,
}

impl Scope {
    pub fn new(name: impl Into<String>, variables_reference: u64) -> Self {
        Self {
            name: name.into(),
            variables_reference,
            expensive: false,
        }
    }

    /// Parse from a DAP response body entry.
    pub fn from_dap(value: &serde_json::Value) -> Option<Self> {
        Some(Self {
            name: value.get("name")?.as_str()?.to_string(),
            variables_reference: value.get("variablesReference")?.as_u64()?,
            expensive: value
                .get("expensive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_frame_with_source() {
        let frame = StackFrame::new(1, "main", 10, 1).with_source("/src/main.rs");
        assert_eq!(frame.source_path.as_deref(), Some("/src/main.rs"));
        assert_eq!(frame.source_name.as_deref(), Some("main.rs"));
    }

    #[test]
    fn stack_frame_from_dap() {
        let val = serde_json::json!({
            "id": 1,
            "name": "main",
            "source": {"path": "/app/main.rs", "name": "main.rs"},
            "line": 42,
            "column": 5,
        });
        let frame = StackFrame::from_dap(&val).unwrap();
        assert_eq!(frame.id, 1);
        assert_eq!(frame.line, 42);
        assert_eq!(frame.source_path.as_deref(), Some("/app/main.rs"));
    }

    #[test]
    fn stopped_reason_roundtrip() {
        assert_eq!(StoppedReason::from_str("breakpoint"), StoppedReason::Breakpoint);
        assert_eq!(StoppedReason::from_str("unknown"), StoppedReason::Other("unknown".into()));
        assert_eq!(StoppedReason::Breakpoint.label(), "breakpoint");
        assert_eq!(StoppedReason::Other("custom".into()).label(), "custom");
    }

    #[test]
    fn thread_from_dap() {
        let val = serde_json::json!({"id": 1, "name": "main"});
        let thread = Thread::from_dap(&val).unwrap();
        assert_eq!(thread.id, 1);
        assert_eq!(thread.name, "main");
        assert!(thread.stopped_reason.is_none());
    }

    #[test]
    fn thread_with_stopped_reason() {
        let thread = Thread::new(1, "main").with_stopped_reason(StoppedReason::Breakpoint);
        assert_eq!(thread.stopped_reason, Some(StoppedReason::Breakpoint));
    }

    #[test]
    fn variable_has_children() {
        let simple = Variable::new("x", "42");
        assert!(!simple.has_children());

        let with_ref = Variable::new("obj", "{...}").with_children_ref(10);
        assert!(with_ref.has_children());
    }

    #[test]
    fn variable_from_dap() {
        let val = serde_json::json!({
            "name": "count",
            "value": "42",
            "type": "i32",
            "variablesReference": 0,
            "namedVariables": 0,
        });
        let var = Variable::from_dap(&val).unwrap();
        assert_eq!(var.name, "count");
        assert_eq!(var.type_name.as_deref(), Some("i32"));
        assert!(!var.has_children());
    }

    #[test]
    fn scope_from_dap() {
        let val = serde_json::json!({
            "name": "Locals",
            "variablesReference": 1000,
            "expensive": false,
        });
        let scope = Scope::from_dap(&val).unwrap();
        assert_eq!(scope.name, "Locals");
        assert_eq!(scope.variables_reference, 1000);
        assert!(!scope.expensive);
    }

    #[test]
    fn variable_with_type() {
        let var = Variable::new("x", "42").with_type("i32");
        assert_eq!(var.type_name.as_deref(), Some("i32"));
    }
}
