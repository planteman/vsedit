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
}
