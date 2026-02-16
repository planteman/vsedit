//! Ext API: FileSystem.
//!
//! RPC bridge between the extension host and the main thread for fs.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_fs";

// ── RPC message types ──

/// Messages exchanged for the `FileSystem` API surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FsMessage {
    ReadFile { uri: String },
    WriteFile { uri: String, content: Vec<u8> },
    Delete { uri: String, recursive: bool },
    Rename { old_uri: String, new_uri: String, overwrite: bool },
    Stat { uri: String },
    ReadDirectory { uri: String },
    CreateDirectory { uri: String },
    Watch { uri: String, recursive: bool },
}

/// Metadata about a file system entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub file_type: FileType,
    pub ctime: u64,
    pub mtime: u64,
    pub size: u64,
}

/// The type of a file system entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileType {
    File,
    Directory,
    SymbolicLink,
    Unknown,
}

/// A directory entry returned by `ReadDirectory`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub file_type: FileType,
}

/// Response payload for file system operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FsResponse {
    FileContent { data: Vec<u8> },
    Stat { stat: FileStat },
    Directory { entries: Vec<DirEntry> },
    WatchId { id: String },
    Ok,
    Error { message: String },
}

// ── Bridge ──

/// In-memory file system bridge for extensions.
#[derive(Debug, Default)]
pub struct FsBridge {
    files: HashMap<String, Vec<u8>>,
    next_watch_id: u64,
}

impl FsBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed a file into the in-memory store (for testing / virtual FS).
    pub fn seed_file(&mut self, uri: String, content: Vec<u8>) {
        self.files.insert(uri, content);
    }

    /// Process an incoming file system message and return a response.
    pub fn handle(&mut self, msg: FsMessage) -> FsResponse {
        match msg {
            FsMessage::ReadFile { uri } => {
                self.files.get(&uri).map_or(
                    FsResponse::Error { message: format!("not found: {uri}") },
                    |data| FsResponse::FileContent { data: data.clone() },
                )
            }
            FsMessage::WriteFile { uri, content } => {
                self.files.insert(uri, content);
                FsResponse::Ok
            }
            FsMessage::Delete { uri, .. } => {
                self.files.remove(&uri);
                FsResponse::Ok
            }
            FsMessage::Rename { old_uri, new_uri, .. } => {
                if let Some(data) = self.files.remove(&old_uri) {
                    self.files.insert(new_uri, data);
                }
                FsResponse::Ok
            }
            FsMessage::Stat { uri } => {
                if let Some(data) = self.files.get(&uri) {
                    FsResponse::Stat {
                        stat: FileStat {
                            file_type: FileType::File,
                            ctime: 0,
                            mtime: 0,
                            size: data.len() as u64,
                        },
                    }
                } else {
                    FsResponse::Error { message: format!("not found: {uri}") }
                }
            }
            FsMessage::ReadDirectory { .. } => {
                FsResponse::Directory { entries: Vec::new() }
            }
            FsMessage::CreateDirectory { .. } => FsResponse::Ok,
            FsMessage::Watch { .. } => {
                let id = format!("watch-{}", self.next_watch_id);
                self.next_watch_id += 1;
                FsResponse::WatchId { id }
            }
        }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

// ── Error types ──

/// Errors produced by file system operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    /// The requested URI was not found.
    NotFound(String),
    /// The URI failed validation.
    InvalidUri(String),
    /// A file already exists at the target URI.
    AlreadyExists(String),
    /// The operation is not supported.
    NotSupported(String),
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsError::NotFound(uri) => write!(f, "not found: {uri}"),
            FsError::InvalidUri(uri) => write!(f, "invalid uri: {uri}"),
            FsError::AlreadyExists(uri) => write!(f, "already exists: {uri}"),
            FsError::NotSupported(op) => write!(f, "not supported: {op}"),
        }
    }
}

impl std::error::Error for FsError {}

impl FsError {
    /// Convert this error into an `FsResponse::Error`.
    pub fn into_response(self) -> FsResponse {
        FsResponse::Error { message: self.to_string() }
    }
}

// ── Display for core types ──

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileType::File => write!(f, "file"),
            FileType::Directory => write!(f, "directory"),
            FileType::SymbolicLink => write!(f, "symlink"),
            FileType::Unknown => write!(f, "unknown"),
        }
    }
}

impl fmt::Display for FileStat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (size={}, ctime={}, mtime={})",
            self.file_type, self.size, self.ctime, self.mtime
        )
    }
}

