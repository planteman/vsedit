//! File download service.

/// State of a download entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Progress information for a download.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub percentage: Option<f64>,
}

impl DownloadProgress {
    fn new() -> Self {
        Self {
            bytes_downloaded: 0,
            total_bytes: None,
            percentage: None,
        }
    }
}

/// A request describing what to download and where to store it.
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub destination: String,
    pub headers: Vec<(String, String)>,
}

/// The result of a completed download.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub path: String,
    pub size: u64,
    pub state: DownloadState,
}

/// A tracked download entry managed by the service.
#[derive(Debug)]
pub struct DownloadEntry {
    pub id: u64,
    pub request: DownloadRequest,
    pub state: DownloadState,
    pub progress: DownloadProgress,
}

/// Service that manages a queue of downloads.
pub struct DownloadService {
    entries: Vec<DownloadEntry>,
    next_id: u64,
}

impl DownloadService {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a download request to the queue. Returns the assigned id.
    pub fn enqueue(&mut self, request: DownloadRequest) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(DownloadEntry {
            id,
            request,
            state: DownloadState::Pending,
            progress: DownloadProgress::new(),
        });
        id
    }

    /// Update progress for a download entry.
    pub fn update_progress(&mut self, id: u64, downloaded: u64, total: Option<u64>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.state = DownloadState::InProgress;
            entry.progress.bytes_downloaded = downloaded;
            entry.progress.total_bytes = total;
            entry.progress.percentage = total.map(|t| {
                if t == 0 {
                    100.0
                } else {
                    (downloaded as f64 / t as f64) * 100.0
                }
            });
        }
    }

    /// Mark a download as completed.
    pub fn complete(&mut self, id: u64, path: String, size: u64) -> Option<DownloadResult> {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.state = DownloadState::Completed;
            entry.progress.bytes_downloaded = size;
            entry.progress.total_bytes = Some(size);
            entry.progress.percentage = Some(100.0);
            Some(DownloadResult {
                path,
                size,
                state: DownloadState::Completed,
            })
        } else {
            None
        }
    }

    /// Mark a download as failed.
    pub fn fail(&mut self, id: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.state = DownloadState::Failed;
        }
    }

    /// Cancel a download.
    pub fn cancel(&mut self, id: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.state = DownloadState::Cancelled;
        }
    }

    /// Get the current state of a download.
    pub fn get_state(&self, id: u64) -> Option<DownloadState> {
        self.entries.iter().find(|e| e.id == id).map(|e| e.state)
    }

    /// Count downloads that are currently in progress.
    pub fn active_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.state == DownloadState::InProgress)
            .count()
    }

    /// Get a reference to a download entry by id.
    pub fn get_entry(&self, id: u64) -> Option<&DownloadEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Get all entries matching a given state.
    pub fn get_entries_by_state(&self, state: DownloadState) -> Vec<&DownloadEntry> {
        self.entries.iter().filter(|e| e.state == state).collect()
    }

    /// Count downloads that are pending.
    pub fn pending_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.state == DownloadState::Pending)
            .count()
    }

    /// Count downloads that completed successfully.
    pub fn completed_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.state == DownloadState::Completed)
            .count()
    }

    /// Count downloads that failed.
    pub fn failed_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.state == DownloadState::Failed)
            .count()
    }

    /// Total bytes downloaded across all entries.
    pub fn total_bytes_downloaded(&self) -> u64 {
        self.entries
            .iter()
            .map(|e| e.progress.bytes_downloaded)
            .sum()
    }

    /// Retry a failed download by resetting it to Pending. Returns true if the
    /// entry was found in Failed state and reset.
    pub fn retry(&mut self, id: u64) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            if entry.state == DownloadState::Failed {
                entry.state = DownloadState::Pending;
                entry.progress = DownloadProgress::new();
                return true;
            }
        }
        false
    }

    /// Cancel all non-completed entries. Returns the number of entries cancelled.
    pub fn cancel_all(&mut self) -> usize {
        let mut count = 0;
        for entry in &mut self.entries {
            if entry.state != DownloadState::Completed
                && entry.state != DownloadState::Cancelled
            {
                entry.state = DownloadState::Cancelled;
                count += 1;
            }
        }
        count
    }

    /// Remove all completed entries. Returns the number of entries removed.
    pub fn remove_completed(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| e.state != DownloadState::Completed);
        before - self.entries.len()
    }

    /// Build aggregate statistics for the current queue.
    pub fn get_stats(&self) -> DownloadStats {
        let mut stats = DownloadStats {
            total: self.entries.len(),
            pending: 0,
            in_progress: 0,
            completed: 0,
            failed: 0,
            cancelled: 0,
            total_bytes: 0,
        };
        for entry in &self.entries {
            match entry.state {
                DownloadState::Pending => stats.pending += 1,
                DownloadState::InProgress => stats.in_progress += 1,
                DownloadState::Completed => stats.completed += 1,
                DownloadState::Failed => stats.failed += 1,
                DownloadState::Cancelled => stats.cancelled += 1,
            }
            stats.total_bytes += entry.progress.bytes_downloaded;
        }
        stats
    }

    /// Check whether a new download can be started given the concurrency limit.
    pub fn can_start_new(&self, config: &DownloadConfig) -> bool {
        self.active_count() < config.max_concurrent
    }
}

/// Aggregate statistics for the download queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadStats {
    pub total: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub total_bytes: u64,
}

/// Configuration for the download service.
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub max_concurrent: usize,
    pub timeout_seconds: u64,
    pub retry_count: u32,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 4,
            timeout_seconds: 60,
            retry_count: 3,
        }
    }
}

