//! File system watcher backed by the `notify` crate.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use notify::{
    Config, Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use tokio::sync::mpsc;

use crate::{FileChangeEvent, FileChangeType};
use vsedit_uri::VsUri;

/// The kind of file change detected by the watcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
    Renamed,
}

impl From<FileChangeKind> for FileChangeType {
    fn from(kind: FileChangeKind) -> Self {
        match kind {
            FileChangeKind::Created => FileChangeType::Created,
            FileChangeKind::Modified => FileChangeType::Changed,
            FileChangeKind::Deleted => FileChangeType::Deleted,
            FileChangeKind::Renamed => FileChangeType::Changed,
        }
    }
}

/// A single file-change event produced by [`FileWatcher`].
#[derive(Debug, Clone)]
pub struct WatcherEvent {
    pub path: PathBuf,
    pub kind: FileChangeKind,
}

/// Watches directories for file-system changes using `notify`.
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    watched: Arc<Mutex<HashSet<PathBuf>>>,
    rx: mpsc::UnboundedReceiver<WatcherEvent>,
}

/// Error type for watch operations.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),
    #[error("path not watched: {0}")]
    NotWatched(PathBuf),
}

pub type WatchResult<T> = Result<T, WatchError>;

fn classify_event(kind: &EventKind) -> Option<FileChangeKind> {
    match kind {
        EventKind::Create(_) => Some(FileChangeKind::Created),
        EventKind::Modify(notify::event::ModifyKind::Name(_)) => Some(FileChangeKind::Renamed),
        EventKind::Modify(_) => Some(FileChangeKind::Modified),
        EventKind::Remove(_) => Some(FileChangeKind::Deleted),
        _ => None,
    }
}

impl FileWatcher {
    /// Create a new [`FileWatcher`].
    pub fn new() -> WatchResult<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let watcher = RecommendedWatcher::new(
            move |res: Result<NotifyEvent, notify::Error>| {
                if let Ok(event) = res {
                    if let Some(kind) = classify_event(&event.kind) {
                        for path in &event.paths {
                            let _ = tx.send(WatcherEvent {
                                path: path.clone(),
                                kind,
                            });
                        }
                    }
                }
            },
            Config::default(),
        )?;

