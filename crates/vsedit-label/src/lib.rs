//! Resource label formatting – path manipulation and label templates.

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct ResourceLabel {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub icon: Option<String>,
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
}
