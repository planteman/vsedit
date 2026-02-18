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

}
