//! File download service.

use std::fmt;
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
    priorities: Vec<(u64, DownloadPriority)>,
}

impl DownloadService {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            priorities: Vec::new(),
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

// ---------------------------------------------------------------------------
// DownloadError
// ---------------------------------------------------------------------------

/// Errors that can occur within the download service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    /// The requested entry was not found.
    NotFound(u64),
    /// The entry has already completed.
    AlreadyCompleted(u64),
    /// The provided URL is invalid.
    InvalidUrl(String),
    /// The state transition is not allowed.
    InvalidState {
        from: DownloadState,
        to: DownloadState,
    },
    /// The download queue has reached its capacity.
    QueueFull { capacity: usize },
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::NotFound(id) => write!(f, "download entry {id} not found"),
            DownloadError::AlreadyCompleted(id) => {
                write!(f, "download entry {id} is already completed")
            }
            DownloadError::InvalidUrl(url) => write!(f, "invalid url: {url}"),
            DownloadError::InvalidState { from, to } => {
                write!(f, "invalid state transition from {from:?} to {to:?}")
            }
            DownloadError::QueueFull { capacity } => {
                write!(f, "download queue is full (capacity: {capacity})")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DownloadPriority
// ---------------------------------------------------------------------------

/// Priority level for a download entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl DownloadPriority {
    /// Returns a numeric rank used for ordering (higher is more urgent).
    fn rank(self) -> u8 {
        match self {
            DownloadPriority::Low => 0,
            DownloadPriority::Normal => 1,
            DownloadPriority::High => 2,
            DownloadPriority::Urgent => 3,
        }
    }
}

impl PartialOrd for DownloadPriority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DownloadPriority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl Default for DownloadPriority {
    fn default() -> Self {
        DownloadPriority::Normal
    }
}

// ---------------------------------------------------------------------------
// Extended DownloadEntry – priority field
// ---------------------------------------------------------------------------

impl DownloadEntry {
    /// Priority of the entry. Stored as a separate companion value because the
    /// struct is already public and adding a field would be a breaking change
    /// for downstream code that constructs it directly. Instead we provide
    /// priority through the service layer and store it in a parallel map.
    ///
    /// For convenience we expose a helper that the service uses internally.
    fn matches_state(&self, state: DownloadState) -> bool {
        self.state == state
    }
}

// ---------------------------------------------------------------------------
// DownloadService extensions
// ---------------------------------------------------------------------------

impl DownloadService {
    /// Enqueue a request with an explicit priority. Returns the assigned id.
    ///
    /// Priority metadata is stored in a position-aware manner: urgent items
    /// are inserted before lower-priority pending items so that iteration
    /// order reflects priority.
    pub fn enqueue_with_priority(
        &mut self,
        request: DownloadRequest,
        priority: DownloadPriority,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let entry = DownloadEntry {
            id,
            request,
            state: DownloadState::Pending,
            progress: DownloadProgress::new(),
        };

        // Insert before the first pending entry whose implicit priority is
        // lower. Non-pending entries are left in place so that ordering is
        // only among pending items.
        let insert_pos = self.find_priority_insert_pos(priority);
        self.entries.insert(insert_pos, entry);
        // Store priority tag alongside the id.
        self.priority_map_insert(id, priority);
        id
    }

    /// Return the id of the highest-priority pending entry, if any.
    pub fn get_next_pending(&self) -> Option<u64> {
        // Entries are kept sorted so that highest-priority pending items
        // appear first among the pending subset.
        self.entries
            .iter()
            .find(|e| e.matches_state(DownloadState::Pending))
            .map(|e| e.id)
    }

    /// Re-enqueue all failed entries by resetting them to Pending. Returns the
    /// number of entries that were re-queued.
    pub fn requeue_failed(&mut self) -> usize {
        let mut count = 0;
        for entry in &mut self.entries {
            if entry.state == DownloadState::Failed {
                entry.state = DownloadState::Pending;
                entry.progress = DownloadProgress::new();
                count += 1;
            }
        }
        count
    }

