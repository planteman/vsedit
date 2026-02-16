//! Hot exit and file backup.

/// A single backup record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub original_path: String,
    pub backup_path: String,
    pub timestamp: u64,
    pub size: u64,
}

/// In-memory backup service that tracks file snapshots.
pub struct BackupService {
    pub backup_dir: String,
    pub max_backups: usize,
    entries: Vec<BackupEntry>,
    next_timestamp: u64,
}

impl BackupService {
    pub fn new(backup_dir: impl Into<String>) -> Self {
        Self {
            backup_dir: backup_dir.into(),
            max_backups: 5,
            entries: Vec::new(),
            next_timestamp: 1,
        }
    }

    /// Create a backup entry for the given path and content.
    pub fn create_backup(&mut self, path: &str, content: &str) -> BackupEntry {
        let ts = self.next_timestamp;
        self.next_timestamp += 1;
        let backup_path = self.generate_backup_path(path, ts);
        let entry = BackupEntry {
            original_path: path.to_string(),
            backup_path,
            timestamp: ts,
            size: content.len() as u64,
        };
        self.entries.push(entry.clone());
        entry
    }

    /// List all backups for a given original path, ordered by timestamp.
    pub fn list_backups(&self, path: &str) -> Vec<&BackupEntry> {
        let mut results: Vec<&BackupEntry> = self
            .entries
            .iter()
            .filter(|e| e.original_path == path)
            .collect();
        results.sort_by_key(|e| e.timestamp);
        results
    }

    /// Return the backup path of the most recent backup for the given path.
    pub fn restore_latest(&self, path: &str) -> Option<String> {
        self.list_backups(path)
            .last()
            .map(|e| e.backup_path.clone())
    }

    /// Keep only the most recent `max_backups` entries for the given path.
    pub fn prune_old_backups(&mut self, path: &str) {
        let mut indices: Vec<(usize, u64)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.original_path == path)
            .map(|(i, e)| (i, e.timestamp))
            .collect();
        indices.sort_by_key(|&(_, ts)| ts);

        if indices.len() > self.max_backups {
            let to_remove = indices.len() - self.max_backups;
            let remove_indices: Vec<usize> =
                indices[..to_remove].iter().map(|&(i, _)| i).collect();
            // Remove in reverse order to preserve indices.
            for &i in remove_indices.iter().rev() {
                self.entries.remove(i);
            }
        }
    }

    fn generate_backup_path(&self, path: &str, timestamp: u64) -> String {
        let file_name = path.rsplit('/').next().unwrap_or(path);
        format!("{}/{}.{}.bak", self.backup_dir, file_name, timestamp)
    }
}

impl Default for BackupService {
    fn default() -> Self {
        Self::new("/tmp/backups")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_backup() {
        let mut svc = BackupService::new("/backups");
        let entry = svc.create_backup("/home/user/file.txt", "hello world");
        assert_eq!(entry.original_path, "/home/user/file.txt");
        assert_eq!(entry.size, 11);
        assert_eq!(entry.backup_path, "/backups/file.txt.1.bak");
        assert_eq!(svc.list_backups("/home/user/file.txt").len(), 1);
    }

    #[test]
    fn prune_old_backups() {
        let mut svc = BackupService::new("/backups");
        svc.max_backups = 2;
        for i in 0..5 {
            svc.create_backup("/a.txt", &format!("v{i}"));
        }
        assert_eq!(svc.list_backups("/a.txt").len(), 5);
        svc.prune_old_backups("/a.txt");
        let remaining = svc.list_backups("/a.txt");
        assert_eq!(remaining.len(), 2);
        // The two most recent should survive.
        assert_eq!(remaining[0].timestamp, 4);
        assert_eq!(remaining[1].timestamp, 5);
    }

    #[test]
    fn restore_latest() {
        let mut svc = BackupService::new("/backups");
        assert!(svc.restore_latest("/missing.txt").is_none());
        svc.create_backup("/f.txt", "a");
        svc.create_backup("/f.txt", "b");
        let latest = svc.restore_latest("/f.txt").unwrap();
        assert_eq!(latest, "/backups/f.txt.2.bak");
    }
}
