//! File download service.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// DownloadProgress helpers
// ---------------------------------------------------------------------------

impl DownloadProgress {
    /// Returns `true` when the download has finished transferring all bytes.
    pub fn is_complete(&self) -> bool {
        match self.total_bytes {
            Some(total) => self.bytes_downloaded >= total,
            None => false,
        }
    }

    /// Returns the number of bytes still to be downloaded, if the total is
    /// known.
    pub fn remaining(&self) -> Option<u64> {
        self.total_bytes
            .map(|total| total.saturating_sub(self.bytes_downloaded))
    }
}

// ---------------------------------------------------------------------------
// DownloadService helpers
// ---------------------------------------------------------------------------

impl DownloadService {
    /// Get a mutable reference to a download entry by id.
    pub fn get_entry_mut(&mut self, id: u64) -> Option<&mut DownloadEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Returns `true` when the service has no entries at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total number of entries regardless of state.
    pub fn total_count(&self) -> usize {
        self.entries.len()
    }

    /// Return all entries whose request URL matches the given string.
    pub fn find_by_url(&self, url: &str) -> Vec<&DownloadEntry> {
        self.entries.iter().filter(|e| e.request.url == url).collect()
    }
}

// ---------------------------------------------------------------------------
// DownloadStats helpers
// ---------------------------------------------------------------------------

impl DownloadStats {
    /// Fraction of finished downloads that succeeded (completed / (completed +
    /// failed)). Returns `0.0` when no downloads have finished.
    pub fn success_rate(&self) -> f64 {
        let finished = (self.completed + self.failed) as f64;
        if finished == 0.0 {
            return 0.0;
        }
        self.completed as f64 / finished
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl fmt::Display for DownloadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            DownloadState::Pending => "Pending",
            DownloadState::InProgress => "In Progress",
            DownloadState::Completed => "Completed",
            DownloadState::Failed => "Failed",
            DownloadState::Cancelled => "Cancelled",
        };
        f.write_str(label)
    }
}

impl fmt::Display for DownloadStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Downloads: {} total, {} completed, {} failed, {} pending, {} in progress, {} cancelled, {} bytes",
            self.total,
            self.completed,
            self.failed,
            self.pending,
            self.in_progress,
            self.cancelled,
            self.total_bytes,
        )
    }
}

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

// ---------------------------------------------------------------------------
// DownloadBandwidth – rolling window average
// ---------------------------------------------------------------------------

/// Tracks bytes-per-second throughput using a rolling window of samples.
#[derive(Debug, Clone)]
pub struct DownloadBandwidth {
    /// Each sample is (timestamp_secs, bytes_transferred_in_interval).
    samples: Vec<(f64, u64)>,
    /// Maximum number of samples retained in the window.
    window_size: usize,
}

impl DownloadBandwidth {
    /// Create a new tracker with the given rolling window size.
    pub fn new(window_size: usize) -> Self {
        Self {
            samples: Vec::new(),
            window_size: window_size.max(1),
        }
    }

    /// Record a sample: `bytes` transferred at `timestamp` seconds.
    pub fn record(&mut self, timestamp: f64, bytes: u64) {
        self.samples.push((timestamp, bytes));
        if self.samples.len() > self.window_size {
            self.samples.remove(0);
        }
    }

    /// Rolling-window average speed in bytes per second.
    pub fn average_bps(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let first_ts = self.samples[0].0;
        let last_ts = self.samples[self.samples.len() - 1].0;
        let dt = last_ts - first_ts;
        if dt <= 0.0 {
            return 0.0;
        }
        let total_bytes: u64 = self.samples[1..].iter().map(|s| s.1).sum();
        total_bytes as f64 / dt
    }

    /// Number of samples currently in the window.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Estimate time remaining in seconds given `remaining_bytes`.
    pub fn estimate_remaining(&self, remaining_bytes: u64) -> Option<f64> {
        let bps = self.average_bps();
        if bps <= 0.0 {
            return None;
        }
        Some(remaining_bytes as f64 / bps)
    }
}

impl fmt::Display for DownloadBandwidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bps = self.average_bps();
        if bps >= 1_048_576.0 {
            write!(f, "{:.2} MB/s", bps / 1_048_576.0)
        } else if bps >= 1024.0 {
            write!(f, "{:.2} KB/s", bps / 1024.0)
        } else {
            write!(f, "{:.0} B/s", bps)
        }
    }
}

// ---------------------------------------------------------------------------
// DownloadRetryPolicy – backoff strategy
// ---------------------------------------------------------------------------

/// Backoff strategy for retry delays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffStrategy {
    /// Delay increases linearly: `base_delay * attempt`.
    Linear,
    /// Delay doubles each attempt: `base_delay * 2^(attempt-1)`.
    Exponential,
}

/// Policy controlling how failed downloads are retried.
#[derive(Debug, Clone)]
pub struct DownloadRetryPolicy {
    pub max_retries: u32,
    pub strategy: BackoffStrategy,
    /// Base delay in seconds before the first retry.
    pub base_delay_secs: f64,
    /// Maximum delay cap in seconds.
    pub max_delay_secs: f64,
}

impl DownloadRetryPolicy {
    /// Compute the delay in seconds for the given attempt (1-based).
    /// Returns `None` if the attempt exceeds `max_retries`.
    pub fn delay_for_attempt(&self, attempt: u32) -> Option<f64> {
        if attempt == 0 || attempt > self.max_retries {
            return None;
        }
        let raw = match self.strategy {
            BackoffStrategy::Linear => self.base_delay_secs * attempt as f64,
            BackoffStrategy::Exponential => {
                self.base_delay_secs * 2.0_f64.powi(attempt as i32 - 1)
            }
        };
        Some(raw.min(self.max_delay_secs))
    }

    /// Whether retries are exhausted after the given number of attempts.
    pub fn is_exhausted(&self, attempts: u32) -> bool {
        attempts >= self.max_retries
    }
}

impl Default for DownloadRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            strategy: BackoffStrategy::Exponential,
            base_delay_secs: 1.0,
            max_delay_secs: 30.0,
        }
    }
}

impl fmt::Display for DownloadRetryPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RetryPolicy(max={}, {:?}, base={:.1}s, cap={:.1}s)",
            self.max_retries, self.strategy, self.base_delay_secs, self.max_delay_secs
        )
    }
}

// ---------------------------------------------------------------------------
// DownloadFilter – query downloads
// ---------------------------------------------------------------------------

/// Filter criteria for querying download entries.
#[derive(Debug, Clone, Default)]
pub struct DownloadFilter {
    /// If set, only entries in this state match.
    pub state: Option<DownloadState>,
    /// If set, only entries whose URL contains this substring match.
    pub url_contains: Option<String>,
    /// If set, only entries with this priority match.
    pub priority: Option<DownloadPriority>,
}

impl DownloadFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_state(mut self, state: DownloadState) -> Self {
        self.state = Some(state);
        self
    }

    pub fn with_url_contains(mut self, pattern: &str) -> Self {
        self.url_contains = Some(pattern.to_string());
        self
    }

    pub fn with_priority(mut self, priority: DownloadPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Test whether a download entry matches all set criteria.
    pub fn matches(&self, entry: &DownloadEntry, priority: DownloadPriority) -> bool {
        if let Some(s) = self.state {
            if entry.state != s {
                return false;
            }
        }
        if let Some(ref pat) = self.url_contains {
            if !entry.request.url.contains(pat.as_str()) {
                return false;
            }
        }
        if let Some(p) = self.priority {
            if priority != p {
                return false;
            }
        }
        true
    }
}

impl DownloadService {
    /// Query entries using a filter. Returns matching entries.
    pub fn query(&self, filter: &DownloadFilter) -> Vec<&DownloadEntry> {
        self.entries
            .iter()
            .filter(|e| filter.matches(e, self.priority_for(e.id)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// DownloadSummary – aggregate view
// ---------------------------------------------------------------------------

/// High-level summary of the download service state.
#[derive(Debug, Clone)]
pub struct DownloadSummary {
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub total_bytes: u64,
    pub average_speed_bps: f64,
}

impl DownloadSummary {
    /// Total number of entries across all states.
    pub fn total(&self) -> usize {
        self.pending + self.in_progress + self.completed + self.failed + self.cancelled
    }

    /// Fraction of all entries that are complete.
    pub fn completion_ratio(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            return 0.0;
        }
        self.completed as f64 / total as f64
    }

    /// Estimated seconds to finish all remaining bytes at the recorded average
    /// speed. Returns `None` if speed is zero or unknown.
    pub fn estimate_remaining_secs(&self, remaining_bytes: u64) -> Option<f64> {
        if self.average_speed_bps <= 0.0 {
            return None;
        }
        Some(remaining_bytes as f64 / self.average_speed_bps)
    }
}

impl fmt::Display for DownloadSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Summary: {} total ({} done, {} active, {} pending, {} failed, {} cancelled), {:.0} B/s",
            self.total(),
            self.completed,
            self.in_progress,
            self.pending,
            self.failed,
            self.cancelled,
            self.average_speed_bps,
        )
    }
}

impl DownloadService {
    /// Build a summary of the current service state.
    pub fn summarize(&self, elapsed_secs: f64) -> DownloadSummary {
        let stats = self.get_stats();
        DownloadSummary {
            pending: stats.pending,
            in_progress: stats.in_progress,
            completed: stats.completed,
            failed: stats.failed,
            cancelled: stats.cancelled,
            total_bytes: stats.total_bytes,
            average_speed_bps: self.get_throughput(elapsed_secs),
        }
    }
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<&str> for DownloadRequest {
    /// Create a minimal request from a URL string, using the last path segment
    /// as the destination filename.
    fn from(url: &str) -> Self {
        let dest = UrlValidator::extract_filename(url)
            .unwrap_or_else(|| "download".to_string());
        Self {
            url: url.to_string(),
            destination: dest,
            headers: Vec::new(),
        }
    }
}

impl From<DownloadPriority> for u8 {
    fn from(p: DownloadPriority) -> Self {
        p.rank()
    }
}

// -- DownloadIntegrityChecker with hash verification -------------------------

/// Hash algorithm for integrity checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha256,
    Sha512,
    Md5,
}