    /// Estimate overall throughput in bytes per second based on a supplied
    /// elapsed duration. This is a simple calculation: total bytes downloaded
    /// divided by the elapsed seconds.
    pub fn get_throughput(&self, elapsed_secs: f64) -> f64 {
        if elapsed_secs <= 0.0 {
            return 0.0;
        }
        self.total_bytes_downloaded() as f64 / elapsed_secs
    }

    /// Transition a download entry to a new state with validation. Only
    /// certain state transitions are allowed:
    ///
    /// - Pending → InProgress | Cancelled
    /// - InProgress → Completed | Failed | Cancelled
    /// - Failed → Pending (retry)
    ///
    /// All other transitions return an error.
    pub fn set_entry_state(
        &mut self,
        id: u64,
        new_state: DownloadState,
    ) -> Result<(), DownloadError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(DownloadError::NotFound(id))?;

        if !Self::is_valid_transition(entry.state, new_state) {
            return Err(DownloadError::InvalidState {
                from: entry.state,
                to: new_state,
            });
        }

        // Reset progress when transitioning back to Pending (retry).
        if new_state == DownloadState::Pending {
            entry.progress = DownloadProgress::new();
        }

        entry.state = new_state;
        Ok(())
    }

    /// Check whether a state transition is valid.
    fn is_valid_transition(from: DownloadState, to: DownloadState) -> bool {
        matches!(
            (from, to),
            (DownloadState::Pending, DownloadState::InProgress)
                | (DownloadState::Pending, DownloadState::Cancelled)
                | (DownloadState::InProgress, DownloadState::Completed)
                | (DownloadState::InProgress, DownloadState::Failed)
                | (DownloadState::InProgress, DownloadState::Cancelled)
                | (DownloadState::Failed, DownloadState::Pending)
        )
    }

    // -- internal helpers for priority ordering --------------------------------

    /// Find the position at which to insert a new pending entry so that the
    /// pending subset stays sorted by descending priority.
    fn find_priority_insert_pos(&self, priority: DownloadPriority) -> usize {
        // Walk from the end to find the first pending entry with priority >=
        // the new one. Insert after it.
        let mut pos = self.entries.len();
        for (i, entry) in self.entries.iter().enumerate().rev() {
            if entry.state == DownloadState::Pending {
                let existing_prio = self.priority_for(entry.id);
                if existing_prio >= priority {
                    pos = i + 1;
                    break;
                }
                pos = i;
            }
        }
        pos
    }

    /// Store a priority tag. We use a simple inline Vec of (id, priority)
    /// pairs appended to a field we add below.
    fn priority_map_insert(&mut self, id: u64, priority: DownloadPriority) {
        self.priorities.push((id, priority));
    }

    /// Look up the stored priority for an entry, defaulting to Normal.
    fn priority_for(&self, id: u64) -> DownloadPriority {
        self.priorities
            .iter()
            .find(|(eid, _)| *eid == id)
            .map(|(_, p)| *p)
            .unwrap_or(DownloadPriority::Normal)
    }
}

// ---------------------------------------------------------------------------
// BandwidthTracker
// ---------------------------------------------------------------------------

/// Tracks download speed by recording timestamped byte counts.
#[derive(Debug)]
pub struct BandwidthTracker {
    /// Each sample is (timestamp_secs, cumulative_bytes_at_that_time).
    samples: Vec<(f64, u64)>,
    peak: f64,
}

