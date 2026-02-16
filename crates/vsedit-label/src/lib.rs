//! Resource label formatting – path manipulation and label templates.

use std::fmt;

/// Errors that can occur during label formatting and validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelError {
    /// The format pattern contains no recognized placeholders.
    NoPlaceholders,
    /// The path string is empty.
    EmptyPath,
    /// The label name exceeds the maximum allowed length.
    NameTooLong { max: usize, actual: usize },
    /// An unknown placeholder was found in the pattern.
    UnknownPlaceholder(String),
}

impl fmt::Display for LabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LabelError::NoPlaceholders => write!(f, "format pattern contains no placeholders"),
            LabelError::EmptyPath => write!(f, "path must not be empty"),
            LabelError::NameTooLong { max, actual } => {
                write!(f, "name length {actual} exceeds maximum {max}")
            }
            LabelError::UnknownPlaceholder(p) => write!(f, "unknown placeholder: {p}"),
        }
    }
}

impl std::error::Error for LabelError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelFormat {
    pub pattern: String,
}

/// Display detail level for labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelDetail {
    /// Filename only.
    Short,
    /// Filename + parent directory.
    Medium,
    /// Full path with filename.
    Long,
    /// Full path with icon and description.
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLabel {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub icon: Option<String>,
}

impl fmt::Display for ResourceLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref desc) = self.description {
            write!(f, "{} — {}", self.name, desc)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

impl fmt::Display for LabelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LabelFormat({})", self.pattern)
    }
}

impl fmt::Display for LabelDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LabelDetail::Short => write!(f, "short"),
            LabelDetail::Medium => write!(f, "medium"),
            LabelDetail::Long => write!(f, "long"),
            LabelDetail::Full => write!(f, "full"),
        }
    }
}

/// Builder for constructing a [`ResourceLabel`] with validation.
#[derive(Debug, Clone, Default)]
pub struct ResourceLabelBuilder {
    name: Option<String>,
    path: Option<String>,
    description: Option<String>,
    icon: Option<String>,
}

impl ResourceLabelBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Build the label, deriving the name from the path filename if not set.
    pub fn build(self) -> Result<ResourceLabel, LabelError> {
        let path = self.path.ok_or(LabelError::EmptyPath)?;
        if path.is_empty() {
            return Err(LabelError::EmptyPath);
        }
        let name = self.name.unwrap_or_else(|| extract_filename(&path).to_string());
        const MAX_NAME_LEN: usize = 255;
        if name.len() > MAX_NAME_LEN {
            return Err(LabelError::NameTooLong {
                max: MAX_NAME_LEN,
                actual: name.len(),
            });
        }
        Ok(ResourceLabel {
            name,
            path,
            description: self.description,
            icon: self.icon,
        })
    }
}

impl ResourceLabel {
    /// Create a builder for this type.
    pub fn builder() -> ResourceLabelBuilder {
        ResourceLabelBuilder::new()
    }

    /// Format this label at the given detail level.
    pub fn format(&self, detail: LabelDetail) -> String {
        format_file_label(&self.path, detail)
    }

    /// Return the file extension of this label's path, if any.
    pub fn extension(&self) -> Option<&str> {
        extract_extension(&self.path)
    }

    /// Return the parent directory portion of this label's path.
    pub fn parent_dir(&self) -> &str {
        self.path.rfind('/').map(|i| &self.path[..i]).unwrap_or("")
    }
}

/// A segment of a highlighted label (for fuzzy match rendering).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSegment {
    pub text: String,
    pub highlighted: bool,
}

/// Replace `${filename}`, `${dirname}`, and `${extname}` in the format pattern.
pub fn format_label(path: &str, format: &LabelFormat) -> String {
    let filename = extract_filename(path);
    let dirname = path
        .rfind('/')
        .map(|i| &path[..i])
        .unwrap_or("");
    let extname = extract_extension(path).unwrap_or("");

    format
        .pattern
        .replace("${filename}", filename)
        .replace("${dirname}", dirname)
        .replace("${extname}", extname)
}

