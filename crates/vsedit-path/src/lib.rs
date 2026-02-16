//! Cross-platform path manipulation.
//!
//! Equivalent to VS Code's `vs/base/common/path.ts`.

use std::fmt;
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

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by path operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// The path string was empty.
    EmptyPath,
    /// The path contained invalid characters or structure.
    InvalidPath(String),
    /// A relative path was expected but an absolute path was given.
    RelativePathExpected,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::EmptyPath => write!(f, "path is empty"),
            PathError::InvalidPath(detail) => write!(f, "invalid path: {detail}"),
            PathError::RelativePathExpected => write!(f, "expected a relative path"),
        }
    }
}

impl std::error::Error for PathError {}

// ---------------------------------------------------------------------------
// Additional path helpers
// ---------------------------------------------------------------------------

/// Return the file stem (filename without extension).
///
/// ```
/// assert_eq!(vsedit_path::stem("a/b/file.tar.gz"), "file.tar");
/// assert_eq!(vsedit_path::stem("noext"), "noext");
/// ```
pub fn stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Replace (or add) the extension of a path.
///
/// The new extension should be provided **without** the leading dot.
/// Pass an empty string to remove the extension.
///
/// ```
/// assert_eq!(vsedit_path::change_extension("a/b/file.rs", "txt"), "a/b/file.txt");
/// ```
pub fn change_extension(path: &str, new_ext: &str) -> String {
    let mut buf = PathBuf::from(path);
    buf.set_extension(new_ext);
    buf.to_string_lossy().into_owned()
}

/// Check whether `child` is a descendant of `parent`.
///
/// Both paths are normalised with forward slashes before comparison so the
/// check works regardless of separator style.
pub fn is_child_of(child: &str, parent: &str) -> bool {
    let norm = |p: &str| {
        let n = normalize(p);
        let mut s = to_forward_slashes(&n);
        if !s.ends_with('/') {
            s.push('/');
        }
        s
    };
    let parent_norm = norm(parent);
    let child_fwd = to_forward_slashes(&normalize(child));
    child_fwd.starts_with(&parent_norm) && child_fwd.len() > parent_norm.len() - 1
}

/// Return the longest common directory prefix of two paths.
///
/// The result always uses forward slashes and never has a trailing separator
/// (unless the common prefix is a root like `/`).
pub fn common_prefix(a: &str, b: &str) -> String {
    let a_parts: Vec<&str> = to_forward_slashes(&normalize(a))
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect::<Vec<_>>()
        .into_iter()
        .map(|s| Box::leak(s.into_boxed_str()) as &str)
        .collect();
    let b_norm = to_forward_slashes(&normalize(b));
    let b_parts: Vec<&str> = b_norm.split('/').filter(|s| !s.is_empty()).collect();

    let common: Vec<&str> = a_parts
        .iter()
        .zip(b_parts.iter())
        .take_while(|(x, y)| x == y)
        .map(|(x, _)| *x)
        .collect();

    common.join("/")
}

/// Add a trailing separator if one is not already present.
pub fn ensure_trailing_separator(path: &str) -> String {
    if path.is_empty() || has_trailing_separator(path) {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

/// Convert a path to lowercase for case-insensitive comparison.
pub fn normalize_case(path: &str) -> String {
    to_forward_slashes(path).to_lowercase()
}

/// Check whether a path is a UNC path (`\\server\share`).
pub fn is_unc_path(path: &str) -> bool {
    let s = path.as_bytes();
    s.len() >= 3
        && (s[0] == b'\\' && s[1] == b'\\' && s[2] != b'\\')
}

// ---------------------------------------------------------------------------
// PathComponents
// ---------------------------------------------------------------------------

/// Parsed components of a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathComponents {
    /// Root portion (e.g. `/`, `C:\`, or empty for relative paths).
    pub root: String,
    /// Directory segments between root and filename.
    pub dir_parts: Vec<String>,
    /// Filename without extension.
    pub stem: String,
    /// Extension **with** the leading dot, or empty.
    pub extension: String,
}

impl PathComponents {
    /// Parse a path string into its components.
    pub fn parse(path: &str) -> Self {
        let p = Path::new(path);

        // Root
        let root = {
            let mut components = p.components();
            match components.next() {
                Some(std::path::Component::Prefix(pre)) => {
                    let prefix = pre.as_os_str().to_string_lossy().into_owned();
                    // Check if a RootDir follows
                    if let Some(std::path::Component::RootDir) = components.next() {
                        format!("{prefix}{}", MAIN_SEPARATOR)
                    } else {
                        prefix
                    }
                }
                Some(std::path::Component::RootDir) => "/".to_string(),
                _ => String::new(),
            }
        };

        // Collect normal components
        let normal_parts: Vec<String> = p
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => {
                    Some(s.to_string_lossy().into_owned())
                }
                _ => None,
            })
            .collect();

        let (dir_parts, file_part) = if normal_parts.is_empty() {
            (Vec::new(), None)
        } else {
            let (dirs, file) = normal_parts.split_at(normal_parts.len() - 1);
            (dirs.to_vec(), file.first().cloned())
        };

        let stem_val = file_part
            .as_deref()
            .and_then(|f| Path::new(f).file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let ext_val = file_part
            .as_deref()
            .and_then(|f| Path::new(f).extension())
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        Self {
            root,
            dir_parts,
            stem: stem_val,
            extension: ext_val,
        }
    }
}