impl BandwidthTracker {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            peak: 0.0,
        }
    }

    /// Record that `bytes` additional bytes were transferred at `timestamp`
    /// (seconds since an arbitrary epoch).
    pub fn record_bytes(&mut self, timestamp: f64, bytes: u64) {
        let cumulative = self.samples.last().map_or(0, |s| s.1) + bytes;
        self.samples.push((timestamp, cumulative));

        // Update peak speed using the last two samples.
        if self.samples.len() >= 2 {
            let prev = &self.samples[self.samples.len() - 2];
            let curr = &self.samples[self.samples.len() - 1];
            let dt = curr.0 - prev.0;
            if dt > 0.0 {
                let speed = (curr.1 - prev.1) as f64 / dt;
                if speed > self.peak {
                    self.peak = speed;
                }
            }
        }
    }

    /// Instantaneous speed derived from the last two samples (bytes/sec).
    pub fn current_speed(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let prev = &self.samples[self.samples.len() - 2];
        let curr = &self.samples[self.samples.len() - 1];
        let dt = curr.0 - prev.0;
        if dt <= 0.0 {
            return 0.0;
        }
        (curr.1 - prev.1) as f64 / dt
    }

    /// Average speed from the first sample to the last (bytes/sec).
    pub fn average_speed(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let first = &self.samples[0];
        let last = &self.samples[self.samples.len() - 1];
        let dt = last.0 - first.0;
        if dt <= 0.0 {
            return 0.0;
        }
        (last.1 - first.1) as f64 / dt
    }

    /// Peak speed observed across all consecutive sample pairs (bytes/sec).
    pub fn peak_speed(&self) -> f64 {
        self.peak
    }
}

impl Default for BandwidthTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// UrlValidator
// ---------------------------------------------------------------------------

/// Simple URL validation and extraction utilities (no external crates).
pub struct UrlValidator;

impl UrlValidator {
    /// Basic validity check: must start with `http://` or `https://` and
    /// contain a host portion with at least one dot.
    pub fn is_valid_url(url: &str) -> bool {
        let rest = if let Some(r) = url.strip_prefix("https://") {
            r
        } else if let Some(r) = url.strip_prefix("http://") {
            r
        } else {
            return false;
        };

        // Must have a non-empty host with at least one dot.
        let host = rest.split('/').next().unwrap_or("");
        !host.is_empty() && host.contains('.')
    }

    /// Extract the filename component from a URL path, if present.
    pub fn extract_filename(url: &str) -> Option<String> {
        let path = Self::url_path(url)?;
        let segment = path.rsplit('/').next()?;
        if segment.is_empty() || !segment.contains('.') {
            return None;
        }
        Some(segment.to_string())
    }

    /// Extract the file extension from a URL path, if present.
    pub fn extract_extension(url: &str) -> Option<String> {
        let filename = Self::extract_filename(url)?;
        let ext = filename.rsplit('.').next()?;
        if ext.is_empty() || ext == filename {
            return None;
        }
        Some(ext.to_string())
    }

    /// Normalize a URL by lowercasing the scheme and host, removing a
    /// trailing slash on the path, and stripping default ports (80 for http,
    /// 443 for https).
    pub fn normalize_url(url: &str) -> String {
        let (scheme, rest) = if let Some(r) = url.strip_prefix("https://") {
            ("https", r)
        } else if let Some(r) = url.strip_prefix("http://") {
            ("http", r)
        } else if let Some(r) = url.strip_prefix("HTTPS://") {
            ("https", r)
        } else if let Some(r) = url.strip_prefix("HTTP://") {
            ("http", r)
        } else {
            return url.to_string();
        };

        let (host_port, path) = match rest.find('/') {
            Some(idx) => (&rest[..idx], &rest[idx..]),
            None => (rest, "/"),
        };

        let host_port_lower = host_port.to_ascii_lowercase();

        // Strip default ports.
        let host_clean = if scheme == "https" {
            host_port_lower
                .strip_suffix(":443")
                .unwrap_or(&host_port_lower)
        } else {
            host_port_lower
                .strip_suffix(":80")
                .unwrap_or(&host_port_lower)
        };

        let path_clean = if path.len() > 1 {
            path.trim_end_matches('/')
        } else {
            path
        };

        format!("{scheme}://{host_clean}{path_clean}")
    }

    // -- internal helpers ---