impl Default for DownloadService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> DownloadRequest {
        DownloadRequest {
            url: "https://example.com/file.bin".into(),
            destination: "/tmp/file.bin".into(),
            headers: vec![],
        }
    }

    #[test]
    fn enqueue_and_get_state() {
        let mut svc = DownloadService::new();
        let id = svc.enqueue(sample_request());
        assert_eq!(svc.get_state(id), Some(DownloadState::Pending));
        assert_eq!(svc.get_state(999), None);
    }

    #[test]
    fn progress_and_complete() {
        let mut svc = DownloadService::new();
        let id = svc.enqueue(sample_request());

        svc.update_progress(id, 500, Some(1000));
        assert_eq!(svc.get_state(id), Some(DownloadState::InProgress));
        assert_eq!(svc.active_count(), 1);

        let result = svc.complete(id, "/tmp/file.bin".into(), 1000).unwrap();
        assert_eq!(result.size, 1000);
        assert_eq!(result.state, DownloadState::Completed);
        assert_eq!(svc.active_count(), 0);
    }

    #[test]
    fn fail_and_cancel() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());

        svc.fail(id1);
        svc.cancel(id2);

        assert_eq!(svc.get_state(id1), Some(DownloadState::Failed));
        assert_eq!(svc.get_state(id2), Some(DownloadState::Cancelled));
    }

    #[test]
    fn get_entry_returns_entry() {
        let mut svc = DownloadService::new();
        let id = svc.enqueue(sample_request());
        let entry = svc.get_entry(id).unwrap();
        assert_eq!(entry.id, id);
        assert_eq!(entry.state, DownloadState::Pending);
        assert!(svc.get_entry(999).is_none());
    }

    #[test]
    fn get_entries_by_state_filters_correctly() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let _id2 = svc.enqueue(sample_request());
        let id3 = svc.enqueue(sample_request());

        svc.fail(id1);
        svc.fail(id3);

        let failed = svc.get_entries_by_state(DownloadState::Failed);
        assert_eq!(failed.len(), 2);
        let pending = svc.get_entries_by_state(DownloadState::Pending);
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn count_helpers() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());
        let id3 = svc.enqueue(sample_request());
        let _id4 = svc.enqueue(sample_request());

        svc.complete(id1, "/tmp/a".into(), 100);
        svc.fail(id2);
        svc.cancel(id3);

        assert_eq!(svc.pending_count(), 1);
        assert_eq!(svc.completed_count(), 1);
        assert_eq!(svc.failed_count(), 1);
    }

    #[test]
    fn total_bytes_downloaded_sums_all() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());

        svc.update_progress(id1, 300, Some(1000));
        svc.update_progress(id2, 200, Some(500));

        assert_eq!(svc.total_bytes_downloaded(), 500);
    }

    #[test]
    fn retry_resets_failed_entry() {
        let mut svc = DownloadService::new();
        let id = svc.enqueue(sample_request());

        svc.update_progress(id, 100, Some(500));
        svc.fail(id);
        assert_eq!(svc.get_state(id), Some(DownloadState::Failed));

        assert!(svc.retry(id));
        assert_eq!(svc.get_state(id), Some(DownloadState::Pending));
        assert_eq!(svc.get_entry(id).unwrap().progress.bytes_downloaded, 0);
    }

    #[test]
    fn retry_returns_false_for_non_failed() {
        let mut svc = DownloadService::new();
        let id = svc.enqueue(sample_request());
        assert!(!svc.retry(id));
        assert!(!svc.retry(999));
    }

    #[test]
    fn cancel_all_cancels_non_completed() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());
        let id3 = svc.enqueue(sample_request());
        let id4 = svc.enqueue(sample_request());

        svc.complete(id1, "/tmp/a".into(), 100);
        svc.update_progress(id2, 50, Some(200));

        let cancelled = svc.cancel_all();
        assert_eq!(cancelled, 3);
        assert_eq!(svc.get_state(id1), Some(DownloadState::Completed));
        assert_eq!(svc.get_state(id3), Some(DownloadState::Cancelled));
        assert_eq!(svc.get_state(id4), Some(DownloadState::Cancelled));
    }

    #[test]
    fn remove_completed_entries() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());
        let _id3 = svc.enqueue(sample_request());

        svc.complete(id1, "/tmp/a".into(), 100);
        svc.complete(id2, "/tmp/b".into(), 200);

        let removed = svc.remove_completed();
        assert_eq!(removed, 2);
        assert_eq!(svc.get_stats().total, 1);
    }

    #[test]
    fn get_stats_reflects_queue() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());
        let id3 = svc.enqueue(sample_request());
        let id4 = svc.enqueue(sample_request());
        let _id5 = svc.enqueue(sample_request());

        svc.update_progress(id1, 50, Some(100));
        svc.complete(id2, "/tmp/a".into(), 100);
        svc.fail(id3);
        svc.cancel(id4);

        let stats = svc.get_stats();
        assert_eq!(stats.total, 5);
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.in_progress, 1);
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.cancelled, 1);
        assert_eq!(stats.total_bytes, 150);
    }

    #[test]
    fn can_start_new_respects_config() {
        let mut svc = DownloadService::new();
        let config = DownloadConfig {
            max_concurrent: 2,
            timeout_seconds: 30,
            retry_count: 1,
        };

        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());
        let id3 = svc.enqueue(sample_request());

        svc.update_progress(id1, 10, None);
        assert!(svc.can_start_new(&config));

        svc.update_progress(id2, 20, None);
        assert!(!svc.can_start_new(&config));

        svc.complete(id1, "/tmp/a".into(), 10);
        assert!(svc.can_start_new(&config));

        svc.update_progress(id3, 5, None);
        assert!(!svc.can_start_new(&config));
    }

    #[test]
    fn download_config_default() {
        let config = DownloadConfig::default();
        assert_eq!(config.max_concurrent, 4);
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.retry_count, 3);
    }
}