impl fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HashAlgorithm::Sha256 => f.write_str("SHA-256"),
            HashAlgorithm::Sha512 => f.write_str("SHA-512"),
            HashAlgorithm::Md5 => f.write_str("MD5"),
        }
    }
}

/// Integrity check specification.
#[derive(Debug, Clone)]
pub struct IntegrityCheck {
    pub algorithm: HashAlgorithm,
    pub expected_hash: String,
}

impl IntegrityCheck {
    pub fn sha256(hash: &str) -> Self {
        Self { algorithm: HashAlgorithm::Sha256, expected_hash: hash.to_string() }
    }

    pub fn sha512(hash: &str) -> Self {
        Self { algorithm: HashAlgorithm::Sha512, expected_hash: hash.to_string() }
    }

    /// Verify a computed hash against the expected value.
    pub fn verify(&self, computed_hash: &str) -> bool {
        self.expected_hash.eq_ignore_ascii_case(computed_hash)
    }
}

impl fmt::Display for IntegrityCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let short = if self.expected_hash.len() > 16 {
            &self.expected_hash[..16]
        } else {
            &self.expected_hash
        };
        write!(f, "{}:{short}...", self.algorithm)
    }
}

// -- DownloadBandwidthThrottle -----------------------------------------------

/// Bandwidth throttle configuration.
#[derive(Debug, Clone)]
pub struct BandwidthThrottle {
    pub max_bytes_per_second: u64,
    pub enabled: bool,
}

impl BandwidthThrottle {
    pub fn new(max_bps: u64) -> Self {
        Self { max_bytes_per_second: max_bps, enabled: true }
    }

    pub fn unlimited() -> Self {
        Self { max_bytes_per_second: 0, enabled: false }
    }

    /// Calculate how long to wait given bytes transferred.
    pub fn delay_for_bytes(&self, bytes: u64, elapsed_ms: u64) -> u64 {
        if !self.enabled || self.max_bytes_per_second == 0 {
            return 0;
        }
        let expected_ms = (bytes * 1000) / self.max_bytes_per_second;
        if expected_ms > elapsed_ms {
            expected_ms - elapsed_ms
        } else {
            0
        }
    }

    /// Current effective rate limit in human-readable form.
    pub fn display_limit(&self) -> String {
        if !self.enabled {
            return "unlimited".to_string();
        }
        if self.max_bytes_per_second >= 1_048_576 {
            format!("{:.1} MB/s", self.max_bytes_per_second as f64 / 1_048_576.0)
        } else if self.max_bytes_per_second >= 1024 {
            format!("{:.1} KB/s", self.max_bytes_per_second as f64 / 1024.0)
        } else {
            format!("{} B/s", self.max_bytes_per_second)
        }
    }
}

impl fmt::Display for BandwidthThrottle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Throttle({})", self.display_limit())
    }
}

// -- Download proxy configuration --------------------------------------------

/// Proxy configuration for downloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub protocol: ProxyProtocol,
    pub auth: Option<ProxyAuth>,
    pub bypass_list: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyProtocol {
    Http,
    Https,
    Socks5,
}

impl fmt::Display for ProxyProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProxyProtocol::Http => f.write_str("http"),
            ProxyProtocol::Https => f.write_str("https"),
            ProxyProtocol::Socks5 => f.write_str("socks5"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyAuth {
    pub username: String,
    pub password_set: bool,
}

impl ProxyConfig {
    pub fn new(host: &str, port: u16, protocol: ProxyProtocol) -> Self {
        Self {
            host: host.to_string(),
            port,
            protocol,
            auth: None,
            bypass_list: Vec::new(),
        }
    }

    pub fn with_auth(mut self, username: &str) -> Self {
        self.auth = Some(ProxyAuth { username: username.to_string(), password_set: false });
        self
    }

    pub fn with_bypass(mut self, pattern: &str) -> Self {
        self.bypass_list.push(pattern.to_string());
        self
    }

    /// Check if a URL should bypass the proxy.
    pub fn should_bypass(&self, url: &str) -> bool {
        self.bypass_list.iter().any(|pattern| url.contains(pattern))
    }

    /// Format as a proxy URL.
    pub fn to_url(&self) -> String {
        format!("{}://{}:{}", self.protocol, self.host, self.port)
    }
}

impl fmt::Display for ProxyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Proxy({})", self.to_url())?;
        if self.auth.is_some() {
            write!(f, " [auth]")?;
        }
        if !self.bypass_list.is_empty() {
            write!(f, " [{} bypass rules]", self.bypass_list.len())?;
        }
        Ok(())
    }
}


