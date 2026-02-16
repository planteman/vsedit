//! Debug console for expression evaluation and output display.

use serde::{Deserialize, Serialize};

/// Category of debug output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputCategory {
    Console,
    Stdout,
    Stderr,
    Telemetry,
}

impl OutputCategory {
    pub fn from_dap(category: &str) -> Self {
        match category {
            "stdout" => OutputCategory::Stdout,
            "stderr" => OutputCategory::Stderr,
            "telemetry" => OutputCategory::Telemetry,
            _ => OutputCategory::Console,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            OutputCategory::Console => "console",
            OutputCategory::Stdout => "stdout",
            OutputCategory::Stderr => "stderr",
            OutputCategory::Telemetry => "telemetry",
        }
    }
}

/// An entry in the debug console.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DebugConsoleEntry {
    Input(String),
    Output(String, OutputCategory),
}

impl DebugConsoleEntry {
    pub fn text(&self) -> &str {
        match self {
            DebugConsoleEntry::Input(s) => s,
            DebugConsoleEntry::Output(s, _) => s,
        }
    }

    pub fn is_input(&self) -> bool {
        matches!(self, DebugConsoleEntry::Input(_))
    }

    pub fn is_output(&self) -> bool {
        matches!(self, DebugConsoleEntry::Output(..))
    }
}

/// Debug console that stores input/output history.
#[derive(Debug, Clone, Default)]
pub struct DebugConsole {
    entries: Vec<DebugConsoleEntry>,
    input_history: Vec<String>,
    history_index: Option<usize>,
    max_entries: usize,
}

impl DebugConsole {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            input_history: Vec::new(),
            history_index: None,
            max_entries: 10_000,
        }
    }

    /// Add an input expression (from user).
    pub fn add_input(&mut self, expression: impl Into<String>) {
        let expr = expression.into();
        self.input_history.push(expr.clone());
        self.history_index = None;
        self.entries.push(DebugConsoleEntry::Input(expr));
        self.trim();
    }

    /// Add an output entry (from debug adapter).
    pub fn add_output(&mut self, text: impl Into<String>, category: OutputCategory) {
        self.entries
            .push(DebugConsoleEntry::Output(text.into(), category));
        self.trim();
    }

    /// Get all entries.
    pub fn entries(&self) -> &[DebugConsoleEntry] {
        &self.entries
    }

    /// Get only output entries.
    pub fn outputs(&self) -> Vec<&DebugConsoleEntry> {
        self.entries.iter().filter(|e| e.is_output()).collect()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the console is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Navigate to previous input in history.
    pub fn previous_input(&mut self) -> Option<&str> {
        if self.input_history.is_empty() {
            return None;
        }
        let idx = match self.history_index {
            Some(0) => 0,
            Some(i) => i - 1,
            None => self.input_history.len() - 1,
        };
        self.history_index = Some(idx);
        Some(&self.input_history[idx])
    }

    /// Navigate to next input in history.
    pub fn next_input(&mut self) -> Option<&str> {
        let idx = match self.history_index {
            Some(i) if i + 1 < self.input_history.len() => i + 1,
            _ => return None,
        };
        self.history_index = Some(idx);
        Some(&self.input_history[idx])
    }

    fn trim(&mut self) {
        if self.entries.len() > self.max_entries {
            let excess = self.entries.len() - self.max_entries;
            self.entries.drain(..excess);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_category_from_dap() {
        assert_eq!(OutputCategory::from_dap("stdout"), OutputCategory::Stdout);
        assert_eq!(OutputCategory::from_dap("stderr"), OutputCategory::Stderr);
        assert_eq!(
            OutputCategory::from_dap("telemetry"),
            OutputCategory::Telemetry
        );
        assert_eq!(
            OutputCategory::from_dap("console"),
            OutputCategory::Console
        );
        assert_eq!(
            OutputCategory::from_dap("unknown"),
            OutputCategory::Console
        );
    }

    #[test]
    fn output_category_label() {
        assert_eq!(OutputCategory::Stdout.label(), "stdout");
        assert_eq!(OutputCategory::Console.label(), "console");
    }

    #[test]
    fn console_entry_text() {
        let input = DebugConsoleEntry::Input("x + 1".into());
        assert_eq!(input.text(), "x + 1");
        assert!(input.is_input());
        assert!(!input.is_output());

        let output = DebugConsoleEntry::Output("hello\n".into(), OutputCategory::Stdout);
        assert_eq!(output.text(), "hello\n");
        assert!(output.is_output());
    }

    #[test]
    fn console_add_and_retrieve() {
        let mut console = DebugConsole::new();
        console.add_input("x + 1");
        console.add_output("42", OutputCategory::Console);

        assert_eq!(console.len(), 2);
        assert!(!console.is_empty());
        assert_eq!(console.entries()[0].text(), "x + 1");
        assert_eq!(console.entries()[1].text(), "42");
    }

    #[test]
    fn console_clear() {
        let mut console = DebugConsole::new();
        console.add_input("test");
        console.clear();
        assert!(console.is_empty());
    }

    #[test]
    fn console_outputs_filter() {
        let mut console = DebugConsole::new();
        console.add_input("x");
        console.add_output("result", OutputCategory::Console);
        console.add_input("y");
        console.add_output("error", OutputCategory::Stderr);

        let outputs = console.outputs();
        assert_eq!(outputs.len(), 2);
    }

    #[test]
    fn console_history_navigation() {
        let mut console = DebugConsole::new();
        console.add_input("first");
        console.add_input("second");
        console.add_input("third");

        assert_eq!(console.previous_input(), Some("third"));
        assert_eq!(console.previous_input(), Some("second"));
        assert_eq!(console.previous_input(), Some("first"));
        assert_eq!(console.previous_input(), Some("first")); // stays at beginning

        assert_eq!(console.next_input(), Some("second"));
        assert_eq!(console.next_input(), Some("third"));
        assert!(console.next_input().is_none()); // at end
    }

    #[test]
    fn console_history_empty() {
        let mut console = DebugConsole::new();
        assert!(console.previous_input().is_none());
        assert!(console.next_input().is_none());
    }
}
