//! Cross-platform path manipulation.
//!
//! Equivalent to VS Code's `vs/base/common/path.ts`.

use std::path::{Path, PathBuf, MAIN_SEPARATOR};

/// The platform path separator.
pub const SEP: char = MAIN_SEPARATOR;

/// Normalize a path by resolving `.` and `..` segments.
pub fn normalize(path: &str) -> String {
    let p = PathBuf::from(path);
    let mut components = Vec::new();
    for component in p.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            other => components.push(other),
        }
    }
    let result: PathBuf = components.iter().collect();
    result.to_string_lossy().into_owned()
}

/// Join path segments.
pub fn join(base: &str, segments: &[&str]) -> String {
    let mut path = PathBuf::from(base);
    for seg in segments {
        path.push(seg);
    }
    path.to_string_lossy().into_owned()
}

/// Get the directory name of a path.
pub fn dirname(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Get the base name (file name) of a path.
pub fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Get the file extension (without the dot).
pub fn extname(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default()
}

/// Check if a path is absolute.
pub fn is_absolute(path: &str) -> bool {
    Path::new(path).is_absolute()
}

/// Compute a relative path from `from` to `to`.
pub fn relative(from: &str, to: &str) -> String {
    let from_path = PathBuf::from(from);
    let to_path = PathBuf::from(to);

    let from_components: Vec<_> = from_path.components().collect();
    let to_components: Vec<_> = to_path.components().collect();

    let common_len = from_components
        .iter()
        .zip(to_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let ups = from_components.len() - common_len;
    let mut result = PathBuf::new();
    for _ in 0..ups {
        result.push("..");
    }
    for component in &to_components[common_len..] {
        result.push(component);
    }
    result.to_string_lossy().into_owned()
}

/// Convert backslashes to forward slashes (for cross-platform normalization).
pub fn to_forward_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

/// Convert forward slashes to backslashes (Windows-style).
pub fn to_back_slashes(path: &str) -> String {
    path.replace('/', "\\")
}

/// Remove trailing path separator.
pub fn remove_trailing_separator(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
}

/// Check if a path has a trailing separator.
pub fn has_trailing_separator(path: &str) -> bool {
    path.ends_with('/') || path.ends_with('\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("a/b/../c"), "a/c");
        assert_eq!(normalize("a/./b/c"), "a/b/c");
    }

    #[test]
    fn test_join() {
        let result = join("a", &["b", "c"]);
        assert!(result == "a/b/c" || result == "a\\b\\c");
    }

    #[test]
    fn test_dirname() {
        assert_eq!(dirname("a/b/c.txt"), "a/b");
    }

    #[test]
    fn test_basename() {
        assert_eq!(basename("a/b/c.txt"), "c.txt");
    }

    #[test]
    fn test_extname() {
        assert_eq!(extname("file.rs"), ".rs");
        assert_eq!(extname("file"), "");
    }

    #[test]
    fn test_to_forward_slashes() {
        assert_eq!(to_forward_slashes("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn test_remove_trailing_separator() {
        assert_eq!(remove_trailing_separator("path/to/dir/"), "path/to/dir");
    }
}