/// Format a file path label at the requested detail level.
pub fn format_file_label(path: &str, detail: LabelDetail) -> String {
    match detail {
        LabelDetail::Short => extract_filename(path).to_string(),
        LabelDetail::Medium => {
            let filename = extract_filename(path);
            let parent = path
                .rfind('/')
                .and_then(|i| path[..i].rfind('/').map(|j| &path[j + 1..i]))
                .unwrap_or("");
            if parent.is_empty() {
                filename.to_string()
            } else {
                format!("{filename} — {parent}")
            }
        }
        LabelDetail::Long => path.to_string(),
        LabelDetail::Full => {
            let filename = extract_filename(path);
            let ext = extract_extension(path).unwrap_or("file");
            format!("[{ext}] {filename} — {path}")
        }
    }
}

/// Format a workspace name for display.
pub fn format_workspace_label(name: &str, root_path: Option<&str>) -> String {
    match root_path {
        Some(rp) => format!("{name} ({rp})"),
        None => name.to_string(),
    }
}

/// Highlight characters matching a fuzzy query in a label text.
/// Returns styled segments indicating which parts are highlighted.
pub fn highlight_label(label: &str, query: &str) -> Vec<LabelSegment> {
    if query.is_empty() {
        return vec![LabelSegment {
            text: label.to_string(),
            highlighted: false,
        }];
    }

    let label_lower = label.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut match_indices = Vec::new();
    let mut qi = 0;
    let query_chars: Vec<char> = query_lower.chars().collect();

    for (i, ch) in label_lower.chars().enumerate() {
        if qi < query_chars.len() && ch == query_chars[qi] {
            match_indices.push(i);
            qi += 1;
        }
    }

    if qi != query_chars.len() {
        // No full match; return unhighlighted
        return vec![LabelSegment {
            text: label.to_string(),
            highlighted: false,
        }];
    }

    let mut segments = Vec::new();
    let chars: Vec<char> = label.chars().collect();
    let mut i = 0;
    let mut mi = 0;

    while i < chars.len() {
        if mi < match_indices.len() && i == match_indices[mi] {
            let mut end = i;
            while mi < match_indices.len() && match_indices[mi] == end {
                mi += 1;
                end += 1;
            }
            segments.push(LabelSegment {
                text: chars[i..end].iter().collect(),
                highlighted: true,
            });
            i = end;
        } else {
            let start = i;
            while i < chars.len() && (mi >= match_indices.len() || i != match_indices[mi]) {
                i += 1;
            }
            segments.push(LabelSegment {
                text: chars[start..i].iter().collect(),
                highlighted: false,
            });
        }
    }

    segments
}

/// Shorten a path by replacing middle segments with `...` if it exceeds `max_len`.
pub fn shorten_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 2 {
        return path.to_string();
    }
    let first = parts[0];
    let last = parts[parts.len() - 1];
    let shortened = format!("{first}/.../{last}");
    if shortened.len() >= path.len() {
        return path.to_string();
    }
    shortened
}

/// Extract the filename component from a path.
pub fn extract_filename(path: &str) -> &str {
    path.rfind('/').map(|i| &path[i + 1..]).unwrap_or(path)
}

/// Extract the file extension (without the dot) from a path.
pub fn extract_extension(path: &str) -> Option<&str> {
    let filename = extract_filename(path);
    filename.rfind('.').map(|i| &filename[i + 1..])
}

/// Validate a format pattern, returning an error if it contains no known placeholders.
pub fn validate_format(format: &LabelFormat) -> Result<(), LabelError> {
    let known = ["${filename}", "${dirname}", "${extname}"];
    if !known.iter().any(|p| format.pattern.contains(p)) {
        return Err(LabelError::NoPlaceholders);
    }
    Ok(())
}

/// Extract the stem (filename without extension) from a path.
pub fn extract_stem(path: &str) -> &str {
    let filename = extract_filename(path);
    match filename.rfind('.') {
        Some(i) if i > 0 => &filename[..i],
        _ => filename,
    }
}

