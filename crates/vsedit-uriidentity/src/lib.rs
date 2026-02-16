//! URI comparison and normalization.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceUri {
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl ResourceUri {
    pub fn new(scheme: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            scheme: scheme.into(),
            authority: String::new(),
            path: path.into(),
            query: None,
            fragment: None,
        }
    }

    pub fn file(path: impl Into<String>) -> Self {
        Self::new("file", path)
    }

    pub fn from_string(uri_str: &str) -> Option<Self> {
        let (scheme, rest) = uri_str.split_once("://")?;
        let (authority_and_path, fragment) = match rest.rsplit_once('#') {
            Some((ap, f)) => (ap, Some(f.to_string())),
            None => (rest, None),
        };
        let (authority_and_path, query) = match authority_and_path.split_once('?') {
            Some((ap, q)) => (ap, Some(q.to_string())),
            None => (authority_and_path, None),
        };
        let (authority, path) = if scheme == "file" {
            (String::new(), authority_and_path.to_string())
        } else {
            match authority_and_path.find('/') {
                Some(idx) => (
                    authority_and_path[..idx].to_string(),
                    authority_and_path[idx..].to_string(),
                ),
                None => (authority_and_path.to_string(), String::new()),
            }
        };
        Some(Self {
            scheme: scheme.to_string(),
            authority,
            path,
            query,
            fragment,
        })
    }

    pub fn to_string(&self) -> String {
        let mut s = format!("{}://{}{}", self.scheme, self.authority, self.path);
        if let Some(q) = &self.query {
            s.push('?');
            s.push_str(q);
        }
        if let Some(f) = &self.fragment {
            s.push('#');
            s.push_str(f);
        }
        s
    }

    pub fn filename(&self) -> Option<&str> {
        self.path.rsplit('/').next().filter(|s| !s.is_empty())
    }

    pub fn extension(&self) -> Option<&str> {
        self.filename()
            .and_then(|name| name.rsplit_once('.'))
            .map(|(_, ext)| ext)
    }
}

pub fn are_equal(a: &ResourceUri, b: &ResourceUri) -> bool {
    a.scheme.eq_ignore_ascii_case(&b.scheme)
        && a.authority.eq_ignore_ascii_case(&b.authority)
        && a.path.eq_ignore_ascii_case(&b.path)
        && a.query == b.query
        && a.fragment == b.fragment
}

pub struct UriIdentityService {
    mappings: HashMap<String, ResourceUri>,
}

impl UriIdentityService {
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    pub fn register(&mut self, key: impl Into<String>, uri: ResourceUri) {
        self.mappings.insert(key.into(), uri);
    }

    pub fn resolve(&self, key: &str) -> Option<&ResourceUri> {
        self.mappings.get(key)
    }
}

impl Default for UriIdentityService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_parts() {
        let uri = ResourceUri::file("/home/user/file.rs");
        assert_eq!(uri.scheme, "file");
        assert_eq!(uri.filename(), Some("file.rs"));
        assert_eq!(uri.extension(), Some("rs"));
    }

    #[test]
    fn parse_and_roundtrip() {
        let uri = ResourceUri::from_string("https://example.com/path?q=1#frag").unwrap();
        assert_eq!(uri.scheme, "https");
        assert_eq!(uri.authority, "example.com");
        assert_eq!(uri.path, "/path");
        assert_eq!(uri.query.as_deref(), Some("q=1"));
        assert_eq!(uri.fragment.as_deref(), Some("frag"));
        assert_eq!(uri.to_string(), "https://example.com/path?q=1#frag");
    }

    #[test]
    fn case_insensitive_equality() {
        let a = ResourceUri::file("/Home/User/File.rs");
        let b = ResourceUri::file("/home/user/file.rs");
        assert!(are_equal(&a, &b));
    }

    #[test]
    fn identity_service() {
        let mut svc = UriIdentityService::new();
        svc.register("main", ResourceUri::file("/src/main.rs"));
        assert!(svc.resolve("main").is_some());
        assert!(svc.resolve("other").is_none());
    }
}