    /// Extract the path portion of a URL (after scheme + host).
    fn url_path(url: &str) -> Option<&str> {
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))?;
        rest.find('/').map(|i| &rest[i..])
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

    // -----------------------------------------------------------------------
    // New tests for extended functionality
    // -----------------------------------------------------------------------

    #[test]
    fn download_error_display() {
        assert_eq!(
            DownloadError::NotFound(42).to_string(),
            "download entry 42 not found"
        );
        assert_eq!(
            DownloadError::AlreadyCompleted(7).to_string(),
            "download entry 7 is already completed"
        );
        assert_eq!(
            DownloadError::InvalidUrl("ftp://bad".into()).to_string(),
            "invalid url: ftp://bad"
        );
        assert_eq!(
            DownloadError::QueueFull { capacity: 10 }.to_string(),
            "download queue is full (capacity: 10)"
        );
        let err = DownloadError::InvalidState {
            from: DownloadState::Completed,
            to: DownloadState::Pending,
        };
        assert!(err.to_string().contains("Completed"));
    }

    #[test]
    fn priority_ordering() {
        assert!(DownloadPriority::Urgent > DownloadPriority::High);
        assert!(DownloadPriority::High > DownloadPriority::Normal);
        assert!(DownloadPriority::Normal > DownloadPriority::Low);
        assert_eq!(DownloadPriority::default(), DownloadPriority::Normal);

        let mut prios = vec![
            DownloadPriority::Normal,
            DownloadPriority::Urgent,
            DownloadPriority::Low,
            DownloadPriority::High,
        ];
        prios.sort();
        assert_eq!(
            prios,
            vec![
                DownloadPriority::Low,
                DownloadPriority::Normal,
                DownloadPriority::High,
                DownloadPriority::Urgent,
            ]
        );
    }

    #[test]
    fn enqueue_with_priority_and_get_next_pending() {
        let mut svc = DownloadService::new();
        let low_id = svc.enqueue_with_priority(sample_request(), DownloadPriority::Low);
        let _normal_id = svc.enqueue_with_priority(sample_request(), DownloadPriority::Normal);
        let urgent_id = svc.enqueue_with_priority(sample_request(), DownloadPriority::Urgent);

        // The highest-priority pending entry should be Urgent.
        let next = svc.get_next_pending();
        assert_eq!(next, Some(urgent_id));

        // After starting the urgent one, Normal should be next.
        svc.update_progress(urgent_id, 10, None);
        let next = svc.get_next_pending();
        // Should not be the urgent one anymore (it's InProgress).
        assert_ne!(next, Some(urgent_id));

        // Low should come after normal.
        assert_ne!(next, Some(low_id));
    }

    #[test]
    fn requeue_failed_resets_all_failures() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());
        let id3 = svc.enqueue(sample_request());

        svc.fail(id1);
        svc.fail(id2);
        // id3 stays pending.

        let requeued = svc.requeue_failed();
        assert_eq!(requeued, 2);
        assert_eq!(svc.get_state(id1), Some(DownloadState::Pending));
        assert_eq!(svc.get_state(id2), Some(DownloadState::Pending));
        assert_eq!(svc.get_state(id3), Some(DownloadState::Pending));
        assert_eq!(svc.failed_count(), 0);
    }

    #[test]
    fn get_throughput_calculation() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());

        svc.update_progress(id1, 5000, Some(10000));
        svc.update_progress(id2, 3000, Some(6000));

        // 8000 bytes over 4 seconds = 2000 bytes/sec.
        let tp = svc.get_throughput(4.0);
        assert!((tp - 2000.0).abs() < f64::EPSILON);

        // Zero or negative elapsed should return 0.
        assert_eq!(svc.get_throughput(0.0), 0.0);
        assert_eq!(svc.get_throughput(-1.0), 0.0);
    }

    #[test]
    fn set_entry_state_valid_transitions() {
        let mut svc = DownloadService::new();
        let id = svc.enqueue(sample_request());

        // Pending → InProgress
        assert!(svc.set_entry_state(id, DownloadState::InProgress).is_ok());
        assert_eq!(svc.get_state(id), Some(DownloadState::InProgress));

        // InProgress → Failed
        assert!(svc.set_entry_state(id, DownloadState::Failed).is_ok());
        assert_eq!(svc.get_state(id), Some(DownloadState::Failed));

        // Failed → Pending (retry)
        assert!(svc.set_entry_state(id, DownloadState::Pending).is_ok());
        assert_eq!(svc.get_state(id), Some(DownloadState::Pending));
    }

    #[test]
    fn set_entry_state_invalid_transitions() {
        let mut svc = DownloadService::new();
        let id = svc.enqueue(sample_request());

        // Pending → Completed is not allowed.
        let err = svc.set_entry_state(id, DownloadState::Completed);
        assert!(err.is_err());
        assert_eq!(
            err.unwrap_err(),
            DownloadError::InvalidState {
                from: DownloadState::Pending,
                to: DownloadState::Completed,
            }
        );

        // Non-existent id.
        let err = svc.set_entry_state(999, DownloadState::InProgress);
        assert_eq!(err.unwrap_err(), DownloadError::NotFound(999));
    }

    #[test]
    fn bandwidth_tracker_speed_calculations() {
        let mut tracker = BandwidthTracker::new();
        assert_eq!(tracker.current_speed(), 0.0);
        assert_eq!(tracker.average_speed(), 0.0);
        assert_eq!(tracker.peak_speed(), 0.0);

        // Simulate: 1000 bytes at t=0, 2000 bytes at t=1, 500 bytes at t=2.
        tracker.record_bytes(0.0, 1000);
        tracker.record_bytes(1.0, 2000);
        tracker.record_bytes(2.0, 500);

        // Current speed = last interval: 500 bytes / 1 sec = 500 b/s.
        assert!((tracker.current_speed() - 500.0).abs() < f64::EPSILON);

        // Average speed = (3500 - 1000) / (2.0 - 0.0) = 1250 b/s
        // Note: cumulative at t=0 is 1000, at t=2 is 3500.
        let avg = tracker.average_speed();
        assert!((avg - 1250.0).abs() < f64::EPSILON);

        // Peak speed was during the second interval: 2000/1 = 2000 b/s.
        assert!((tracker.peak_speed() - 2000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn url_validator_is_valid_url() {
        assert!(UrlValidator::is_valid_url("https://example.com/file.bin"));
        assert!(UrlValidator::is_valid_url("http://cdn.example.org/path"));
        assert!(!UrlValidator::is_valid_url("ftp://example.com/file"));
        assert!(!UrlValidator::is_valid_url("https://localhost/file"));
        assert!(!UrlValidator::is_valid_url("not a url"));
    }

    #[test]
    fn url_validator_extract_filename_and_extension() {
        assert_eq!(
            UrlValidator::extract_filename("https://example.com/downloads/archive.tar.gz"),
            Some("archive.tar.gz".into())
        );
        assert_eq!(
            UrlValidator::extract_extension("https://example.com/downloads/archive.tar.gz"),
            Some("gz".into())
        );
        assert_eq!(
            UrlValidator::extract_filename("https://example.com/"),
            None
        );
        assert_eq!(
            UrlValidator::extract_extension("https://example.com/no-extension"),
            None
        );
    }

    #[test]
    fn url_validator_normalize_url() {
        assert_eq!(
            UrlValidator::normalize_url("HTTPS://Example.COM:443/path/"),
            "https://example.com/path"
        );
        assert_eq!(
            UrlValidator::normalize_url("HTTP://CDN.Example.Org:80/file.bin"),
            "http://cdn.example.org/file.bin"
        );
        // Non-default port is preserved.
        assert_eq!(
            UrlValidator::normalize_url("https://example.com:8080/api"),
            "https://example.com:8080/api"
        );
    }
}