        Ok(Self {
            watcher,
            watched: Arc::new(Mutex::new(HashSet::new())),
            rx,
        })
    }

    /// Start watching a path recursively.
    pub fn watch(&mut self, path: &Path) -> WatchResult<()> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        self.watched.lock().unwrap().insert(path.to_path_buf());
        Ok(())
    }

    /// Stop watching a path.
    pub fn unwatch(&mut self, path: &Path) -> WatchResult<()> {
        let mut watched = self.watched.lock().unwrap();
        if !watched.remove(path) {
            return Err(WatchError::NotWatched(path.to_path_buf()));
        }
        self.watcher.unwatch(path)?;
        Ok(())
    }

    /// Returns the set of currently watched paths.
    pub fn watched_paths(&self) -> HashSet<PathBuf> {
        self.watched.lock().unwrap().clone()
    }

    /// Receive the next change event (async).
    pub async fn on_change(&mut self) -> Option<WatcherEvent> {
        self.rx.recv().await
    }

    /// Try to receive a change event without blocking.
    pub fn try_recv(&mut self) -> Option<WatcherEvent> {
        self.rx.try_recv().ok()
    }

    /// Convert a [`WatcherEvent`] into a [`FileChangeEvent`].
    pub fn to_file_change_event(event: &WatcherEvent) -> FileChangeEvent {
        FileChangeEvent {
            uri: VsUri::file(&event.path.to_string_lossy()),
            change_type: event.kind.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    #[test]
    fn classify_create() {
        let kind = EventKind::Create(notify::event::CreateKind::File);
        assert_eq!(classify_event(&kind), Some(FileChangeKind::Created));
    }

    #[test]
    fn classify_modify() {
        let kind = EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Content,
        ));
        assert_eq!(classify_event(&kind), Some(FileChangeKind::Modified));
    }

    #[test]
    fn classify_rename() {
        let kind = EventKind::Modify(notify::event::ModifyKind::Name(
            notify::event::RenameMode::Both,
        ));
        assert_eq!(classify_event(&kind), Some(FileChangeKind::Renamed));
    }

    #[test]
    fn classify_delete() {
        let kind = EventKind::Remove(notify::event::RemoveKind::File);
        assert_eq!(classify_event(&kind), Some(FileChangeKind::Deleted));
    }

    #[test]
    fn classify_other_returns_none() {
        let kind = EventKind::Other;
        assert_eq!(classify_event(&kind), None);
    }

    #[test]
    fn file_change_kind_into_file_change_type() {
        assert_eq!(FileChangeType::from(FileChangeKind::Created), FileChangeType::Created);
        assert_eq!(FileChangeType::from(FileChangeKind::Modified), FileChangeType::Changed);
        assert_eq!(FileChangeType::from(FileChangeKind::Deleted), FileChangeType::Deleted);
        assert_eq!(FileChangeType::from(FileChangeKind::Renamed), FileChangeType::Changed);
    }

    #[test]
    fn watcher_new_succeeds() {
        let watcher = FileWatcher::new();
        assert!(watcher.is_ok());
    }

    #[test]
    fn watch_and_unwatch() {
        let dir = tempfile::tempdir().unwrap();
        let mut watcher = FileWatcher::new().unwrap();
        watcher.watch(dir.path()).unwrap();
        assert!(watcher.watched_paths().contains(dir.path()));

        watcher.unwatch(dir.path()).unwrap();
        assert!(watcher.watched_paths().is_empty());
    }

    #[test]
    fn unwatch_not_watched_returns_error() {
        let mut watcher = FileWatcher::new().unwrap();
        let result = watcher.unwatch(Path::new("/not/watched"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WatchError::NotWatched(_)));
    }

    #[test]
    fn try_recv_returns_none_when_empty() {
        let mut watcher = FileWatcher::new().unwrap();
        assert!(watcher.try_recv().is_none());
    }

    #[test]
    fn to_file_change_event_conversion() {
        let event = WatcherEvent {
            path: PathBuf::from("/tmp/test.txt"),
            kind: FileChangeKind::Created,
        };
        let fce = FileWatcher::to_file_change_event(&event);
        assert_eq!(fce.change_type, FileChangeType::Created);
        assert!(fce.uri.path.contains("test.txt"));
    }

    #[tokio::test]
    async fn watch_detects_file_creation() {
        let dir = tempfile::tempdir().unwrap();
        let mut watcher = FileWatcher::new().unwrap();
        watcher.watch(dir.path()).unwrap();

        let file_path = dir.path().join("new_file.txt");
        fs::write(&file_path, "hello").unwrap();

        // Poll for event with timeout
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut found = false;
        while tokio::time::Instant::now() < deadline {
            if let Some(evt) = watcher.try_recv() {
                if evt.path == file_path {
                    found = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(found, "should have received a file change event");
    }

    #[tokio::test]
    async fn watch_detects_file_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("to_delete.txt");
        fs::write(&file_path, "bye").unwrap();

        let mut watcher = FileWatcher::new().unwrap();
        watcher.watch(dir.path()).unwrap();
        // Small delay so watcher is established
        tokio::time::sleep(Duration::from_millis(100)).await;

        fs::remove_file(&file_path).unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut found = false;
        while tokio::time::Instant::now() < deadline {
            if let Some(evt) = watcher.try_recv() {
                if evt.path == file_path && evt.kind == FileChangeKind::Deleted {
                    found = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(found, "should have received a delete event");
    }

    #[test]
    fn watcher_event_debug() {
        let event = WatcherEvent {
            path: PathBuf::from("/tmp/test.txt"),
            kind: FileChangeKind::Modified,
        };
        let dbg = format!("{event:?}");
        assert!(dbg.contains("Modified"));
    }

    #[test]
    fn watch_error_display() {
        let err = WatchError::NotWatched(PathBuf::from("/foo"));
        assert!(err.to_string().contains("/foo"));
    }

    #[test]
    fn file_change_kind_equality() {
        assert_eq!(FileChangeKind::Created, FileChangeKind::Created);
        assert_ne!(FileChangeKind::Created, FileChangeKind::Deleted);
    }
}
