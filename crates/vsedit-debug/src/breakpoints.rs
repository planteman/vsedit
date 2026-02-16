//! Breakpoint management across files.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::protocol::SourceBreakpoint;

/// A breakpoint with full metadata (after adapter verification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Breakpoint {
    pub id: Option<u64>,
    pub verified: bool,
    pub line: u32,
    pub column: Option<u32>,
    pub source_path: String,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
    pub log_message: Option<String>,
}

impl Breakpoint {
    /// Create a simple breakpoint at a line.
    pub fn at_line(source_path: impl Into<String>, line: u32) -> Self {
        Self {
            id: None,
            verified: false,
            line,
            column: None,
            source_path: source_path.into(),
            condition: None,
            hit_condition: None,
            log_message: None,
        }
    }

    /// Convert to a [`SourceBreakpoint`] for the DAP protocol.
    pub fn to_source_breakpoint(&self) -> SourceBreakpoint {
        SourceBreakpoint {
            line: self.line,
            column: self.column,
            condition: self.condition.clone(),
            hit_condition: self.hit_condition.clone(),
            log_message: self.log_message.clone(),
        }
    }
}

/// Manages breakpoints across multiple files.
#[derive(Debug, Clone, Default)]
pub struct BreakpointStore {
    breakpoints: HashMap<String, Vec<Breakpoint>>,
    next_id: u64,
}

impl BreakpointStore {
    pub fn new() -> Self {
        Self {
            breakpoints: HashMap::new(),
            next_id: 1,
        }
    }

    /// Toggle a breakpoint at the given file and line. Returns `true` if added,
    /// `false` if removed.
    pub fn toggle_breakpoint(&mut self, file: &str, line: u32) -> bool {
        let bps = self.breakpoints.entry(file.to_string()).or_default();
        if let Some(idx) = bps.iter().position(|bp| bp.line == line) {
            bps.remove(idx);
            if bps.is_empty() {
                self.breakpoints.remove(file);
            }
            false
        } else {
            let mut bp = Breakpoint::at_line(file, line);
            bp.id = Some(self.next_id);
            self.next_id += 1;
            bps.push(bp);
            true
        }
    }

    /// Set a conditional breakpoint at the given file and line.
    pub fn set_conditional_breakpoint(
        &mut self,
        file: &str,
        line: u32,
        condition: impl Into<String>,
    ) {
        let bps = self.breakpoints.entry(file.to_string()).or_default();
        // Replace if exists at same line
        bps.retain(|bp| bp.line != line);
        let mut bp = Breakpoint::at_line(file, line);
        bp.id = Some(self.next_id);
        bp.condition = Some(condition.into());
        self.next_id += 1;
        bps.push(bp);
    }

    /// Set a log-point (tracepoint) at the given file and line.
    pub fn set_logpoint(&mut self, file: &str, line: u32, message: impl Into<String>) {
        let bps = self.breakpoints.entry(file.to_string()).or_default();
        bps.retain(|bp| bp.line != line);
        let mut bp = Breakpoint::at_line(file, line);
        bp.id = Some(self.next_id);
        bp.log_message = Some(message.into());
        self.next_id += 1;
        bps.push(bp);
    }

    /// Get all breakpoints for a given file.
    pub fn get_breakpoints(&self, file: &str) -> Vec<&Breakpoint> {
        self.breakpoints
            .get(file)
            .map(|bps| bps.iter().collect())
            .unwrap_or_default()
    }

    /// Get all files that have breakpoints set.
    pub fn files_with_breakpoints(&self) -> Vec<&str> {
        self.breakpoints.keys().map(|s| s.as_str()).collect()
    }

    /// Clear all breakpoints.
    pub fn clear_all_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Clear all breakpoints in a specific file.
    pub fn clear_file_breakpoints(&mut self, file: &str) {
        self.breakpoints.remove(file);
    }

    /// Total number of breakpoints across all files.
    pub fn total_count(&self) -> usize {
        self.breakpoints.values().map(|v| v.len()).sum()
    }

    /// Get source breakpoints for a file (for sending to DAP adapter).
    pub fn source_breakpoints_for(&self, file: &str) -> Vec<SourceBreakpoint> {
        self.get_breakpoints(file)
            .iter()
            .map(|bp| bp.to_source_breakpoint())
            .collect()
    }

    /// Update breakpoints from adapter verification response.
    pub fn update_verified(&mut self, file: &str, verified: &[serde_json::Value]) {
        if let Some(bps) = self.breakpoints.get_mut(file) {
            for (bp, v) in bps.iter_mut().zip(verified.iter()) {
                if let Some(ver) = v.get("verified").and_then(|v| v.as_bool()) {
                    bp.verified = ver;
                }
                if let Some(id) = v.get("id").and_then(|v| v.as_u64()) {
                    bp.id = Some(id);
                }
                if let Some(line) = v.get("line").and_then(|v| v.as_u64()) {
                    bp.line = line as u32;
                }
            }
        }
    }