impl fmt::Display for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.name, self.file_type)
    }
}

// ── FileStat builder ──

/// Builder for constructing `FileStat` instances with validation.
#[derive(Debug, Clone)]
pub struct FileStatBuilder {
    file_type: FileType,
    ctime: u64,
    mtime: u64,
    size: u64,
}

impl FileStatBuilder {
    pub fn new(file_type: FileType) -> Self {
        Self { file_type, ctime: 0, mtime: 0, size: 0 }
    }

    pub fn ctime(mut self, ctime: u64) -> Self {
        self.ctime = ctime;
        self
    }

    pub fn mtime(mut self, mtime: u64) -> Self {
        self.mtime = mtime;
        self
    }

    pub fn size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    /// Build the `FileStat`, ensuring mtime >= ctime.
    pub fn build(self) -> Result<FileStat, FsError> {
        if self.mtime != 0 && self.ctime != 0 && self.mtime < self.ctime {
            return Err(FsError::InvalidUri(
                "mtime must be >= ctime".to_string(),
            ));
        }
        Ok(FileStat {
            file_type: self.file_type,
            ctime: self.ctime,
            mtime: self.mtime,
            size: self.size,
        })
    }
}

// ── URI helpers ──

/// Validate that a URI has the expected `file://` scheme.
pub fn validate_file_uri(uri: &str) -> Result<(), FsError> {
    if !uri.starts_with("file://") {
        return Err(FsError::InvalidUri(uri.to_string()));
    }
    if uri.len() <= "file://".len() {
        return Err(FsError::InvalidUri(uri.to_string()));
    }
    Ok(())
}

/// Extract the path component from a `file://` URI.
pub fn uri_to_path(uri: &str) -> Result<&str, FsError> {
    validate_file_uri(uri)?;
    Ok(&uri["file://".len()..])
}

/// Extract the file name (last path segment) from a URI.
pub fn uri_file_name(uri: &str) -> Result<&str, FsError> {
    let path = uri_to_path(uri)?;
    Ok(path.rsplit('/').next().unwrap_or(path))
}

/// Extract the parent directory URI from a file URI.
pub fn uri_parent(uri: &str) -> Result<String, FsError> {
    let path = uri_to_path(uri)?;
    match path.rfind('/') {
        Some(pos) if pos > 0 => Ok(format!("file://{}", &path[..pos])),
        _ => Err(FsError::InvalidUri(format!("no parent for: {uri}"))),
    }
}

// ── Extended FsBridge methods ──

impl Clone for FsBridge {
    fn clone(&self) -> Self {
        Self {
            files: self.files.clone(),
            next_watch_id: self.next_watch_id,
        }
    }
}

impl PartialEq for FsBridge {
    fn eq(&self, other: &Self) -> bool {
        self.files == other.files && self.next_watch_id == other.next_watch_id
    }
}

impl FsBridge {
    /// Handle a message but validate URIs first, returning structured errors.
    pub fn handle_checked(&mut self, msg: FsMessage) -> Result<FsResponse, FsError> {
        match &msg {
            FsMessage::ReadFile { uri }
            | FsMessage::Stat { uri }
            | FsMessage::Delete { uri, .. }
            | FsMessage::ReadDirectory { uri }
            | FsMessage::CreateDirectory { uri }
            | FsMessage::Watch { uri, .. } => {
                validate_file_uri(uri)?;
            }
            FsMessage::WriteFile { uri, .. } => {
                validate_file_uri(uri)?;
            }
            FsMessage::Rename { old_uri, new_uri, .. } => {
                validate_file_uri(old_uri)?;
                validate_file_uri(new_uri)?;
            }
        }
        Ok(self.handle(msg))
    }

    /// Check whether a URI exists in the in-memory store.
    pub fn exists(&self, uri: &str) -> bool {
        self.files.contains_key(uri)
    }

    /// Return all URIs currently stored.
    pub fn uris(&self) -> Vec<&str> {
        self.files.keys().map(|s| s.as_str()).collect()
    }

    /// Return total bytes stored across all files.
    pub fn total_bytes(&self) -> u64 {
        self.files.values().map(|v| v.len() as u64).sum()
    }