/// Count the depth (number of path segments) of a path.
pub fn path_depth(path: &str) -> usize {
    if path.is_empty() {
        return 0;
    }
    path.split('/').filter(|s| !s.is_empty()).count()
}

/// Compute a common prefix shared by all given paths.
pub fn common_path_prefix<'a>(paths: &[&'a str]) -> &'a str {
    if paths.is_empty() {
        return "";
    }
    let first = paths[0];
    let mut end = first.len();
    for p in &paths[1..] {
        end = end.min(p.len());
        for (i, (a, b)) in first.bytes().zip(p.bytes()).enumerate() {
            if a != b {
                end = end.min(i);
                break;
            }
        }
    }
    // Snap back to the last '/' boundary so we don't split mid-segment.
    if let Some(pos) = first[..end].rfind('/') {
        &first[..=pos]
    } else {
        ""
    }
}

/// Strip a prefix from a path, returning the relative remainder.
pub fn strip_prefix<'a>(path: &'a str, prefix: &str) -> &'a str {
    path.strip_prefix(prefix).unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_label_replaces_variables() {
        let fmt = LabelFormat {
            pattern: "${filename} — ${dirname}".to_string(),
        };
        let result = format_label("/home/user/project/main.rs", &fmt);
        assert_eq!(result, "main.rs — /home/user/project");
    }

    #[test]
    fn extract_filename_and_extension() {
        assert_eq!(extract_filename("/foo/bar/baz.txt"), "baz.txt");
        assert_eq!(extract_extension("/foo/bar/baz.txt"), Some("txt"));
        assert_eq!(extract_filename("nopath"), "nopath");
        assert_eq!(extract_extension("noext"), None);
    }

    #[test]
    fn shorten_path_long() {
        let path = "/home/user/projects/deeply/nested/file.rs";
        let short = shorten_path(path, 20);
        assert!(short.contains("..."));
        assert!(short.len() < path.len());
    }

    #[test]
    fn shorten_path_already_short() {
        assert_eq!(shorten_path("a/b.rs", 50), "a/b.rs");
    }

    #[test]
    fn format_file_label_short() {
        assert_eq!(format_file_label("/a/b/c.rs", LabelDetail::Short), "c.rs");
    }

    #[test]
    fn format_file_label_medium() {
        let label = format_file_label("/home/user/project/main.rs", LabelDetail::Medium);
        assert!(label.contains("main.rs"));
        assert!(label.contains("project"));
    }

    #[test]
    fn format_file_label_full() {
        let label = format_file_label("/a/b.rs", LabelDetail::Full);
        assert!(label.contains("[rs]"));
        assert!(label.contains("b.rs"));
    }

    #[test]
    fn format_workspace_label_with_path() {
        assert_eq!(
            format_workspace_label("my-project", Some("/home/user/project")),
            "my-project (/home/user/project)"
        );
    }

    #[test]
    fn format_workspace_label_without_path() {
        assert_eq!(format_workspace_label("my-project", None), "my-project");
    }

    #[test]
    fn highlight_label_basic() {
        let segments = highlight_label("main.rs", "mn");
        let highlighted: String = segments
            .iter()
            .filter(|s| s.highlighted)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(highlighted, "mn");
    }

    #[test]
    fn highlight_label_no_match() {
        let segments = highlight_label("hello", "xyz");
        assert_eq!(segments.len(), 1);
        assert!(!segments[0].highlighted);
    }

    #[test]
    fn highlight_label_empty_query() {
        let segments = highlight_label("hello", "");
        assert_eq!(segments.len(), 1);
        assert!(!segments[0].highlighted);
    }

    // --- New tests ---

    #[test]
    fn validate_format_ok() {
        let fmt = LabelFormat {
            pattern: "${filename}".to_string(),
        };
        assert!(validate_format(&fmt).is_ok());
    }

    #[test]
    fn validate_format_no_placeholders() {
        let fmt = LabelFormat {
            pattern: "just text".to_string(),
        };
        assert_eq!(validate_format(&fmt), Err(LabelError::NoPlaceholders));
    }

    #[test]
    fn extract_stem_basic() {
        assert_eq!(extract_stem("/foo/bar/baz.txt"), "baz");
        assert_eq!(extract_stem("noext"), "noext");
        assert_eq!(extract_stem("/a/.hidden"), ".hidden");
    }

    #[test]
    fn path_depth_counts_segments() {
        assert_eq!(path_depth(""), 0);
        assert_eq!(path_depth("file.rs"), 1);
        assert_eq!(path_depth("/home/user/file.rs"), 3);
        assert_eq!(path_depth("/a/b/c/d/"), 4);
    }

    #[test]
    fn common_path_prefix_basic() {
        let paths = vec!["/home/user/a/foo.rs", "/home/user/b/bar.rs"];
        assert_eq!(common_path_prefix(&paths), "/home/user/");
    }

    #[test]
    fn common_path_prefix_empty() {
        assert_eq!(common_path_prefix(&[]), "");
    }

    #[test]
    fn common_path_prefix_no_shared() {
        let paths = vec!["alpha/one.rs", "beta/two.rs"];
        assert_eq!(common_path_prefix(&paths), "");
    }

    #[test]
    fn strip_prefix_removes_prefix() {
        assert_eq!(strip_prefix("/home/user/file.rs", "/home/user/"), "file.rs");
        assert_eq!(strip_prefix("file.rs", "/nope/"), "file.rs");
    }

    #[test]
    fn resource_label_builder_success() {
        let label = ResourceLabel::builder()
            .path("/src/main.rs")
            .description("Entry point")
            .icon("rs-icon")
            .build()
            .unwrap();
        assert_eq!(label.name, "main.rs");
        assert_eq!(label.description.as_deref(), Some("Entry point"));
        assert_eq!(label.icon.as_deref(), Some("rs-icon"));
    }

    #[test]
    fn resource_label_builder_empty_path_error() {
        let result = ResourceLabel::builder().path("").build();
        assert_eq!(result, Err(LabelError::EmptyPath));
    }

    #[test]
    fn resource_label_builder_no_path_error() {
        let result = ResourceLabel::builder().name("test").build();
        assert_eq!(result, Err(LabelError::EmptyPath));
    }

    #[test]
    fn resource_label_display() {
        let label = ResourceLabel {
            name: "main.rs".into(),
            path: "/src/main.rs".into(),
            description: Some("Rust source".into()),
            icon: None,
        };
        assert_eq!(format!("{label}"), "main.rs — Rust source");

        let no_desc = ResourceLabel {
            name: "lib.rs".into(),
            path: "/src/lib.rs".into(),
            description: None,
            icon: None,
        };
        assert_eq!(format!("{no_desc}"), "lib.rs");
    }

    #[test]
    fn resource_label_helpers() {
        let label = ResourceLabel {
            name: "main.rs".into(),
            path: "/home/user/src/main.rs".into(),
            description: None,
            icon: None,
        };
        assert_eq!(label.extension(), Some("rs"));
        assert_eq!(label.parent_dir(), "/home/user/src");
        assert_eq!(label.format(LabelDetail::Short), "main.rs");
    }

    #[test]
    fn label_error_display() {
        assert_eq!(
            LabelError::EmptyPath.to_string(),
            "path must not be empty"
        );
        assert_eq!(
            LabelError::NameTooLong { max: 10, actual: 20 }.to_string(),
            "name length 20 exceeds maximum 10"
        );
        assert_eq!(
            LabelError::UnknownPlaceholder("${bad}".into()).to_string(),
            "unknown placeholder: ${bad}"
        );
    }

    #[test]
    fn label_detail_display() {
        assert_eq!(LabelDetail::Short.to_string(), "short");
        assert_eq!(LabelDetail::Full.to_string(), "full");
    }

    #[test]
    fn label_format_display() {
        let fmt = LabelFormat {
            pattern: "${filename}".to_string(),
        };
        assert_eq!(format!("{fmt}"), "LabelFormat(${filename})");
    }
}
