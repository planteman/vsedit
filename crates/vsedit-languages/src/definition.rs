//! Language definition struct.

/// Configuration for a registered language.
#[derive(Debug, Clone)]
pub struct LanguageDefinition {
    /// Unique identifier (e.g. `"rust"`).
    pub id: String,
    /// Human-readable name (e.g. `"Rust"`).
    pub name: String,
    /// File extensions including the dot (e.g. `[".rs"]`).
    pub extensions: Vec<String>,
    /// Exact filenames (e.g. `["Makefile"]`).
    pub filenames: Vec<String>,
    /// Alternative names (e.g. `["Rust", "rust"]`).
    pub aliases: Vec<String>,
    /// Associated MIME types (e.g. `["text/x-rust"]`).
    pub mime_types: Vec<String>,
    /// Regex for first-line detection (e.g. `"^#!.*python"`).
    pub first_line: Option<String>,
}