    /// List directory entries for files whose URI starts with the given prefix.
    pub fn list_directory(&self, dir_uri: &str) -> Vec<DirEntry> {
        let prefix = if dir_uri.ends_with('/') {
            dir_uri.to_string()
        } else {
            format!("{dir_uri}/")
        };

        let mut entries = Vec::new();
        for key in self.files.keys() {
            if let Some(rest) = key.strip_prefix(prefix.as_str()) {
                // Only direct children (no further '/' in remainder).
                if !rest.contains('/') && !rest.is_empty() {
                    entries.push(DirEntry {
                        name: rest.to_string(),
                        file_type: FileType::File,
                    });
                }
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }

    /// Rename with an overwrite check – returns error if target exists and overwrite is false.
    pub fn rename_checked(
        &mut self,
        old_uri: &str,
        new_uri: &str,
        overwrite: bool,
    ) -> Result<(), FsError> {
        if !self.files.contains_key(old_uri) {
            return Err(FsError::NotFound(old_uri.to_string()));
        }
        if !overwrite && self.files.contains_key(new_uri) {
            return Err(FsError::AlreadyExists(new_uri.to_string()));
        }
        if let Some(data) = self.files.remove(old_uri) {
            self.files.insert(new_uri.to_string(), data);
        }
        Ok(())
    }
}

impl FileStat {
    /// Returns true if this entry represents a regular file.
    pub fn is_file(&self) -> bool {
        self.file_type == FileType::File
    }

    /// Returns true if this entry represents a directory.
    pub fn is_directory(&self) -> bool {
        self.file_type == FileType::Directory
    }

    /// Returns true if the file is empty (size == 0).
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// Initialize the fs extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn write_and_read() {
        let mut bridge = FsBridge::new();
        bridge.handle(FsMessage::WriteFile {
            uri: "file:///a.txt".into(),
            content: b"hello".to_vec(),
        });
        let resp = bridge.handle(FsMessage::ReadFile { uri: "file:///a.txt".into() });
        assert_eq!(resp, FsResponse::FileContent { data: b"hello".to_vec() });
    }

    #[test]
    fn read_missing_file() {
        let mut bridge = FsBridge::new();
        let resp = bridge.handle(FsMessage::ReadFile { uri: "file:///nope".into() });
        matches!(resp, FsResponse::Error { .. });
    }

    #[test]
    fn stat_returns_size() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///b.txt".into(), vec![1, 2, 3]);
        let resp = bridge.handle(FsMessage::Stat { uri: "file:///b.txt".into() });
        if let FsResponse::Stat { stat } = resp {
            assert_eq!(stat.size, 3);
            assert_eq!(stat.file_type, FileType::File);
        } else {
            panic!("expected Stat");
        }
    }

