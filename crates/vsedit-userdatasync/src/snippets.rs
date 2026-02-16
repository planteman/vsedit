//! Snippet sync and merge.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A single snippet file with its entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnippetFile {
    pub entries: HashMap<String, SnippetEntry>,
}

/// A single snippet entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnippetEntry {
    pub prefix: String,
    pub body: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Merge local and remote snippet files.
///
/// For each file name, snippets from both sides are merged; when the same
/// snippet name exists in both, the local version wins.
pub fn merge_snippets(
    local_snippets: &HashMap<String, SnippetFile>,
    remote_snippets: &HashMap<String, SnippetFile>,
) -> HashMap<String, SnippetFile> {
    let mut result = local_snippets.clone();

    for (file_name, remote_file) in remote_snippets {
        let merged = result.entry(file_name.clone()).or_insert_with(|| SnippetFile {
            entries: HashMap::new(),
        });
        for (name, entry) in &remote_file.entries {
            merged.entries.entry(name.clone()).or_insert_with(|| entry.clone());
        }
    }

    result
}