// ---------------------------------------------------------------------------
// DownloadProgressReporter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DownloadProgressReporter {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl DownloadProgressReporter {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for DownloadProgressReporter {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for DownloadProgressReporter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "DownloadProgressReporter({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// DownloadQueueManager
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DownloadQueueManager {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl DownloadQueueManager {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for DownloadQueueManager {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for DownloadQueueManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "DownloadQueueManager({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// DownloadProgressReporterSnapshot — point-in-time snapshot of DownloadProgressReporter state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DownloadProgressReporterSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl DownloadProgressReporterSnapshot {
    pub fn capture(source: &DownloadProgressReporter, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for DownloadProgressReporterSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// DownloadQueueManagerStats — aggregate statistics for DownloadQueueManager
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct DownloadQueueManagerStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl DownloadQueueManagerStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for DownloadQueueManagerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// DownloadProgressReporterConfig — configuration for DownloadProgressReporter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DownloadProgressReporterConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl DownloadProgressReporterConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for DownloadProgressReporterConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for DownloadProgressReporterConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ── DownloadSpeedTracker ─────────────────────────────────────────────────

/// Tracks progress of a single download with speed and ETA calculation.
#[derive(Debug, Clone)]
pub struct DownloadSpeedTracker {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: f64,
}

impl DownloadSpeedTracker {
    pub fn new(bytes_downloaded: u64, total_bytes: Option<u64>, speed_bps: f64) -> Self {
        Self { bytes_downloaded, total_bytes, speed_bps }
    }

    /// Returns the percentage completed (0.0–100.0), or `None` if total is unknown.
    pub fn pct_complete(&self) -> Option<f64> {
        self.total_bytes.map(|total| {
            if total == 0 { 100.0 } else { (self.bytes_downloaded as f64 / total as f64) * 100.0 }
        })
    }

    /// Returns estimated seconds remaining, or `None` if unknown.
    pub fn eta_seconds(&self) -> Option<f64> {
        if self.speed_bps <= 0.0 { return None; }
        self.total_bytes.map(|total| {
            let remaining = total.saturating_sub(self.bytes_downloaded) as f64;
            remaining / self.speed_bps
        })
    }

    /// Formats speed in human-readable units (B/s, KB/s, MB/s, GB/s).
    pub fn human_readable_speed(&self) -> String {
        let s = self.speed_bps;
        if s >= 1_073_741_824.0 {
            format!("{:.2} GB/s", s / 1_073_741_824.0)
        } else if s >= 1_048_576.0 {
            format!("{:.2} MB/s", s / 1_048_576.0)
        } else if s >= 1024.0 {
            format!("{:.2} KB/s", s / 1024.0)
        } else {
            format!("{:.0} B/s", s)
        }
    }

    pub fn is_finished(&self) -> bool {
        self.total_bytes.map_or(false, |t| self.bytes_downloaded >= t)
    }
}

impl fmt::Display for DownloadSpeedTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.pct_complete() {
            Some(pct) => write!(f, "{:.1}% @ {}", pct, self.human_readable_speed()),
            None => write!(f, "{} bytes @ {}", self.bytes_downloaded, self.human_readable_speed()),
        }
    }
}

// ── DownloadQueue ────────────────────────────────────────────────────────

/// A queue of download URLs with priority management.
#[derive(Debug, Clone)]
pub struct DownloadQueue {
    pending: Vec<String>,
    active: Vec<String>,
    completed: Vec<String>,
}

impl DownloadQueue {
    pub fn new() -> Self {
        Self { pending: Vec::new(), active: Vec::new(), completed: Vec::new() }
    }

    pub fn add(&mut self, url: String) { self.pending.push(url); }

    pub fn remove(&mut self, url: &str) -> bool {
        if let Some(pos) = self.pending.iter().position(|u| u == url) {
            self.pending.remove(pos);
            return true;
        }
        false
    }

    /// Move a pending URL to the front of the queue (highest priority).
    pub fn prioritize(&mut self, url: &str) -> bool {
        if let Some(pos) = self.pending.iter().position(|u| u == url) {
            let item = self.pending.remove(pos);
            self.pending.insert(0, item);
            true
        } else {
            false
        }
    }

    pub fn start_next(&mut self) -> Option<String> {
        if self.pending.is_empty() { return None; }
        let url = self.pending.remove(0);
        self.active.push(url.clone());
        Some(url)
    }

    pub fn complete(&mut self, url: &str) -> bool {
        if let Some(pos) = self.active.iter().position(|u| u == url) {
            let item = self.active.remove(pos);
            self.completed.push(item);
            true
        } else {
            false
        }
    }

    pub fn pending_count(&self) -> usize { self.pending.len() }
    pub fn active_count(&self) -> usize { self.active.len() }
    pub fn completed_count(&self) -> usize { self.completed.len() }

    pub fn total_progress(&self) -> f64 {
        let total = self.pending.len() + self.active.len() + self.completed.len();
        if total == 0 { return 0.0; }
        self.completed.len() as f64 / total as f64 * 100.0
    }
}

// ── DownloadRetryTracker ─────────────────────────────────────────────────

/// Tracks per-URL failure counts and retry decisions.
#[derive(Debug, Clone)]
pub struct DownloadRetryTracker {
    attempts: HashMap<String, u32>,
    max_retries: u32,
}

impl DownloadRetryTracker {
    pub fn new(max_retries: u32) -> Self {
        Self { attempts: HashMap::new(), max_retries }
    }

    pub fn record_attempt(&mut self, url: &str) {
        *self.attempts.entry(url.to_string()).or_insert(0) += 1;
    }

    pub fn attempt_count(&self, url: &str) -> u32 {
        self.attempts.get(url).copied().unwrap_or(0)
    }

    pub fn should_retry(&self, url: &str) -> bool {
        self.attempt_count(url) < self.max_retries
    }

    pub fn reset(&mut self, url: &str) { self.attempts.remove(url); }

    pub fn reset_all(&mut self) { self.attempts.clear(); }

    pub fn failed_urls(&self) -> Vec<&str> {
        self.attempts
            .iter()
            .filter(|(_, count)| **count >= self.max_retries)
            .map(|(url, _)| url.as_str())
            .collect()
    }

    pub fn total_tracked(&self) -> usize { self.attempts.len() }
}


/// Configuration manager for download functionality.
pub struct DownloadConfigDetail {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl DownloadConfigDetail {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &DownloadConfigDetail) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for download operations.
pub struct DownloadRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl DownloadRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for download.
pub struct DownloadValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl DownloadValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &DownloadValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for download
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaDownloadRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaDownloadRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaDownloadCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaDownloadCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaDownloadCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 32
// ---------------------------------------------------------------------------

/// Generic object pool `Xc32Pool<T>`.
pub struct Xc32Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc32Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc32PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc32Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc32PoolStats {
        Xc32PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc32Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc32Scheduler`.
pub struct Xc32Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc32Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc32Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_32 hash for the given byte slice.
pub fn xc_32_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_32 convention.
pub fn xc_32_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_39 deepening: state machine + event bus ---

/// States for the Xd39 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd39State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd39State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd39Transition {
    pub from: Xd39State,
    pub to: Xd39State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd39StateMachine {
    current: Xd39State,
    history: Vec<Xd39Transition>,
    step_counter: usize,
}

impl Xd39StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd39State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd39State {
        self.current
    }

    pub fn history(&self) -> &[Xd39Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd39State) -> Result<Xd39State, String> {
        let allowed = match (self.current, target) {
            (Xd39State::Idle, Xd39State::Running) => true,
            (Xd39State::Running, Xd39State::Paused) => true,
            (Xd39State::Running, Xd39State::Done) => true,
            (Xd39State::Paused, Xd39State::Running) => true,
            (Xd39State::Paused, Xd39State::Done) => true,
            (Xd39State::Done, Xd39State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_39: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd39Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd39SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd39State> {
        let prefix = "Xd39SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd39State::Idle),
            "Running" => Some(Xd39State::Running),
            "Paused" => Some(Xd39State::Paused),
            "Done" => Some(Xd39State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd39State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd39 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd39Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd39Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd39HandlerFn = Box<dyn Fn(&Xd39Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd39EventBus {
    handlers: Vec<(usize, Option<String>, Xd39HandlerFn)>,
    next_id: usize,
    published: Vec<Xd39Event>,
}

impl Xd39EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd39Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd39Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd39Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd39Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #37
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf37Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf37TrieNode {
    children: std::collections::HashMap<char, Xf37TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf37Trie {
    root: Xf37TrieNode,
    count: usize,
}

impl Xf37Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf37TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf37TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf37TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf37BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf37BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 31).
pub struct Xh31SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh31SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 73 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 31).
pub struct Xh31BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh31BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 31).
pub struct Xi31Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi31Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi31Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi31Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 31).
pub struct Xi31IntervalTree {
    xi_intervals: Vec<Xi31Interval>,
}

impl Xi31IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi31Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi31Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi31Interval) -> Vec<&Xi31Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi31Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi31Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi31Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi31Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi31Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi31Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 31) ---

/// Disjoint set / union-find for crate 31.
pub struct Xj31UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj31UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ31_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 31.
pub struct Xj31BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj31BTreeNode<K, V>>>,
    len: usize,
}

struct Xj31BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj31BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj31BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ31_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ31_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj31BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj31BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj31BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj31BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_31 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk31SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk31SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk31DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk31DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_31).
#[derive(Debug, Clone)]
pub struct Xl31Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl31Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_31).
#[derive(Debug, Clone)]
pub struct Xl31SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl31SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm31MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm31MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm31Tokenizer {
    text: String,
}

impl Xm31Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 31.
pub struct Xn31Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn31Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 31 -----

#[derive(Debug, Clone)]
struct Xn31AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn31AvlNode<K, V>>>,
    right: Option<Box<Xn31AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 31.
#[derive(Debug, Clone)]
pub struct Xn31AVL<K, V> {
    root: Option<Box<Xn31AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn31AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn31AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn31AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn31AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn31AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn31AvlNode<K, V>>) -> Box<Xn31AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn31AvlNode<K, V>>) -> Box<Xn31AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn31AvlNode<K, V>>) -> Box<Xn31AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn31AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn31AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn31AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn31AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn31AvlNode<K, V>>) -> &Xn31AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn31AvlNode<K, V>>) -> (Box<Xn31AvlNode<K, V>>, Option<Box<Xn31AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn31AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn31AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn31AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn31AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn31AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn31AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn31AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo31RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo31Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo31RBNode<K, V> {
    key: K,
    value: V,
    color: Xo31Color,
    left: Option<Box<Xo31RBNode<K, V>>>,
    right: Option<Box<Xo31RBNode<K, V>>>,
}

/// A red-black tree map for crate 31.
#[derive(Debug, Clone)]
pub struct Xo31RedBlack<K, V> {
    root: Option<Box<Xo31RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo31RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo31Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo31RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo31RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo31RBNode {
                    key, value, color: Xo31Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo31RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo31Color::Red)
    }

    fn xo_balance(mut h: Box<Xo31RBNode<K, V>>) -> Box<Xo31RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo31Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo31RBNode<K, V>>) -> Box<Xo31RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo31Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo31RBNode<K, V>>) -> Box<Xo31RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo31Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo31RBNode<K, V>>) {
        h.color = Xo31Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo31Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo31Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo31Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo31RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo31RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo31RBNode<K, V>) -> (K, V, Option<Box<Xo31RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo31RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo31Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo31RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo31ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 31.
#[derive(Debug, Clone)]
pub struct Xo31ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo31ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo31#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo31#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 31).
#[derive(Debug)]
pub struct Xp31SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp31Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp31Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp31Node<K, V>>>,
    xp_right: Option<Box<Xp31Node<K, V>>>,
}

impl<K: Ord, V> Xp31Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp31SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp31SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp31Node<K, V>>>, key: &K) -> Option<Box<Xp31Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp31Node<K, V>>) -> Box<Xp31Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp31Node<K, V>>) -> Box<Xp31Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp31Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp31Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp31Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq31Treap ---------------

use std::cmp::Ordering as Xq31Ord;

struct Xq31TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq31TreapNode<K, V>>>,
    right: Option<Box<Xq31TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq31Treap<K, V> {
    root: Option<Box<Xq31TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq31TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_31_size<K, V>(node: &Option<Box<Xq31TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_31_update_size<K, V>(node: &mut Xq31TreapNode<K, V>) {
    node.size = 1 + xq_31_size(&node.left) + xq_31_size(&node.right);
}

fn xq_31_rotate_right<K, V>(mut node: Box<Xq31TreapNode<K, V>>) -> Box<Xq31TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_31_update_size(&mut node);
    left.right = Some(node);
    xq_31_update_size(&mut left);
    left
}

fn xq_31_rotate_left<K, V>(mut node: Box<Xq31TreapNode<K, V>>) -> Box<Xq31TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_31_update_size(&mut node);
    right.left = Some(node);
    xq_31_update_size(&mut right);
    right
}

fn xq_31_insert_node<K: Ord, V>(
    node: Option<Box<Xq31TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq31TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq31TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq31Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq31Ord::Less => {
                let (new_left, old) = xq_31_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_31_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_31_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq31Ord::Greater => {
                let (new_right, old) = xq_31_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_31_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_31_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_31_remove_node<K: Ord, V>(
    node: Option<Box<Xq31TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq31TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq31Ord::Less => {
                let (new_left, old) = xq_31_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_31_update_size(&mut n);
                (Some(n), old)
            }
            Xq31Ord::Greater => {
                let (new_right, old) = xq_31_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_31_update_size(&mut n);
                (Some(n), old)
            }
            Xq31Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_31_rotate_right(n);
                    let (new_right, old) = xq_31_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_31_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_31_rotate_left(n);
                    let (new_left, old) = xq_31_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_31_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_31_find_min<K, V>(node: &Option<Box<Xq31TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_31_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_31_find_max<K, V>(node: &Option<Box<Xq31TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_31_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_31_rank<K: Ord, V>(node: &Option<Box<Xq31TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq31Ord::Less => xq_31_rank(&n.left, key),
            Xq31Ord::Equal => xq_31_size(&n.left),
            Xq31Ord::Greater => 1 + xq_31_size(&n.left) + xq_31_rank(&n.right, key),
        },
    }
}

fn xq_31_kth<K, V>(node: &Option<Box<Xq31TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_31_size(&n.left);
        if k < left_size {
            xq_31_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_31_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_31_in_order<K: Clone, V>(node: &Option<Box<Xq31TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_31_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_31_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq31Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 31 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_31_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq31Ord::Equal => return Some(&n.value),
                Xq31Ord::Less => cur = &n.left,
                Xq31Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_31_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_31_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_31_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_31_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_31_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_31_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_31_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq31VEBTree ---------------

pub struct Xq31VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq31VEBTree>>,
    clusters: Vec<Option<Box<Xq31VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq31VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq31VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq31VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr31KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr31KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr31BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr31KDNode {
    xr_point: Xr31KDPoint,
    xr_left: Option<Box<Xr31KDNode>>,
    xr_right: Option<Box<Xr31KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr31KDTree {
    xr_root: Option<Box<Xr31KDNode>>,
    xr_size: usize,
}

impl Xr31KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr31KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr31KDNode>>,
        point: Xr31KDPoint,
        depth: usize,
    ) -> Box<Xr31KDNode> {
        match node {
            None => Box::new(Xr31KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr31KDPoint) -> Option<Xr31KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr31KDNode>,
        query: &Xr31KDPoint,
        depth: usize,
        best: &mut Xr31KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr31KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr31KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr31KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr31KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr31KDNode>>, pts: &mut Vec<Xr31KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr31KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr31BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr31BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs31PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs31PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs31PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs31PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs31ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs31ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs31ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs31RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs31RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs31RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs31CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs31CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs31CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
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

    // -----------------------------------------------------------------------
    // Tests for newly added functionality
    // -----------------------------------------------------------------------

    #[test]
    fn progress_is_complete() {
        let mut p = DownloadProgress::new();
        assert!(!p.is_complete());

        p.total_bytes = Some(100);
        p.bytes_downloaded = 50;
        assert!(!p.is_complete());

        p.bytes_downloaded = 100;
        assert!(p.is_complete());

        // Over-download still counts as complete.
        p.bytes_downloaded = 120;
        assert!(p.is_complete());

        // Unknown total is never complete.
        p.total_bytes = None;
        assert!(!p.is_complete());
    }

    #[test]
    fn progress_remaining() {
        let mut p = DownloadProgress::new();
        assert_eq!(p.remaining(), None);

        p.total_bytes = Some(1000);
        p.bytes_downloaded = 300;
        assert_eq!(p.remaining(), Some(700));

        p.bytes_downloaded = 1000;
        assert_eq!(p.remaining(), Some(0));

        // Over-download saturates to 0.
        p.bytes_downloaded = 1500;
        assert_eq!(p.remaining(), Some(0));
    }

    #[test]
    fn get_entry_mut_modifies_entry() {
        let mut svc = DownloadService::new();
        let id = svc.enqueue(sample_request());

        let entry = svc.get_entry_mut(id).unwrap();
        entry.state = DownloadState::InProgress;
        assert_eq!(svc.get_state(id), Some(DownloadState::InProgress));

        assert!(svc.get_entry_mut(999).is_none());
    }

    #[test]
    fn is_empty_and_total_count() {
        let mut svc = DownloadService::new();
        assert!(svc.is_empty());
        assert_eq!(svc.total_count(), 0);

        svc.enqueue(sample_request());
        assert!(!svc.is_empty());
        assert_eq!(svc.total_count(), 1);

        svc.enqueue(sample_request());
        assert_eq!(svc.total_count(), 2);
    }

    #[test]
    fn success_rate_calculation() {
        let stats_none = DownloadStats {
            total: 0,
            pending: 0,
            in_progress: 0,
            completed: 0,
            failed: 0,
            cancelled: 0,
            total_bytes: 0,
        };
        assert_eq!(stats_none.success_rate(), 0.0);

        let stats_all = DownloadStats {
            total: 5,
            pending: 0,
            in_progress: 0,
            completed: 5,
            failed: 0,
            cancelled: 0,
            total_bytes: 500,
        };
        assert!((stats_all.success_rate() - 1.0).abs() < f64::EPSILON);

        let stats_mixed = DownloadStats {
            total: 4,
            pending: 0,
            in_progress: 0,
            completed: 3,
            failed: 1,
            cancelled: 0,
            total_bytes: 300,
        };
        assert!((stats_mixed.success_rate() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn download_state_display() {
        assert_eq!(DownloadState::Pending.to_string(), "Pending");
        assert_eq!(DownloadState::InProgress.to_string(), "In Progress");
        assert_eq!(DownloadState::Completed.to_string(), "Completed");
        assert_eq!(DownloadState::Failed.to_string(), "Failed");
        assert_eq!(DownloadState::Cancelled.to_string(), "Cancelled");
    }

    #[test]
    fn download_stats_display() {
        let stats = DownloadStats {
            total: 10,
            pending: 2,
            in_progress: 1,
            completed: 5,
            failed: 1,
            cancelled: 1,
            total_bytes: 4096,
        };
        let s = stats.to_string();
        assert!(s.contains("10 total"));
        assert!(s.contains("5 completed"));
        assert!(s.contains("1 failed"));
        assert!(s.contains("4096 bytes"));
    }

    #[test]
    fn find_by_url_returns_matching_entries() {
        let mut svc = DownloadService::new();
        let url_a = "https://example.com/a.bin";
        let url_b = "https://example.com/b.bin";

        let req_a = DownloadRequest {
            url: url_a.into(),
            destination: "/tmp/a.bin".into(),
            headers: vec![],
        };
        let req_a2 = DownloadRequest {
            url: url_a.into(),
            destination: "/tmp/a2.bin".into(),
            headers: vec![],
        };
        let req_b = DownloadRequest {
            url: url_b.into(),
            destination: "/tmp/b.bin".into(),
            headers: vec![],
        };

        svc.enqueue(req_a);
        svc.enqueue(req_a2);
        svc.enqueue(req_b);

        let matches = svc.find_by_url(url_a);
        assert_eq!(matches.len(), 2);
        for entry in &matches {
            assert_eq!(entry.request.url, url_a);
        }

        assert_eq!(svc.find_by_url(url_b).len(), 1);
        assert_eq!(svc.find_by_url("https://nope.com").len(), 0);
    }

    // -----------------------------------------------------------------------
    // Tests for DownloadBandwidth, DownloadRetryPolicy, DownloadFilter,
    // DownloadSummary, and From impls
    // -----------------------------------------------------------------------

    #[test]
    fn bandwidth_rolling_window_average() {
        let mut bw = DownloadBandwidth::new(4);
        assert_eq!(bw.average_bps(), 0.0);
        assert_eq!(bw.sample_count(), 0);

        // t=0: baseline, t=1: 1000B, t=2: 2000B, t=3: 3000B
        bw.record(0.0, 0);
        bw.record(1.0, 1000);
        bw.record(2.0, 2000);
        bw.record(3.0, 3000);
        // total bytes in window (excluding first) = 1000+2000+3000 = 6000
        // dt = 3.0 - 0.0 = 3.0 → 2000 B/s
        assert!((bw.average_bps() - 2000.0).abs() < f64::EPSILON);
        assert_eq!(bw.sample_count(), 4);

        // Add one more sample – window_size=4 so oldest is evicted.
        bw.record(4.0, 4000);
        assert_eq!(bw.sample_count(), 4);
        // Window is now [(1,1000),(2,2000),(3,3000),(4,4000)]
        // bytes = 2000+3000+4000 = 9000, dt = 4-1 = 3 → 3000 B/s
        assert!((bw.average_bps() - 3000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn bandwidth_estimate_remaining() {
        let mut bw = DownloadBandwidth::new(10);
        // No samples → None
        assert!(bw.estimate_remaining(1000).is_none());

        bw.record(0.0, 0);
        bw.record(2.0, 1000);
        // 500 B/s → 10_000 bytes remaining ≈ 20 seconds
        let est = bw.estimate_remaining(10_000).unwrap();
        assert!((est - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn bandwidth_display_formatting() {
        let mut bw = DownloadBandwidth::new(10);
        bw.record(0.0, 0);
        bw.record(1.0, 2_000_000); // 2 MB/s
        let s = bw.to_string();
        assert!(s.contains("MB/s"), "expected MB/s in: {s}");

        let mut bw2 = DownloadBandwidth::new(10);
        bw2.record(0.0, 0);
        bw2.record(1.0, 5120); // 5 KB/s
        let s2 = bw2.to_string();
        assert!(s2.contains("KB/s"), "expected KB/s in: {s2}");

        let mut bw3 = DownloadBandwidth::new(10);
        bw3.record(0.0, 0);
        bw3.record(1.0, 100); // 100 B/s
        let s3 = bw3.to_string();
        assert!(s3.contains("B/s"), "expected B/s in: {s3}");
    }

    #[test]
    fn retry_policy_linear_backoff() {
        let policy = DownloadRetryPolicy {
            max_retries: 4,
            strategy: BackoffStrategy::Linear,
            base_delay_secs: 2.0,
            max_delay_secs: 10.0,
        };
        assert_eq!(policy.delay_for_attempt(0), None); // invalid
        assert!((policy.delay_for_attempt(1).unwrap() - 2.0).abs() < f64::EPSILON);
        assert!((policy.delay_for_attempt(2).unwrap() - 4.0).abs() < f64::EPSILON);
        assert!((policy.delay_for_attempt(3).unwrap() - 6.0).abs() < f64::EPSILON);
        // Attempt 4: 2*4=8, under cap
        assert!((policy.delay_for_attempt(4).unwrap() - 8.0).abs() < f64::EPSILON);
        // Attempt 5 exceeds max_retries
        assert_eq!(policy.delay_for_attempt(5), None);

        assert!(!policy.is_exhausted(2));
        assert!(policy.is_exhausted(4));
    }

    #[test]
    fn retry_policy_exponential_backoff_with_cap() {
        let policy = DownloadRetryPolicy {
            max_retries: 5,
            strategy: BackoffStrategy::Exponential,
            base_delay_secs: 1.0,
            max_delay_secs: 10.0,
        };
        // 1*2^0=1, 1*2^1=2, 1*2^2=4, 1*2^3=8, 1*2^4=16 → capped to 10
        assert!((policy.delay_for_attempt(1).unwrap() - 1.0).abs() < f64::EPSILON);
        assert!((policy.delay_for_attempt(2).unwrap() - 2.0).abs() < f64::EPSILON);
        assert!((policy.delay_for_attempt(3).unwrap() - 4.0).abs() < f64::EPSILON);
        assert!((policy.delay_for_attempt(4).unwrap() - 8.0).abs() < f64::EPSILON);
        assert!((policy.delay_for_attempt(5).unwrap() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn retry_policy_display() {
        let policy = DownloadRetryPolicy::default();
        let s = policy.to_string();
        assert!(s.contains("max=3"));
        assert!(s.contains("Exponential"));
    }

    #[test]
    fn filter_matches_by_state() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());
        svc.fail(id1);

        let filter = DownloadFilter::new().with_state(DownloadState::Failed);
        let results = svc.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id1);

        let filter_pending = DownloadFilter::new().with_state(DownloadState::Pending);
        let results2 = svc.query(&filter_pending);
        assert_eq!(results2.len(), 1);
        assert_eq!(results2[0].id, id2);
    }

    #[test]
    fn filter_matches_by_url_substring() {
        let mut svc = DownloadService::new();
        svc.enqueue(DownloadRequest {
            url: "https://cdn.example.com/video.mp4".into(),
            destination: "/tmp/video.mp4".into(),
            headers: vec![],
        });
        svc.enqueue(DownloadRequest {
            url: "https://cdn.example.com/audio.mp3".into(),
            destination: "/tmp/audio.mp3".into(),
            headers: vec![],
        });
        svc.enqueue(DownloadRequest {
            url: "https://other.com/file.zip".into(),
            destination: "/tmp/file.zip".into(),
            headers: vec![],
        });

        let filter = DownloadFilter::new().with_url_contains("cdn.example.com");
        assert_eq!(svc.query(&filter).len(), 2);

        let filter2 = DownloadFilter::new().with_url_contains("audio");
        assert_eq!(svc.query(&filter2).len(), 1);

        let filter3 = DownloadFilter::new().with_url_contains("nope");
        assert_eq!(svc.query(&filter3).len(), 0);
    }

    #[test]
    fn filter_combined_state_and_url() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(DownloadRequest {
            url: "https://cdn.example.com/a.bin".into(),
            destination: "/tmp/a.bin".into(),
            headers: vec![],
        });
        svc.enqueue(DownloadRequest {
            url: "https://cdn.example.com/b.bin".into(),
            destination: "/tmp/b.bin".into(),
            headers: vec![],
        });
        svc.fail(id1);

        let filter = DownloadFilter::new()
            .with_state(DownloadState::Failed)
            .with_url_contains("cdn");
        let results = svc.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id1);
    }

    #[test]
    fn summary_totals_and_completion_ratio() {
        let mut svc = DownloadService::new();
        let id1 = svc.enqueue(sample_request());
        let id2 = svc.enqueue(sample_request());
        let id3 = svc.enqueue(sample_request());
        let _id4 = svc.enqueue(sample_request());

        svc.complete(id1, "/tmp/a".into(), 500);
        svc.complete(id2, "/tmp/b".into(), 300);
        svc.fail(id3);

        let summary = svc.summarize(4.0);
        assert_eq!(summary.total(), 4);
        assert_eq!(summary.completed, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.pending, 1);
        assert!((summary.completion_ratio() - 0.5).abs() < f64::EPSILON);
        // 800 bytes / 4 sec = 200 B/s
        assert!((summary.average_speed_bps - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn summary_estimate_remaining() {
        let summary = DownloadSummary {
            pending: 1,
            in_progress: 1,
            completed: 2,
            failed: 0,
            cancelled: 0,
            total_bytes: 1000,
            average_speed_bps: 250.0,
        };
        // 2000 remaining bytes at 250 B/s = 8 seconds
        let est = summary.estimate_remaining_secs(2000).unwrap();
        assert!((est - 8.0).abs() < f64::EPSILON);

        let zero_speed = DownloadSummary {
            average_speed_bps: 0.0,
            ..summary
        };
        assert!(zero_speed.estimate_remaining_secs(2000).is_none());
    }

    #[test]
    fn summary_display() {
        let summary = DownloadSummary {
            pending: 1,
            in_progress: 2,
            completed: 3,
            failed: 0,
            cancelled: 0,
            total_bytes: 0,
            average_speed_bps: 1024.0,
        };
        let s = summary.to_string();
        assert!(s.contains("6 total"));
        assert!(s.contains("3 done"));
        assert!(s.contains("2 active"));
    }

    #[test]
    fn from_str_for_download_request() {
        let req = DownloadRequest::from("https://example.com/path/archive.tar.gz");
        assert_eq!(req.url, "https://example.com/path/archive.tar.gz");
        assert_eq!(req.destination, "archive.tar.gz");
        assert!(req.headers.is_empty());

        // URL without filename segment
        let req2 = DownloadRequest::from("https://example.com/");
        assert_eq!(req2.destination, "download");
    }

    #[test]
    fn from_priority_to_u8() {
        assert_eq!(u8::from(DownloadPriority::Low), 0);
        assert_eq!(u8::from(DownloadPriority::Normal), 1);
        assert_eq!(u8::from(DownloadPriority::High), 2);
        assert_eq!(u8::from(DownloadPriority::Urgent), 3);
    }

    // -- DownloadRetryPolicy additional tests -----------------------------------

    #[test]
    fn retry_policy_exponential_backoff_delays() {
        let policy = DownloadRetryPolicy::default();
        // 1-based: attempt 1 => base_delay * 2^0 = 1.0
        assert_eq!(policy.delay_for_attempt(1), Some(1.0));
        // attempt 2 => base_delay * 2^1 = 2.0
        assert_eq!(policy.delay_for_attempt(2), Some(2.0));
        // attempt 3 => base_delay * 2^2 = 4.0
        assert_eq!(policy.delay_for_attempt(3), Some(4.0));
    }

    #[test]
    fn retry_policy_max_delay_capped() {
        let policy = DownloadRetryPolicy {
            max_retries: 10,
            strategy: BackoffStrategy::Exponential,
            base_delay_secs: 10.0,
            max_delay_secs: 15.0,
        };
        // attempt 5 => 10 * 2^4 = 160, capped to 15
        assert_eq!(policy.delay_for_attempt(5), Some(15.0));
    }

    #[test]
    fn retry_policy_is_exhausted_check() {
        let policy = DownloadRetryPolicy::default();
        assert!(!policy.is_exhausted(0));
        assert!(!policy.is_exhausted(2));
        assert!(policy.is_exhausted(3));
    }

    #[test]
    fn retry_policy_display_format() {
        let policy = DownloadRetryPolicy::default();
        let s = policy.to_string();
        assert!(s.contains("max=3"));
    }

    // -- IntegrityCheck tests -------------------------------------------------

    #[test]
    fn integrity_check_verify_match() {
        let check = IntegrityCheck::sha256("abc123");
        assert!(check.verify("ABC123"));
        assert!(!check.verify("xyz"));
    }

    #[test]
    fn integrity_check_display() {
        let check = IntegrityCheck::sha256("abcdef1234567890abcdef");
        let s = check.to_string();
        assert!(s.contains("SHA-256"));
    }

    #[test]
    fn hash_algorithm_display() {
        assert_eq!(HashAlgorithm::Sha256.to_string(), "SHA-256");
        assert_eq!(HashAlgorithm::Md5.to_string(), "MD5");
    }

    // -- BandwidthThrottle tests ----------------------------------------------

    #[test]
    fn throttle_delay_calculation() {
        let throttle = BandwidthThrottle::new(1000);
        let delay = throttle.delay_for_bytes(2000, 1000);
        assert_eq!(delay, 1000);
    }

    #[test]
    fn throttle_unlimited_no_delay() {
        let throttle = BandwidthThrottle::unlimited();
        assert_eq!(throttle.delay_for_bytes(1_000_000, 0), 0);
    }

    #[test]
    fn throttle_display_limit() {
        let throttle = BandwidthThrottle::new(1_048_576);
        assert!(throttle.display_limit().contains("MB/s"));
        let small = BandwidthThrottle::new(512);
        assert!(small.display_limit().contains("B/s"));
    }

    // -- ProxyConfig tests ----------------------------------------------------

    #[test]
    fn proxy_to_url() {
        let proxy = ProxyConfig::new("proxy.example.com", 8080, ProxyProtocol::Http);
        assert_eq!(proxy.to_url(), "http://proxy.example.com:8080");
    }

    #[test]
    fn proxy_bypass() {
        let proxy = ProxyConfig::new("proxy.example.com", 8080, ProxyProtocol::Http)
            .with_bypass("localhost")
            .with_bypass("internal.corp");
        assert!(proxy.should_bypass("http://localhost:3000"));
        assert!(!proxy.should_bypass("http://example.com"));
    }

    #[test]
    fn proxy_with_auth() {
        let proxy = ProxyConfig::new("proxy.example.com", 8080, ProxyProtocol::Https)
            .with_auth("user");
        assert!(proxy.auth.is_some());
    }

    #[test]
    fn proxy_display() {
        let proxy = ProxyConfig::new("host", 80, ProxyProtocol::Socks5)
            .with_auth("user")
            .with_bypass("localhost");
        let s = proxy.to_string();
        assert!(s.contains("socks5"));
        assert!(s.contains("[auth]"));
        assert!(s.contains("1 bypass"));
    }

    #[test]
    fn proxy_protocol_display() {
        assert_eq!(ProxyProtocol::Http.to_string(), "http");
        assert_eq!(ProxyProtocol::Socks5.to_string(), "socks5");
    }

    #[test] fn downloadProgressReporter_new() { let s = DownloadProgressReporter::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn downloadProgressReporter_add() { let mut s = DownloadProgressReporter::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn downloadProgressReporter_remove() { let mut s = DownloadProgressReporter::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn downloadProgressReporter_config() { let mut s = DownloadProgressReporter::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn downloadProgressReporter_nav() { let mut s = DownloadProgressReporter::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn downloadProgressReporter_filter() { let mut s = DownloadProgressReporter::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn downloadProgressReporter_display() { assert!(format!("{}", DownloadProgressReporter::new()).contains("DownloadProgressReporter")); }
    #[test] fn downloadQueueManager_new() { let s = DownloadQueueManager::new(); assert!(s.is_empty()); }
    #[test] fn downloadQueueManager_add() { let mut s = DownloadQueueManager::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn downloadQueueManager_active() { let mut s = DownloadQueueManager::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn downloadQueueManager_error() { let mut s = DownloadQueueManager::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn downloadQueueManager_rm_group() { let mut s = DownloadQueueManager::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn downloadQueueManager_display() { assert!(format!("{}", DownloadQueueManager::new()).contains("DownloadQueueManager")); }


    #[test] fn downloadProgressReporter_snap_capture() {
        let s = DownloadProgressReporter::new();
        let snap = DownloadProgressReporterSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn downloadProgressReporter_snap_stale() {
        let s = DownloadProgressReporter::new();
        let snap = DownloadProgressReporterSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn downloadProgressReporter_snap_diff() {
        let s = DownloadProgressReporter::new();
        let s1v = DownloadProgressReporterSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn downloadProgressReporter_snap_display() {
        let s = DownloadProgressReporter::new();
        let snap = DownloadProgressReporterSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn downloadQueueManager_stats_record() {
        let mut st = DownloadQueueManagerStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn downloadQueueManager_stats_hit_ratio() {
        let mut st = DownloadQueueManagerStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn downloadQueueManager_stats_merge() {
        let mut a = DownloadQueueManagerStats::new();
        a.total_adds = 5;
        let mut b = DownloadQueueManagerStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn downloadQueueManager_stats_display() {
        let st = DownloadQueueManagerStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn downloadProgressReporter_config_default() {
        let c = DownloadProgressReporterConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn downloadProgressReporter_config_builder() {
        let c = DownloadProgressReporterConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn downloadProgressReporter_config_labels() {
        let mut c = DownloadProgressReporterConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn downloadProgressReporter_config_cleanup_threshold() {
        let c = DownloadProgressReporterConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn downloadProgressReporter_config_display() {
        assert!(format!("{}", DownloadProgressReporterConfig::new()).contains("Config"));
    }
    #[test] fn downloadQueueManager_stats_peaks() {
        let mut st = DownloadQueueManagerStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // ── DownloadSpeedTracker tests ──

    #[test]
    fn speed_tracker_pct_known_total() {
        let p = DownloadSpeedTracker::new(50, Some(200), 1000.0);
        assert!((p.pct_complete().unwrap() - 25.0).abs() < 0.01);
    }

    #[test]
    fn speed_tracker_pct_unknown_total() {
        let p = DownloadSpeedTracker::new(50, None, 1000.0);
        assert!(p.pct_complete().is_none());
    }

    #[test]
    fn speed_tracker_pct_zero_total() {
        let p = DownloadSpeedTracker::new(0, Some(0), 0.0);
        assert!((p.pct_complete().unwrap() - 100.0).abs() < 0.01);
    }

    #[test]
    fn speed_tracker_eta_seconds() {
        let p = DownloadSpeedTracker::new(100, Some(500), 100.0);
        assert!((p.eta_seconds().unwrap() - 4.0).abs() < 0.01);
    }

    #[test]
    fn speed_tracker_eta_zero_speed() {
        let p = DownloadSpeedTracker::new(100, Some(500), 0.0);
        assert!(p.eta_seconds().is_none());
    }

    #[test]
    fn speed_tracker_human_speed_bytes() {
        let p = DownloadSpeedTracker::new(0, None, 512.0);
        assert_eq!(p.human_readable_speed(), "512 B/s");
    }

    #[test]
    fn speed_tracker_human_speed_mb() {
        let p = DownloadSpeedTracker::new(0, None, 5_242_880.0);
        assert_eq!(p.human_readable_speed(), "5.00 MB/s");
    }

    #[test]
    fn speed_tracker_is_finished() {
        assert!(DownloadSpeedTracker::new(100, Some(100), 0.0).is_finished());
        assert!(!DownloadSpeedTracker::new(50, Some(100), 0.0).is_finished());
        assert!(!DownloadSpeedTracker::new(50, None, 0.0).is_finished());
    }

    #[test]
    fn speed_tracker_display() {
        let p = DownloadSpeedTracker::new(50, Some(100), 1024.0);
        let s = format!("{}", p);
        assert!(s.contains("50.0%"));
        assert!(s.contains("KB/s"));
    }

    // ── DownloadQueue tests ──

    #[test]
    fn queue_add_and_counts() {
        let mut q = DownloadQueue::new();
        q.add("http://a.com".into());
        q.add("http://b.com".into());
        assert_eq!(q.pending_count(), 2);
        assert_eq!(q.active_count(), 0);
        assert_eq!(q.completed_count(), 0);
    }

    #[test]
    fn queue_start_and_complete() {
        let mut q = DownloadQueue::new();
        q.add("http://a.com".into());
        let url = q.start_next().unwrap();
        assert_eq!(url, "http://a.com");
        assert_eq!(q.active_count(), 1);
        assert!(q.complete("http://a.com"));
        assert_eq!(q.completed_count(), 1);
        assert_eq!(q.active_count(), 0);
    }

    #[test]
    fn queue_prioritize() {
        let mut q = DownloadQueue::new();
        q.add("http://a.com".into());
        q.add("http://b.com".into());
        q.prioritize("http://b.com");
        assert_eq!(q.start_next().unwrap(), "http://b.com");
    }

    #[test]
    fn queue_remove() {
        let mut q = DownloadQueue::new();
        q.add("http://a.com".into());
        assert!(q.remove("http://a.com"));
        assert!(!q.remove("http://a.com"));
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn queue_total_progress() {
        let mut q = DownloadQueue::new();
        q.add("http://a.com".into());
        q.add("http://b.com".into());
        q.start_next();
        q.complete("http://a.com");
        assert!((q.total_progress() - 50.0).abs() < 0.01);
    }

    // ── DownloadRetryTracker tests ──

    #[test]
    fn retry_tracker_should_retry() {
        let mut t = DownloadRetryTracker::new(3);
        assert!(t.should_retry("http://a.com"));
        t.record_attempt("http://a.com");
        t.record_attempt("http://a.com");
        assert!(t.should_retry("http://a.com"));
        t.record_attempt("http://a.com");
        assert!(!t.should_retry("http://a.com"));
    }

    #[test]
    fn retry_tracker_reset() {
        let mut t = DownloadRetryTracker::new(2);
        t.record_attempt("http://a.com");
        t.record_attempt("http://a.com");
        t.reset("http://a.com");
        assert!(t.should_retry("http://a.com"));
        assert_eq!(t.attempt_count("http://a.com"), 0);
    }

    #[test]
    fn retry_tracker_failed_urls() {
        let mut t = DownloadRetryTracker::new(1);
        t.record_attempt("http://a.com");
        t.record_attempt("http://b.com");
        let failed = t.failed_urls();
        assert_eq!(failed.len(), 2);
    }

    #[test]
    fn download_config_new() {
        let cfg = DownloadConfigDetail::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn download_config_set_get() {
        let mut cfg = DownloadConfigDetail::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn download_config_remove() {
        let mut cfg = DownloadConfigDetail::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn download_config_keys_sorted() {
        let mut cfg = DownloadConfigDetail::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn download_config_bump_version() {
        let mut cfg = DownloadConfigDetail::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn download_config_clear() {
        let mut cfg = DownloadConfigDetail::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn download_config_merge() {
        let mut cfg1 = DownloadConfigDetail::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = DownloadConfigDetail::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn download_config_disable() {
        let mut cfg = DownloadConfigDetail::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn download_rate_tracker_empty() {
        let rt = DownloadRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn download_rate_tracker_record() {
        let mut rt = DownloadRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn download_rate_tracker_prune() {
        let mut rt = DownloadRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn download_validator_valid() {
        let v = DownloadValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn download_validator_errors() {
        let mut v = DownloadValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn download_validator_clear() {
        let mut v = DownloadValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn download_validator_merge() {
        let mut v1 = DownloadValidator::new();
        v1.add_error("e1");
        let mut v2 = DownloadValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn download_rate_tracker_clear() {
        let mut rt = DownloadRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for download
    #[test]
    fn xa_download_ring_new() {
        let rb = super::XaDownloadRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_download_ring_push_len() {
        let mut rb = super::XaDownloadRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_download_ring_wrap() {
        let mut rb = super::XaDownloadRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_download_ring_mean_empty() {
        let rb = super::XaDownloadRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_download_ring_mean_values() {
        let mut rb = super::XaDownloadRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_download_ring_min_max() {
        let mut rb = super::XaDownloadRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_download_ring_iter() {
        let mut rb = super::XaDownloadRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_download_counter_new() {
        let c = super::XaDownloadCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_download_counter_inc() {
        let mut c = super::XaDownloadCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_download_counter_inc_by() {
        let mut c = super::XaDownloadCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_download_counter_reset() {
        let mut c = super::XaDownloadCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_download_counter_clear() {
        let mut c = super::XaDownloadCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_download_counter_default() {
        let c = super::XaDownloadCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 32 ----

    #[test]
    fn xc_32_pool_new_empty() {
        let pool: super::Xc32Pool<i32> = super::Xc32Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_32_pool_release_acquire() {
        let mut pool = super::Xc32Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_32_pool_acquire_empty() {
        let mut pool: super::Xc32Pool<i32> = super::Xc32Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_32_pool_full() {
        let mut pool = super::Xc32Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_32_pool_drain() {
        let mut pool = super::Xc32Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_32_pool_stats() {
        let mut pool = super::Xc32Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_32_pool_clear() {
        let mut pool = super::Xc32Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_32_pool_shrink() {
        let mut pool = super::Xc32Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_32_pool_default() {
        let pool: super::Xc32Pool<String> = super::Xc32Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_32_pool_extend() {
        let mut pool = super::Xc32Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_32_pool_retain() {
        let mut pool = super::Xc32Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_32_scheduler_round_robin() {
        let mut sched = super::Xc32Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_32_scheduler_empty() {
        let mut sched = super::Xc32Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_32_scheduler_reset() {
        let mut sched = super::Xc32Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_32_scheduler_add_remove() {
        let mut sched = super::Xc32Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_32_scheduler_targets() {
        let sched = super::Xc32Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_32_hash_empty() {
        assert_eq!(super::xc_32_hash(b""), 5381);
    }

    #[test]
    fn xc_32_hash_data() {
        let h = super::xc_32_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_32_hash(b"hello"), h);
    }

    #[test]
    fn xc_32_reverse_str() {
        assert_eq!(super::xc_32_reverse("abc"), "cba");
        assert_eq!(super::xc_32_reverse(""), "");
    }


    // --- xd_39 deepening tests ---

    #[test]
    fn xd_39_sm_initial_state() {
        let sm = Xd39StateMachine::new();
        assert_eq!(sm.current_state(), Xd39State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_39_sm_valid_idle_to_running() {
        let mut sm = Xd39StateMachine::new();
        assert!(sm.transition(Xd39State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd39State::Running);
    }

    #[test]
    fn xd_39_sm_valid_running_to_paused() {
        let mut sm = Xd39StateMachine::new();
        sm.transition(Xd39State::Running).unwrap();
        assert!(sm.transition(Xd39State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd39State::Paused);
    }

    #[test]
    fn xd_39_sm_valid_running_to_done() {
        let mut sm = Xd39StateMachine::new();
        sm.transition(Xd39State::Running).unwrap();
        assert!(sm.transition(Xd39State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd39State::Done);
    }

    #[test]
    fn xd_39_sm_valid_paused_to_running() {
        let mut sm = Xd39StateMachine::new();
        sm.transition(Xd39State::Running).unwrap();
        sm.transition(Xd39State::Paused).unwrap();
        assert!(sm.transition(Xd39State::Running).is_ok());
    }

    #[test]
    fn xd_39_sm_valid_done_to_idle() {
        let mut sm = Xd39StateMachine::new();
        sm.transition(Xd39State::Running).unwrap();
        sm.transition(Xd39State::Done).unwrap();
        assert!(sm.transition(Xd39State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd39State::Idle);
    }

    #[test]
    fn xd_39_sm_invalid_idle_to_done() {
        let mut sm = Xd39StateMachine::new();
        assert!(sm.transition(Xd39State::Done).is_err());
    }

    #[test]
    fn xd_39_sm_invalid_idle_to_paused() {
        let mut sm = Xd39StateMachine::new();
        assert!(sm.transition(Xd39State::Paused).is_err());
    }

    #[test]
    fn xd_39_sm_history_tracking() {
        let mut sm = Xd39StateMachine::new();
        sm.transition(Xd39State::Running).unwrap();
        sm.transition(Xd39State::Paused).unwrap();
        sm.transition(Xd39State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd39State::Idle);
        assert_eq!(sm.history()[0].to, Xd39State::Running);
        assert_eq!(sm.history()[1].from, Xd39State::Running);
        assert_eq!(sm.history()[2].to, Xd39State::Done);
    }

    #[test]
    fn xd_39_sm_serialize_deserialize() {
        let mut sm = Xd39StateMachine::new();
        sm.transition(Xd39State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd39StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd39State::Running));
    }

    #[test]
    fn xd_39_sm_deserialize_invalid() {
        assert_eq!(Xd39StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_39_sm_reset() {
        let mut sm = Xd39StateMachine::new();
        sm.transition(Xd39State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd39State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_39_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd39EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd39Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_39_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd39EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd39Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd39Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_39_bus_unsubscribe() {
        let mut bus = Xd39EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_39_event_kind_and_payload() {
        let e = Xd39Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd39Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_39_bus_clear_history() {
        let mut bus = Xd39EventBus::new();
        bus.publish(Xd39Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_39_sm_step_counter_increments() {
        let mut sm = Xd39StateMachine::new();
        sm.transition(Xd39State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd39State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #37 --

    #[test]
    fn xf37_trie_insert_search() {
        let mut t = Xf37Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf37_trie_starts_with() {
        let mut t = Xf37Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf37_trie_remove() {
        let mut t = Xf37Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf37_trie_word_count() {
        let mut t = Xf37Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf37_trie_longest_prefix() {
        let mut t = Xf37Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf37_trie_all_words() {
        let mut t = Xf37Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf37_trie_autocomplete() {
        let mut t = Xf37Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf37_trie_empty_search() {
        let t = Xf37Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf37_bloom_add_contains() {
        let mut bf = Xf37BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf37_bloom_probably_absent() {
        let bf = Xf37BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf37_bloom_false_positive_rate() {
        let mut bf = Xf37BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf37_bloom_clear() {
        let mut bf = Xf37BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf37_bloom_union() {
        let mut a = Xf37BloomFilter::xf_new(512, 2);
        let mut b = Xf37BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf37_bloom_intersection_estimate() {
        let mut a = Xf37BloomFilter::xf_new(512, 2);
        let mut b = Xf37BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf37_bloom_union_size_mismatch() {
        let a = Xf37BloomFilter::xf_new(256, 2);
        let b = Xf37BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh31_skip_insert_contains() {
        let mut sl = super::Xh31SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh31_skip_remove() {
        let mut sl = super::Xh31SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh31_skip_len() {
        let mut sl = super::Xh31SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh31_skip_range_query() {
        let mut sl = super::Xh31SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh31_skip_floor_ceiling() {
        let mut sl = super::Xh31SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh31_skip_rank() {
        let mut sl = super::Xh31SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh31_skip_empty() {
        let sl = super::Xh31SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh31_skip_duplicates() {
        let mut sl = super::Xh31SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh31_bitset_set_test() {
        let mut bs = super::Xh31BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh31_bitset_clear_count() {
        let mut bs = super::Xh31BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh31_bitset_and_or_xor() {
        let mut a = super::Xh31BitSet::xh_new(128);
        let mut b = super::Xh31BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh31_bitset_iter_ones() {
        let mut bs = super::Xh31BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh31_bitset_first_last() {
        let mut bs = super::Xh31BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh31_bitset_empty() {
        let bs = super::Xh31BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi31_deque_push_pop_back() {
        let mut dq = super::Xi31Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi31_deque_push_pop_front() {
        let mut dq = super::Xi31Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi31_deque_mixed_ops() {
        let mut dq = super::Xi31Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi31_deque_get_and_split() {
        let mut dq = super::Xi31Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi31_deque_rotate_left() {
        let mut dq = super::Xi31Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi31_deque_rotate_right() {
        let mut dq = super::Xi31Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi31_deque_grow() {
        let mut dq = super::Xi31Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi31_deque_empty() {
        let dq = super::Xi31Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi31_interval_tree_insert_query() {
        let mut tree = super::Xi31IntervalTree::xi_new();
        tree.xi_insert(super::Xi31Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi31Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi31Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi31_interval_tree_overlap() {
        let mut tree = super::Xi31IntervalTree::xi_new();
        tree.xi_insert(super::Xi31Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi31Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi31Interval::xi_new(12, 20));
        let q = super::Xi31Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi31_interval_tree_remove() {
        let mut tree = super::Xi31IntervalTree::xi_new();
        tree.xi_insert(super::Xi31Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi31Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi31_interval_tree_gaps() {
        let mut tree = super::Xi31IntervalTree::xi_new();
        tree.xi_insert(super::Xi31Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi31Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi31Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi31Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi31Interval::xi_new(8, 10));
    }

    #[test]
    fn xi31_interval_tree_merge() {
        let mut tree = super::Xi31IntervalTree::xi_new();
        tree.xi_insert(super::Xi31Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi31Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi31Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi31Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi31Interval::xi_new(10, 15));
    }

    #[test]
    fn xi31_interval_tree_all() {
        let mut tree = super::Xi31IntervalTree::xi_new();
        tree.xi_insert(super::Xi31Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi31Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi31_interval_tree_empty() {
        let tree = super::Xi31IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi31_interval_tree_contains_point() {
        let iv = super::Xi31Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 31) ---

    #[test]
    fn xj_31_uf_make_and_find() {
        let mut uf = super::Xj31UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_31_uf_union_connected() {
        let mut uf = super::Xj31UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_31_uf_component_count() {
        let mut uf = super::Xj31UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_31_uf_component_size() {
        let mut uf = super::Xj31UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_31_uf_largest_component() {
        let mut uf = super::Xj31UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_31_uf_many_elements() {
        let mut uf = super::Xj31UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_31_uf_separate_components() {
        let mut uf = super::Xj31UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_31_uf_path_compression() {
        let mut uf = super::Xj31UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_31_bt_insert_get() {
        let mut bt = super::Xj31BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_31_bt_contains_len() {
        let mut bt = super::Xj31BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_31_bt_replace() {
        let mut bt = super::Xj31BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_31_bt_remove() {
        let mut bt = super::Xj31BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_31_bt_keys_values() {
        let mut bt = super::Xj31BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_31_bt_range() {
        let mut bt = super::Xj31BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_31_bt_min_max() {
        let mut bt = super::Xj31BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_31_bt_many_inserts() {
        let mut bt = super::Xj31BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_31 segment tree tests ---

    #[test]
    fn xk_31_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk31SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_31_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk31SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_31_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk31SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_31_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk31SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_31_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk31SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_31_st_single_element() {
        let data = vec![42];
        let st = super::Xk31SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_31_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk31SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_31_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk31SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_31 disjoint intervals tests ---

    #[test]
    fn xk_31_di_add_and_count() {
        let mut di = super::Xk31DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_31_di_merge_overlap() {
        let mut di = super::Xk31DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_31_di_contains() {
        let mut di = super::Xk31DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_31_di_remove() {
        let mut di = super::Xk31DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_31_di_covered_length() {
        let mut di = super::Xk31DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_31_di_gaps() {
        let mut di = super::Xk31DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_31_di_merge_adjacent() {
        let mut di = super::Xk31DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_31_di_empty() {
        let di = super::Xk31DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_31_rope_new_empty() {
        let rope = super::Xl31Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_31_rope_from_str() {
        let rope = super::Xl31Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_31_rope_insert_at() {
        let mut rope = super::Xl31Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_31_rope_delete_range() {
        let mut rope = super::Xl31Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_31_rope_char_at() {
        let rope = super::Xl31Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_31_rope_split_concat() {
        let rope = super::Xl31Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_31_rope_line_count() {
        let rope = super::Xl31Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_31_rope_line_at() {
        let rope = super::Xl31Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_31_sa_build_and_search() {
        let sa = super::Xl31SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_31_sa_count() {
        let sa = super::Xl31SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_31_sa_longest_repeated() {
        let sa = super::Xl31SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_31_sa_all_positions() {
        let sa = super::Xl31SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_31_sa_len() {
        let sa = super::Xl31SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_31_sa_empty() {
        let sa = super::Xl31SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_31_rope_slice() {
        let rope = super::Xl31Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_31_sa_search_start() {
        let sa = super::Xl31SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_31_sparse_set_get() {
        let mut m = super::Xm31MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_31_sparse_row_col() {
        let mut m = super::Xm31MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_31_sparse_transpose() {
        let mut m = super::Xm31MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_31_sparse_multiply_vec() {
        let mut m = super::Xm31MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_31_sparse_nnz_density() {
        let mut m = super::Xm31MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_31_sparse_clear() {
        let mut m = super::Xm31MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_31_sparse_overwrite_zero() {
        let mut m = super::Xm31MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_31_tokenizer_basic() {
        let t = super::Xm31Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_31_tokenizer_count() {
        let t = super::Xm31Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_31_tokenizer_unique() {
        let t = super::Xm31Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_31_tokenizer_frequency() {
        let t = super::Xm31Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_31_tokenizer_delimiter() {
        let t = super::Xm31Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_31_tokenizer_whitespace() {
        let t = super::Xm31Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_31_tokenizer_empty() {
        let t = super::Xm31Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 31 ----

    #[test]
    fn xn_31_fenwick_prefix_sum() {
        let mut ft = super::Xn31Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_31_fenwick_range_sum() {
        let mut ft = super::Xn31Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_31_fenwick_point_query() {
        let mut ft = super::Xn31Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_31_fenwick_len() {
        let ft = super::Xn31Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_31_fenwick_multiple_updates() {
        let mut ft = super::Xn31Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_31_fenwick_single_element() {
        let mut ft = super::Xn31Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_31_fenwick_find_kth() {
        let mut ft = super::Xn31Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_31_fenwick_negative_delta() {
        let mut ft = super::Xn31Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 31 ----

    #[test]
    fn xn_31_avl_insert_get() {
        let mut m = super::Xn31AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_31_avl_remove() {
        let mut m = super::Xn31AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_31_avl_in_order() {
        let mut m = super::Xn31AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_31_avl_min_max() {
        let mut m = super::Xn31AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_31_avl_floor_ceiling() {
        let mut m = super::Xn31AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_31_avl_height_balanced() {
        let mut m = super::Xn31AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_31_avl_overwrite() {
        let mut m = super::Xn31AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_31_avl_empty() {
        let m: super::Xn31AVL<i32, i32> = super::Xn31AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo31RedBlack tests ---

    #[test]
    fn xo_31_rb_insert_and_get() {
        let mut tree = super::Xo31RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_31_rb_len_and_empty() {
        let mut tree = super::Xo31RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_31_rb_min_max() {
        let mut tree = super::Xo31RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_31_rb_contains() {
        let mut tree = super::Xo31RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_31_rb_remove() {
        let mut tree = super::Xo31RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_31_rb_in_order() {
        let mut tree = super::Xo31RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_31_rb_black_height() {
        let mut tree = super::Xo31RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_31_rb_overwrite() {
        let mut tree = super::Xo31RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo31ConsistentHash tests ---

    #[test]
    fn xo_31_ch_add_and_count() {
        let mut ring = super::Xo31ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_31_ch_remove_node() {
        let mut ring = super::Xo31ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_31_ch_get_node() {
        let mut ring = super::Xo31ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_31_ch_empty_ring() {
        let ring = super::Xo31ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_31_ch_distribution() {
        let mut ring = super::Xo31ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_31_ch_rebalance() {
        let mut ring = super::Xo31ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_31_ch_virtual_nodes() {
        let mut ring = super::Xo31ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_31_ch_consistent_lookup() {
        let mut ring = super::Xo31ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_31_splay_insert_get() {
        let mut t = super::Xp31SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_31_splay_remove() {
        let mut t = super::Xp31SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_31_splay_count_increases() {
        let mut t = super::Xp31SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_31_splay_depth() {
        let mut t = super::Xp31SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_31_splay_len_empty() {
        let t = super::Xp31SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_31_splay_min_max() {
        let mut t = super::Xp31SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_31_splay_overwrite() {
        let mut t = super::Xp31SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_31_splay_remove_missing() {
        let mut t = super::Xp31SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_31 treap tests ----
    #[test]
    fn xq_31_treap_empty() {
        let t = super::Xq31Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_31_treap_insert_get() {
        let mut t = super::Xq31Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_31_treap_overwrite() {
        let mut t = super::Xq31Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_31_treap_remove() {
        let mut t = super::Xq31Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_31_treap_min_max() {
        let mut t = super::Xq31Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_31_treap_rank() {
        let mut t = super::Xq31Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_31_treap_kth() {
        let mut t = super::Xq31Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_31_treap_in_order() {
        let mut t = super::Xq31Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_31 VEB tree tests ----
    #[test]
    fn xq_31_veb_empty() {
        let v = super::Xq31VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_31_veb_insert_contains() {
        let mut v = super::Xq31VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_31_veb_min_max() {
        let mut v = super::Xq31VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_31_veb_delete() {
        let mut v = super::Xq31VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_31_veb_successor() {
        let mut v = super::Xq31VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_31_veb_predecessor() {
        let mut v = super::Xq31VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_31_veb_count() {
        let mut v = super::Xq31VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_31_veb_duplicate_insert() {
        let mut v = super::Xq31VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_31_kdtree_empty() {
        let tree = super::Xr31KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_31_kdtree_insert_one() {
        let mut tree = super::Xr31KDTree::xr_new();
        tree.xr_insert(super::Xr31KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_31_kdtree_insert_multiple() {
        let mut tree = super::Xr31KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr31KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_31_kdtree_nearest_neighbor() {
        let mut tree = super::Xr31KDTree::xr_new();
        tree.xr_insert(super::Xr31KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr31KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr31KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_31_kdtree_nn_empty() {
        let tree = super::Xr31KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr31KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_31_kdtree_range_search() {
        let mut tree = super::Xr31KDTree::xr_new();
        tree.xr_insert(super::Xr31KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr31KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr31KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_31_kdtree_range_empty() {
        let mut tree = super::Xr31KDTree::xr_new();
        tree.xr_insert(super::Xr31KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_31_kdtree_all_points() {
        let mut tree = super::Xr31KDTree::xr_new();
        tree.xr_insert(super::Xr31KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr31KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_31_kdtree_depth() {
        let mut tree = super::Xr31KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr31KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_31_kdtree_bounding_box() {
        let mut tree = super::Xr31KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr31KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr31KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_31_persistent_array_new() {
        let arr = super::Xs31PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_31_persistent_array_push() {
        let mut arr = super::Xs31PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_31_persistent_array_set() {
        let mut arr = super::Xs31PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_31_persistent_array_diff() {
        let mut arr = super::Xs31PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_31_persistent_array_rollback() {
        let mut arr = super::Xs31PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_31_persistent_array_history() {
        let mut arr = super::Xs31PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_31_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs31PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_31_persistent_array_from_vec() {
        let arr = super::Xs31PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_31_concurrent_queue_new() {
        let q = super::Xs31ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_31_concurrent_queue_push_pop() {
        let mut q = super::Xs31ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_31_concurrent_queue_full() {
        let mut q = super::Xs31ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_31_concurrent_queue_drain() {
        let mut q = super::Xs31ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_31_concurrent_queue_try_pop() {
        let mut q = super::Xs31ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_31_concurrent_queue_clear() {
        let mut q = super::Xs31ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_31_range_map_new() {
        let rm = super::Xs31RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_31_range_map_insert_get() {
        let mut rm = super::Xs31RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_31_range_map_overlap() {
        let mut rm = super::Xs31RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_31_range_map_remove() {
        let mut rm = super::Xs31RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_31_range_map_gaps() {
        let mut rm = super::Xs31RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_31_range_map_coverage() {
        let mut rm = super::Xs31RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_31_range_map_contains() {
        let mut rm = super::Xs31RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_31_range_map_clear() {
        let mut rm = super::Xs31RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_31_circular_buffer_new() {
        let buf = super::Xs31CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_31_circular_buffer_push_pop() {
        let mut buf = super::Xs31CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_31_circular_buffer_overwrite() {
        let mut buf = super::Xs31CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_31_circular_buffer_peek() {
        let mut buf = super::Xs31CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_31_circular_buffer_is_full() {
        let mut buf = super::Xs31CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_31_circular_buffer_iter() {
        let mut buf = super::Xs31CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_31_circular_buffer_clear() {
        let mut buf = super::Xs31CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_31_circular_buffer_to_vec() {
        let mut buf = super::Xs31CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }

}