impl fmt::Display for PathComponents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)?;
        for (i, part) in self.dir_parts.iter().enumerate() {
            if i > 0 || !self.root.is_empty() {
                write!(f, "/")?;
            }
            write!(f, "{part}")?;
        }
        if !self.dir_parts.is_empty() || !self.root.is_empty() {
            write!(f, "/")?;
        }
        write!(f, "{}{}", self.stem, self.extension)
    }
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

    #[test]
    fn test_stem() {
        assert_eq!(stem("a/b/file.rs"), "file");
        assert_eq!(stem("archive.tar.gz"), "archive.tar");
        assert_eq!(stem("noext"), "noext");
        assert_eq!(stem(""), "");
    }

    #[test]
    fn test_change_extension() {
        assert_eq!(change_extension("a/b/file.rs", "txt"), "a/b/file.txt");
        assert_eq!(change_extension("noext", "md"), "noext.md");
        // remove extension
        let without = change_extension("a/b/file.rs", "");
        assert!(!without.ends_with('.'));
    }

    #[test]
    fn test_is_child_of() {
        assert!(is_child_of("a/b/c", "a/b"));
        assert!(is_child_of("a/b/c/d", "a"));
        assert!(!is_child_of("a/b", "a/b"));
        assert!(!is_child_of("a/bc", "a/b"));
    }

    #[test]
    fn test_common_prefix() {
        assert_eq!(common_prefix("a/b/c", "a/b/d"), "a/b");
        assert_eq!(common_prefix("x/y", "a/b"), "");
        assert_eq!(common_prefix("a/b/c", "a/b/c"), "a/b/c");
    }

    #[test]
    fn test_ensure_trailing_separator() {
        assert_eq!(ensure_trailing_separator("path/to"), "path/to/");
        assert_eq!(ensure_trailing_separator("path/to/"), "path/to/");
        assert_eq!(ensure_trailing_separator(""), "");
    }

    #[test]
    fn test_path_components() {
        let pc = PathComponents::parse("a/b/file.rs");
        assert_eq!(pc.root, "");
        assert_eq!(pc.dir_parts, vec!["a", "b"]);
        assert_eq!(pc.stem, "file");
        assert_eq!(pc.extension, ".rs");

        let pc2 = PathComponents::parse("/usr/local/bin/tool");
        assert_eq!(pc2.root, "/");
        assert_eq!(pc2.dir_parts, vec!["usr", "local", "bin"]);
        assert_eq!(pc2.stem, "tool");
        assert_eq!(pc2.extension, "");
    }

    #[test]
    fn test_path_components_display() {
        let pc = PathComponents::parse("a/b/file.rs");
        assert_eq!(pc.to_string(), "a/b/file.rs");
    }

    #[test]
    fn test_normalize_case() {
        assert_eq!(normalize_case("A/B/File.RS"), "a/b/file.rs");
        assert_eq!(normalize_case("C:\\Users\\Foo"), "c:/users/foo");
    }

    #[test]
    fn test_is_unc_path() {
        assert!(is_unc_path("\\\\server\\share"));
        assert!(!is_unc_path("/normal/path"));
        assert!(!is_unc_path("\\not_unc"));
        assert!(!is_unc_path("\\\\\\triple"));
    }

    #[test]
    fn test_path_error_display() {
        assert_eq!(PathError::EmptyPath.to_string(), "path is empty");
        assert_eq!(
            PathError::InvalidPath("bad".into()).to_string(),
            "invalid path: bad"
        );
        assert_eq!(
            PathError::RelativePathExpected.to_string(),
            "expected a relative path"
        );
    }
}
