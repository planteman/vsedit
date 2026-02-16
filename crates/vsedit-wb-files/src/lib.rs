//! Text file operations.

/// The type of a file system entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    SymbolicLink,
    Unknown,
}

/// Metadata about a file system entry.
#[derive(Debug, Clone)]
pub struct FileStat {
    pub file_type: FileType,
    pub size: u64,
    pub modified: u64,
    pub created: u64,
    pub readonly: bool,
}

/// An event produced by a file watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    Created(String),
    Changed(String),
    Deleted(String),
}

/// Configuration for a single file watcher.
#[derive(Debug, Clone)]
pub struct FileWatcherConfig {
    pub glob_pattern: String,
    pub recursive: bool,
    pub exclude_patterns: Vec<String>,
}

/// Service for files workbench functionality.
#[derive(Debug, Clone)]
pub struct FileService {
    pub watchers: Vec<FileWatcherConfig>,
    pub events: Vec<FileEvent>,
}

impl FileService {
    pub fn new() -> Self {
        Self {
            watchers: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn add_watcher(&mut self, config: FileWatcherConfig) {
        self.watchers.push(config);
    }

    pub fn remove_watcher(&mut self, pattern: &str) -> bool {
        let len = self.watchers.len();
        self.watchers.retain(|w| w.glob_pattern != pattern);
        self.watchers.len() != len
    }

    pub fn record_event(&mut self, event: FileEvent) {
        self.events.push(event);
    }

    pub fn get_events(&self) -> &[FileEvent] {
        &self.events
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Basic glob matching supporting `*` as a wildcard for any sequence of characters.
    pub fn matches_watcher(&self, path: &str) -> bool {
        self.watchers.iter().any(|w| glob_match(&w.glob_pattern, path))
    }

    pub fn get_events_for_uri(&self, uri: &str) -> Vec<&FileEvent> {
        self.events
            .iter()
            .filter(|e| match e {
                FileEvent::Created(u) | FileEvent::Changed(u) | FileEvent::Deleted(u) => u == uri,
            })
            .collect()
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    pub fn watcher_count(&self) -> usize {
        self.watchers.len()
    }

    pub fn get_stats(&self) -> FileServiceStats {
        let created = self.events.iter().filter(|e| matches!(e, FileEvent::Created(_))).count();
        let changed = self.events.iter().filter(|e| matches!(e, FileEvent::Changed(_))).count();
        let deleted = self.events.iter().filter(|e| matches!(e, FileEvent::Deleted(_))).count();
        FileServiceStats {
            total_events: self.events.len(),
            created_count: created,
            changed_count: changed,
            deleted_count: deleted,
            watcher_count: self.watchers.len(),
        }
    }
}

impl Default for FileService {
    fn default() -> Self {
        Self::new()
    }
}

/// Match `path` against a simple glob `pattern` where `*` matches any sequence of characters.
fn glob_match(pattern: &str, path: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == path;
    }

    let mut remaining = path;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if let Some(rest) = remaining.strip_prefix(part) {
                remaining = rest;
            } else {
                return false;
            }
        } else if i == parts.len() - 1 {
            if !remaining.ends_with(part) {
                return false;
            }
            remaining = "";
        } else {
            if let Some(pos) = remaining.find(part) {
                remaining = &remaining[pos + part.len()..];
            } else {
                return false;
            }
        }
    }

    true
}

impl FileWatcherConfig {
    pub fn is_excluded(&self, path: &str) -> bool {
        self.exclude_patterns.iter().any(|p| glob_match(p, path))
    }
}

impl FileStat {
    pub fn is_file(&self) -> bool {
        self.file_type == FileType::File
    }

    pub fn is_directory(&self) -> bool {
        self.file_type == FileType::Directory
    }

    pub fn is_symlink(&self) -> bool {
        self.file_type == FileType::SymbolicLink
    }

    pub fn age_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.modified)
    }
}

#[derive(Debug, Clone)]
pub struct FileServiceStats {
    pub total_events: usize,
    pub created_count: usize,
    pub changed_count: usize,
    pub deleted_count: usize,
    pub watcher_count: usize,
}

pub trait FileSystemProvider {
    fn stat(&self, uri: &str) -> Option<FileStat>;

    fn read_file(&self, uri: &str) -> Option<Vec<u8>> {
        let _ = uri;
        None
    }

    fn write_file(&self, uri: &str, content: &[u8]) -> bool {
        let _ = (uri, content);
        false
    }

    fn delete(&self, uri: &str) -> bool {
        let _ = uri;
        false
    }

    fn rename(&self, old_uri: &str, new_uri: &str) -> bool {
        let _ = (old_uri, new_uri);
        false
    }

