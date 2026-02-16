//! Platform path resolution.

/// Path separator style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSeparator {
    Unix,
    Windows,
}

/// Service for path manipulation.
pub struct PathService {
    separator: PathSeparator,
}

impl PathService {
    pub fn new(separator: PathSeparator) -> Self {
        Self { separator }
    }

    pub fn join(&self, base: &str, path: &str) -> String {
        if self.is_absolute(path) {
            return path.to_string();
        }
        let sep = self.sep_char();
        let base = base.trim_end_matches(|c| c == '/' || c == '\\');
        format!("{base}{sep}{path}")
    }

    /// Normalize a path by resolving `.` and `..` segments and collapsing separators.
    pub fn normalize(&self, path: &str) -> String {
        let sep = self.sep_char();
        let is_abs = self.is_absolute(path);
        let parts: Vec<&str> = path
            .split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty() && *s != ".")
            .collect();
        let mut stack: Vec<&str> = Vec::new();
        for part in &parts {
            if *part == ".." {
                if let Some(top) = stack.last() {
                    if *top != ".." {
                        stack.pop();
                        continue;
                    }
                }
                if !is_abs {
                    stack.push(part);
                }
            } else {
                stack.push(part);
            }
        }
        let joined = stack.join(&sep.to_string());
        if is_abs {
            // Preserve Windows drive prefix like C:
            let prefix = self.abs_prefix(path);
            format!("{prefix}{sep}{joined}")
        } else if joined.is_empty() {
            ".".to_string()
        } else {
            joined
        }
    }

    pub fn dirname<'a>(&self, path: &'a str) -> &'a str {
        let path = path.trim_end_matches(|c| c == '/' || c == '\\');
        match path.rfind(|c: char| c == '/' || c == '\\') {
            Some(0) => &path[..1],
            Some(i) => &path[..i],
            None => ".",
        }
    }

    pub fn basename<'a>(&self, path: &'a str) -> &'a str {
        let path = path.trim_end_matches(|c| c == '/' || c == '\\');
        match path.rfind(|c: char| c == '/' || c == '\\') {
            Some(i) => &path[i + 1..],
            None => path,
        }
    }

    pub fn extname<'a>(&self, path: &'a str) -> Option<&'a str> {
        let base = self.basename(path);
        match base.rfind('.') {
            Some(0) | None => None,
            Some(i) => Some(&base[i..]),
        }
    }

    pub fn is_absolute(&self, path: &str) -> bool {
        match self.separator {
            PathSeparator::Unix => path.starts_with('/'),
            PathSeparator::Windows => {
                path.starts_with('/')
                    || path.starts_with('\\')
                    || (path.len() >= 3
                        && path.as_bytes()[0].is_ascii_alphabetic()
                        && path.as_bytes()[1] == b':'
                        && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/'))
            }
        }
    }

    pub fn to_unix(&self, path: &str) -> String {
        path.replace('\\', "/")
    }

    pub fn to_windows(&self, path: &str) -> String {
        path.replace('/', "\\")
    }

    pub fn resolve_relative(&self, base: &str, relative: &str) -> String {
        let dir = self.dirname(base);
        let joined = self.join(dir, relative);
        self.normalize(&joined)
    }

    fn sep_char(&self) -> char {
        match self.separator {
            PathSeparator::Unix => '/',
            PathSeparator::Windows => '\\',
        }
    }

    fn abs_prefix(&self, path: &str) -> String {
        match self.separator {
            PathSeparator::Unix => String::new(),
            PathSeparator::Windows => {
                if path.len() >= 2
                    && path.as_bytes()[0].is_ascii_alphabetic()
                    && path.as_bytes()[1] == b':'
                {
                    path[..2].to_string()
                } else {
                    String::new()
                }
            }
        }
    }

    /// Compute a relative path from `from` to `to`.
    pub fn relative(&self, from: &str, to: &str) -> String {
        let sep = self.sep_char();
        let norm_from = self.normalize(from);
        let norm_to = self.normalize(to);
        let from_parts: Vec<&str> = norm_from
            .split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .collect();
        let to_parts: Vec<&str> = norm_to
            .split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .collect();
        let common = from_parts
            .iter()
            .zip(to_parts.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let ups = from_parts.len() - common;
        let mut result: Vec<&str> = Vec::new();
        for _ in 0..ups {
            result.push("..");
        }
        for part in &to_parts[common..] {
            result.push(part);
        }
        if result.is_empty() {
            ".".to_string()
        } else {
            result.join(&sep.to_string())
        }
    }

    /// Check whether `child` is a descendant of `parent`.
    pub fn is_child_of(&self, child: &str, parent: &str) -> bool {
        let norm_child = self.normalize(child);
        let norm_parent = self.normalize(parent);
        let sep = self.sep_char();
        let parent_prefix = if norm_parent.ends_with(sep) {
            norm_parent.clone()
        } else {
            format!("{norm_parent}{sep}")
        };
        norm_child.starts_with(&parent_prefix) && norm_child.len() > parent_prefix.len()
    }

    /// Return the longest common path prefix among the given paths.
    pub fn common_prefix(&self, paths: &[&str]) -> String {
        if paths.is_empty() {
            return String::new();
        }
        let sep = self.sep_char();
        let normalized: Vec<String> = paths.iter().map(|p| self.normalize(p)).collect();
        let first_parts: Vec<&str> = normalized[0]
            .split(|c: char| c == '/' || c == '\\')
            .collect();
        let mut prefix_len = first_parts.len();
        for path in &normalized[1..] {
            let parts: Vec<&str> = path.split(|c: char| c == '/' || c == '\\').collect();
            let common = first_parts
                .iter()
                .zip(parts.iter())
                .take_while(|(a, b)| a == b)
                .count();
            prefix_len = prefix_len.min(common);
        }
        let result = first_parts[..prefix_len].join(&sep.to_string());
        if result.is_empty() && self.is_absolute(&normalized[0]) {
            sep.to_string()
        } else {
            result
        }
    }

    /// Replace the extension of `path` with `ext`.
    pub fn with_extension(&self, path: &str, ext: &str) -> String {
        let stripped = self.strip_extension(path);
        if ext.is_empty() {
            stripped
        } else if ext.starts_with('.') {
            format!("{stripped}{ext}")
        } else {
            format!("{stripped}.{ext}")
        }
    }

    /// Remove the file extension from `path`.
    pub fn strip_extension(&self, path: &str) -> String {
        let base = self.basename(path);
        match base.rfind('.') {
            Some(0) | None => path.to_string(),
            Some(i) => {
                let dir = self.dirname(path);
                let stem = &base[..i];
                if dir == "." {
                    stem.to_string()
                } else {
                    let sep = self.sep_char();
                    format!("{dir}{sep}{stem}")
                }
            }
        }
    }

    /// Split a path into its individual components.
    pub fn components(&self, path: &str) -> Vec<String> {
        path.split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    /// Return the depth (number of components) of a path.
    pub fn depth(&self, path: &str) -> usize {
        self.components(path).len()
    }

    /// Check whether `path` represents a root directory.
    pub fn is_root(&self, path: &str) -> bool {
        match self.separator {
            PathSeparator::Unix => path == "/",
            PathSeparator::Windows => {
                path == "/" || path == "\\"
                    || (path.len() == 3
                        && path.as_bytes()[0].is_ascii_alphabetic()
                        && path.as_bytes()[1] == b':'
                        && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/'))
            }
        }
    }

    /// Make a relative `path` absolute by joining it with `cwd`.
    pub fn make_absolute(&self, path: &str, cwd: &str) -> String {
        if self.is_absolute(path) {
            self.normalize(path)
        } else {
            let joined = self.join(cwd, path);
            self.normalize(&joined)
        }
    }
}

impl Default for PathService {
    fn default() -> Self {
        Self::new(PathSeparator::Unix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_normalize() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.normalize("/a/b/../c/./d"), "/a/c/d");
        assert_eq!(svc.normalize("a//b"), "a/b");
    }

    #[test]
    fn basename_extname() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.basename("/foo/bar.txt"), "bar.txt");
        assert_eq!(svc.extname("/foo/bar.txt"), Some(".txt"));
        assert_eq!(svc.extname("/foo/.hidden"), None);
        assert_eq!(svc.dirname("/foo/bar.txt"), "/foo");
    }

    #[test]
    fn windows_paths() {
        let svc = PathService::new(PathSeparator::Windows);
        assert!(svc.is_absolute("C:\\Users"));
        assert!(!svc.is_absolute("relative\\path"));
        assert_eq!(svc.to_unix("C:\\a\\b"), "C:/a/b");
    }

    #[test]
    fn resolve_relative() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.resolve_relative("/a/b/c.txt", "../d.txt"), "/a/d.txt");
    }

    #[test]
    fn relative_path() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.relative("/a/b", "/a/c/d"), "../c/d");
        assert_eq!(svc.relative("/a/b/c", "/a/b/c"), ".");
        assert_eq!(svc.relative("/a", "/a/b/c"), "b/c");
    }

    #[test]
    fn is_child_of_check() {
        let svc = PathService::new(PathSeparator::Unix);
        assert!(svc.is_child_of("/a/b/c", "/a/b"));
        assert!(svc.is_child_of("/a/b/c/d", "/a"));
        assert!(!svc.is_child_of("/a/b", "/a/b"));
        assert!(!svc.is_child_of("/x/y", "/a/b"));
    }

    #[test]
    fn common_prefix_paths() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.common_prefix(&["/a/b/c", "/a/b/d", "/a/b/e"]), "/a/b");
        assert_eq!(svc.common_prefix(&["/a/b", "/c/d"]), "/");
        assert_eq!(svc.common_prefix(&[]), "");
    }

    #[test]
    fn with_and_strip_extension() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.with_extension("/a/b.txt", "rs"), "/a/b.rs");
        assert_eq!(svc.with_extension("/a/b.txt", ".rs"), "/a/b.rs");
        assert_eq!(svc.strip_extension("/a/b.txt"), "/a/b");
        assert_eq!(svc.strip_extension("/a/.hidden"), "/a/.hidden");
    }

    #[test]
    fn components_and_depth() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.components("/a/b/c"), vec!["a", "b", "c"]);
        assert_eq!(svc.depth("/a/b/c"), 3);
        assert_eq!(svc.depth("a"), 1);
    }

    #[test]
    fn is_root_check() {
        let unix = PathService::new(PathSeparator::Unix);
        assert!(unix.is_root("/"));
        assert!(!unix.is_root("/a"));

        let win = PathService::new(PathSeparator::Windows);
        assert!(win.is_root("C:\\"));
        assert!(!win.is_root("C:\\Users"));
    }

    #[test]
    fn make_absolute_path() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.make_absolute("b/c", "/a"), "/a/b/c");
        assert_eq!(svc.make_absolute("/x/y", "/a"), "/x/y");
    }

    #[test]
    fn with_extension_empty() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.with_extension("/a/b.txt", ""), "/a/b");
    }

    #[test]
    fn relative_windows() {
        let svc = PathService::new(PathSeparator::Windows);
        assert_eq!(svc.relative("C:\\a\\b", "C:\\a\\c\\d"), "..\\c\\d");
    }

    #[test]
    fn depth_edge_cases() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.depth("/"), 0);
        assert_eq!(svc.depth("//"), 0);
        assert_eq!(svc.depth("/a/b/c/d/e"), 5);
    }
}