    /// Serialize all breakpoints to JSON (for saving to launch.json).
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (file, bps) in &self.breakpoints {
            map.insert(
                file.clone(),
                serde_json::to_value(bps).unwrap_or_default(),
            );
        }
        serde_json::Value::Object(map)
    }

    /// Load breakpoints from a JSON object.
    pub fn from_json(value: &serde_json::Value) -> Self {
        let mut store = Self::new();
        if let Some(obj) = value.as_object() {
            for (file, bps_val) in obj {
                if let Ok(bps) = serde_json::from_value::<Vec<Breakpoint>>(bps_val.clone()) {
                    let max_id = bps.iter().filter_map(|bp| bp.id).max().unwrap_or(0);
                    if max_id >= store.next_id {
                        store.next_id = max_id + 1;
                    }
                    store.breakpoints.insert(file.clone(), bps);
                }
            }
        }
        store
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_breakpoint_add_remove() {
        let mut store = BreakpointStore::new();
        assert!(store.toggle_breakpoint("main.rs", 10));
        assert_eq!(store.total_count(), 1);
        assert!(!store.toggle_breakpoint("main.rs", 10));
        assert_eq!(store.total_count(), 0);
    }

    #[test]
    fn conditional_breakpoint() {
        let mut store = BreakpointStore::new();
        store.set_conditional_breakpoint("main.rs", 10, "x > 5");
        let bps = store.get_breakpoints("main.rs");
        assert_eq!(bps.len(), 1);
        assert_eq!(bps[0].condition.as_deref(), Some("x > 5"));
    }

    #[test]
    fn conditional_replaces_existing() {
        let mut store = BreakpointStore::new();
        store.toggle_breakpoint("main.rs", 10);
        store.set_conditional_breakpoint("main.rs", 10, "y == 0");
        let bps = store.get_breakpoints("main.rs");
        assert_eq!(bps.len(), 1);
        assert_eq!(bps[0].condition.as_deref(), Some("y == 0"));
    }

    #[test]
    fn logpoint() {
        let mut store = BreakpointStore::new();
        store.set_logpoint("main.rs", 20, "value is {x}");
        let bps = store.get_breakpoints("main.rs");
        assert_eq!(bps[0].log_message.as_deref(), Some("value is {x}"));
    }

    #[test]
    fn get_breakpoints_empty_file() {
        let store = BreakpointStore::new();
        assert!(store.get_breakpoints("nonexistent.rs").is_empty());
    }

    #[test]
    fn clear_all() {
        let mut store = BreakpointStore::new();
        store.toggle_breakpoint("a.rs", 1);
        store.toggle_breakpoint("b.rs", 2);
        assert_eq!(store.total_count(), 2);
        store.clear_all_breakpoints();
        assert_eq!(store.total_count(), 0);
    }

    #[test]
    fn clear_file() {
        let mut store = BreakpointStore::new();
        store.toggle_breakpoint("a.rs", 1);
        store.toggle_breakpoint("b.rs", 2);
        store.clear_file_breakpoints("a.rs");
        assert_eq!(store.total_count(), 1);
        assert!(store.get_breakpoints("a.rs").is_empty());
    }

    #[test]
    fn files_with_breakpoints() {
        let mut store = BreakpointStore::new();
        store.toggle_breakpoint("a.rs", 1);
        store.toggle_breakpoint("b.rs", 2);
        let mut files = store.files_with_breakpoints();
        files.sort();
        assert_eq!(files, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn source_breakpoints_for() {
        let mut store = BreakpointStore::new();
        store.set_conditional_breakpoint("main.rs", 10, "x > 0");
        let sbs = store.source_breakpoints_for("main.rs");
        assert_eq!(sbs.len(), 1);
        assert_eq!(sbs[0].line, 10);
        assert_eq!(sbs[0].condition.as_deref(), Some("x > 0"));
    }

    #[test]
    fn serialize_deserialize() {
        let mut store = BreakpointStore::new();
        store.toggle_breakpoint("main.rs", 10);
        store.set_conditional_breakpoint("lib.rs", 20, "y != 0");

        let json = store.to_json();
        let loaded = BreakpointStore::from_json(&json);
        assert_eq!(loaded.total_count(), 2);
        let bps = loaded.get_breakpoints("lib.rs");
        assert_eq!(bps[0].condition.as_deref(), Some("y != 0"));
    }

    #[test]
    fn update_verified() {
        let mut store = BreakpointStore::new();
        store.toggle_breakpoint("main.rs", 10);
        store.toggle_breakpoint("main.rs", 20);

        let verified = vec![
            serde_json::json!({"id": 1, "verified": true, "line": 10}),
            serde_json::json!({"id": 2, "verified": true, "line": 21}),
        ];
        store.update_verified("main.rs", &verified);

        let bps = store.get_breakpoints("main.rs");
        assert!(bps[0].verified);
        assert_eq!(bps[1].line, 21); // adjusted by adapter
    }

    #[test]
    fn breakpoint_to_source_breakpoint() {
        let mut bp = Breakpoint::at_line("test.rs", 42);
        bp.condition = Some("i > 0".into());
        bp.hit_condition = Some("5".into());
        let sb = bp.to_source_breakpoint();
        assert_eq!(sb.line, 42);
        assert_eq!(sb.condition.as_deref(), Some("i > 0"));
        assert_eq!(sb.hit_condition.as_deref(), Some("5"));
    }
}