    fn create_directory(&self, uri: &str) -> bool {
        let _ = uri;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_watchers() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: true,
            exclude_patterns: vec![],
        });
        assert_eq!(svc.watchers.len(), 1);
        assert!(svc.remove_watcher("*.rs"));
        assert!(svc.watchers.is_empty());
        assert!(!svc.remove_watcher("*.rs"));
    }

    #[test]
    fn record_and_clear_events() {
        let mut svc = FileService::new();
        svc.record_event(FileEvent::Created("a.txt".into()));
        svc.record_event(FileEvent::Changed("a.txt".into()));
        svc.record_event(FileEvent::Deleted("b.txt".into()));
        assert_eq!(svc.get_events().len(), 3);
        svc.clear_events();
        assert!(svc.get_events().is_empty());
    }

    #[test]
    fn matches_watcher_glob() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: false,
            exclude_patterns: vec![],
        });
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "src/*/*.ts".into(),
            recursive: true,
            exclude_patterns: vec![],
        });
        assert!(svc.matches_watcher("main.rs"));
        assert!(svc.matches_watcher("lib.rs"));
        assert!(!svc.matches_watcher("main.py"));
        assert!(svc.matches_watcher("src/components/app.ts"));
        assert!(!svc.matches_watcher("src/app.py"));
    }

    #[test]
    fn is_excluded_matches_patterns() {
        let config = FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: true,
            exclude_patterns: vec!["*.test.rs".into(), "target/*".into()],
        };
        assert!(config.is_excluded("foo.test.rs"));
        assert!(config.is_excluded("target/debug.rs"));
        assert!(!config.is_excluded("main.rs"));
    }

    #[test]
    fn get_events_for_uri_filters() {
        let mut svc = FileService::new();
        svc.record_event(FileEvent::Created("a.txt".into()));
        svc.record_event(FileEvent::Changed("a.txt".into()));
        svc.record_event(FileEvent::Deleted("b.txt".into()));
        let events = svc.get_events_for_uri("a.txt");
        assert_eq!(events.len(), 2);
        assert_eq!(svc.get_events_for_uri("b.txt").len(), 1);
        assert_eq!(svc.get_events_for_uri("c.txt").len(), 0);
    }

    #[test]
    fn event_and_watcher_counts() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: false,
            exclude_patterns: vec![],
        });
        svc.record_event(FileEvent::Created("a.rs".into()));
        assert_eq!(svc.event_count(), 1);
        assert_eq!(svc.watcher_count(), 1);
    }

    #[test]
    fn file_service_stats() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: false,
            exclude_patterns: vec![],
        });
        svc.record_event(FileEvent::Created("a.rs".into()));
        svc.record_event(FileEvent::Created("b.rs".into()));
        svc.record_event(FileEvent::Changed("a.rs".into()));
        svc.record_event(FileEvent::Deleted("c.rs".into()));
        let stats = svc.get_stats();
        assert_eq!(stats.total_events, 4);
        assert_eq!(stats.created_count, 2);
        assert_eq!(stats.changed_count, 1);
        assert_eq!(stats.deleted_count, 1);
        assert_eq!(stats.watcher_count, 1);
    }

    #[test]
    fn file_stat_type_checks() {
        let stat = FileStat {
            file_type: FileType::File,
            size: 100,
            modified: 1000,
            created: 900,
            readonly: false,
        };
        assert!(stat.is_file());
        assert!(!stat.is_directory());
        assert!(!stat.is_symlink());

        let dir = FileStat { file_type: FileType::Directory, ..stat.clone() };
        assert!(dir.is_directory());

        let link = FileStat { file_type: FileType::SymbolicLink, ..stat };
        assert!(link.is_symlink());
    }

    #[test]
    fn file_stat_age_seconds() {
        let stat = FileStat {
            file_type: FileType::File,
            size: 50,
            modified: 1000,
            created: 900,
            readonly: false,
        };
        assert_eq!(stat.age_seconds(1500), 500);
        assert_eq!(stat.age_seconds(500), 0);
    }

    #[test]
    fn file_system_provider_defaults() {
        struct NullProvider;
        impl FileSystemProvider for NullProvider {
            fn stat(&self, _uri: &str) -> Option<FileStat> {
                None
            }
        }
        let provider = NullProvider;
        assert!(provider.stat("file:///a").is_none());
        assert!(provider.read_file("file:///a").is_none());
        assert!(!provider.write_file("file:///a", b"data"));
        assert!(!provider.delete("file:///a"));
        assert!(!provider.rename("file:///a", "file:///b"));
        assert!(!provider.create_directory("file:///dir"));
    }

    #[test]
    fn file_stat_clone_and_debug() {
        let stat = FileStat {
            file_type: FileType::File,
            size: 42,
            modified: 100,
            created: 50,
            readonly: true,
        };
        let cloned = stat.clone();
        assert_eq!(cloned.size, 42);
        assert!(cloned.readonly);
        let debug = format!("{:?}", cloned);
        assert!(debug.contains("FileStat"));
    }
}
