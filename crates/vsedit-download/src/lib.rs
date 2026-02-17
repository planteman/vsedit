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
}