    #[test]
    fn delete_removes_file() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///c.txt".into(), vec![]);
        assert_eq!(bridge.file_count(), 1);
        bridge.handle(FsMessage::Delete { uri: "file:///c.txt".into(), recursive: false });
        assert_eq!(bridge.file_count(), 0);
    }

    #[test]
    fn serde_round_trip() {
        let msg = FsMessage::Rename {
            old_uri: "file:///old".into(),
            new_uri: "file:///new".into(),
            overwrite: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: FsMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    // ── Additional tests ──

    #[test]
    fn fs_error_display() {
        let err = FsError::NotFound("file:///gone".into());
        assert_eq!(err.to_string(), "not found: file:///gone");

        let err2 = FsError::InvalidUri("bad".into());
        assert_eq!(err2.to_string(), "invalid uri: bad");

        let err3 = FsError::AlreadyExists("file:///dup".into());
        assert_eq!(err3.to_string(), "already exists: file:///dup");
    }

    #[test]
    fn fs_error_into_response() {
        let resp = FsError::NotFound("file:///x".into()).into_response();
        assert_eq!(resp, FsResponse::Error { message: "not found: file:///x".into() });
    }

    #[test]
    fn validate_file_uri_ok() {
        assert!(validate_file_uri("file:///foo.txt").is_ok());
    }

    #[test]
    fn validate_file_uri_bad_scheme() {
        assert!(validate_file_uri("http://example.com").is_err());
        assert!(validate_file_uri("file://").is_err());
    }

    #[test]
    fn uri_to_path_extracts() {
        assert_eq!(uri_to_path("file:///home/a.txt").unwrap(), "/home/a.txt");
    }

    #[test]
    fn uri_file_name_extracts() {
        assert_eq!(uri_file_name("file:///home/user/doc.txt").unwrap(), "doc.txt");
    }

    #[test]
    fn uri_parent_extracts() {
        assert_eq!(
            uri_parent("file:///home/user/doc.txt").unwrap(),
            "file:///home/user"
        );
    }

    #[test]
    fn file_stat_builder_ok() {
        let stat = FileStatBuilder::new(FileType::File)
            .ctime(100)
            .mtime(200)
            .size(512)
            .build()
            .unwrap();
        assert_eq!(stat.size, 512);
        assert!(stat.is_file());
        assert!(!stat.is_directory());
    }

    #[test]
    fn file_stat_builder_bad_times() {
        let result = FileStatBuilder::new(FileType::File)
            .ctime(200)
            .mtime(100)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn bridge_exists_and_uris() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///a".into(), vec![1]);
        bridge.seed_file("file:///b".into(), vec![2, 3]);
        assert!(bridge.exists("file:///a"));
        assert!(!bridge.exists("file:///c"));
        assert_eq!(bridge.uris().len(), 2);
    }

    #[test]
    fn bridge_total_bytes() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///a".into(), vec![0; 10]);
        bridge.seed_file("file:///b".into(), vec![0; 5]);
        assert_eq!(bridge.total_bytes(), 15);
    }

    #[test]
    fn bridge_list_directory() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///root/a.txt".into(), vec![]);
        bridge.seed_file("file:///root/b.txt".into(), vec![]);
        bridge.seed_file("file:///root/sub/c.txt".into(), vec![]);
        let entries = bridge.list_directory("file:///root");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "a.txt");
        assert_eq!(entries[1].name, "b.txt");
    }

    #[test]
    fn rename_checked_no_overwrite() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///src".into(), vec![1]);
        bridge.seed_file("file:///dst".into(), vec![2]);
        let err = bridge.rename_checked("file:///src", "file:///dst", false);
        assert!(err.is_err());
        // With overwrite allowed, it succeeds.
        bridge.rename_checked("file:///src", "file:///dst", true).unwrap();
        assert!(!bridge.exists("file:///src"));
        assert!(bridge.exists("file:///dst"));
    }

    #[test]
    fn rename_checked_missing_source() {
        let mut bridge = FsBridge::new();
        let err = bridge.rename_checked("file:///nope", "file:///dst", true);
        assert!(err.is_err());
    }

    #[test]
    fn handle_checked_rejects_bad_uri() {
        let mut bridge = FsBridge::new();
        let result = bridge.handle_checked(FsMessage::ReadFile { uri: "bad".into() });
        assert!(result.is_err());
    }

    #[test]
    fn handle_checked_ok_for_valid_uri() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///x.txt".into(), b"data".to_vec());
        let resp = bridge.handle_checked(FsMessage::ReadFile { uri: "file:///x.txt".into() });
        assert!(resp.is_ok());
        assert_eq!(resp.unwrap(), FsResponse::FileContent { data: b"data".to_vec() });
    }

    #[test]
    fn bridge_clone_and_eq() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///f".into(), vec![42]);
        let clone = bridge.clone();
        assert_eq!(bridge, clone);
    }

    #[test]
    fn display_impls() {
        assert_eq!(FileType::File.to_string(), "file");
        assert_eq!(FileType::Directory.to_string(), "directory");
        assert_eq!(FileType::SymbolicLink.to_string(), "symlink");

        let entry = DirEntry { name: "test.rs".into(), file_type: FileType::File };
        assert_eq!(entry.to_string(), "test.rs [file]");

        let stat = FileStat { file_type: FileType::File, ctime: 1, mtime: 2, size: 100 };
        assert!(stat.to_string().contains("size=100"));
    }

    #[test]
    fn file_stat_helpers() {
        let stat = FileStat { file_type: FileType::Directory, ctime: 0, mtime: 0, size: 0 };
        assert!(stat.is_directory());
        assert!(!stat.is_file());
        assert!(stat.is_empty());
    }

    #[test]
    fn watch_increments_id() {
        let mut bridge = FsBridge::new();
        let r1 = bridge.handle(FsMessage::Watch { uri: "file:///a".into(), recursive: false });
        let r2 = bridge.handle(FsMessage::Watch { uri: "file:///b".into(), recursive: true });
        if let (FsResponse::WatchId { id: id1 }, FsResponse::WatchId { id: id2 }) = (&r1, &r2) {
            assert_ne!(id1, id2);
            assert_eq!(id1, "watch-0");
            assert_eq!(id2, "watch-1");
        } else {
            panic!("expected WatchId responses");
        }
    }
}
