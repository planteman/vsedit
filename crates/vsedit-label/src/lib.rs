//! Resource label formatting – path manipulation and label templates.

#[derive(Debug, Clone)]
pub struct LabelFormat {
    pub pattern: String,
}

#[derive(Debug, Clone)]
pub struct ResourceLabel {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub icon: Option<String>,
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
}
