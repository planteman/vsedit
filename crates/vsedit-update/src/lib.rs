//! Update mechanism.

use std::fmt;
use std::error;

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateChannel {
    Stable,
    Insider,
    Exploration,
}

impl UpdateChannel {
    pub fn all() -> Vec<UpdateChannel> {
        vec![
            UpdateChannel::Stable,
            UpdateChannel::Insider,
            UpdateChannel::Exploration,
        ]
    }

    pub fn is_preview(&self) -> bool {
        matches!(self, UpdateChannel::Insider | UpdateChannel::Exploration)
    }
}

impl fmt::Display for UpdateChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateChannel::Stable => write!(f, "Stable"),
            UpdateChannel::Insider => write!(f, "Insider"),
            UpdateChannel::Exploration => write!(f, "Exploration"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionParts {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl VersionParts {
    pub fn distance_to(&self, other: &Self) -> (i64, i64, i64) {
        (
            i64::from(other.major) - i64::from(self.major),
            i64::from(other.minor) - i64::from(self.minor),
            i64::from(other.patch) - i64::from(self.patch),
        )
    }

    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major
    }

    pub fn parse(version: &str) -> Option<Self> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }

    pub fn is_older_than(&self, other: &Self) -> bool {
        if self.major != other.major {
            return self.major < other.major;
        }
        if self.minor != other.minor {
            return self.minor < other.minor;
        }
        self.patch < other.patch
    }

    /// Returns the version as a single comparable integer (major * 1_000_000 + minor * 1_000 + patch).
    pub fn as_numeric(&self) -> u64 {
        u64::from(self.major) * 1_000_000 + u64::from(self.minor) * 1_000 + u64::from(self.patch)
    }

    /// Bump the patch version, returning a new `VersionParts`.
    pub fn bump_patch(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor,
            patch: self.patch + 1,
        }
    }

    /// Bump the minor version (resets patch to 0).
    pub fn bump_minor(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor + 1,
            patch: 0,
        }
    }

    /// Bump the major version (resets minor and patch to 0).
    pub fn bump_major(&self) -> Self {
        Self {
            major: self.major + 1,
            minor: 0,
            patch: 0,
        }
    }

    /// Returns `true` if this is a pre-release version (0.x.y).
    pub fn is_prerelease(&self) -> bool {
        self.major == 0
    }
}

impl fmt::Display for VersionParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Ord for VersionParts {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_numeric().cmp(&other.as_numeric())
    }
}

impl PartialOrd for VersionParts {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    CheckingForUpdates,
    UpdateAvailable,
    Downloading,
    Ready,
    Error(String),
}

impl UpdateState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, UpdateState::Ready | UpdateState::Error(_))
    }

    pub fn is_error(&self) -> bool {
        matches!(self, UpdateState::Error(_))
    }
}

impl fmt::Display for UpdateState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateState::Idle => write!(f, "Idle"),
            UpdateState::CheckingForUpdates => write!(f, "Checking for updates"),
            UpdateState::UpdateAvailable => write!(f, "Update available"),
            UpdateState::Downloading => write!(f, "Downloading"),
            UpdateState::Ready => write!(f, "Ready to install"),
            UpdateState::Error(e) => write!(f, "Error: {e}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateInfo {
    pub version: String,
    pub product_version: String,
    pub url: Option<String>,
    pub release_notes: Option<String>,
}

impl fmt::Display for UpdateInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.version)?;
        if let Some(url) = &self.url {
            write!(f, " ({url})")?;
        }
        Ok(())
    }
}

/// Builder for constructing [`UpdateInfo`] with validation.
#[derive(Debug, Default)]
pub struct UpdateInfoBuilder {
    version: Option<String>,
    product_version: Option<String>,
    url: Option<String>,
    release_notes: Option<String>,
}

impl UpdateInfoBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn product_version(mut self, pv: impl Into<String>) -> Self {
        self.product_version = Some(pv.into());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn release_notes(mut self, notes: impl Into<String>) -> Self {
        self.release_notes = Some(notes.into());
        self
    }

    /// Build the `UpdateInfo`, returning an error if required fields are missing
    /// or the version string is malformed.
    pub fn build(self) -> Result<UpdateInfo, UpdateError> {
        let version = self.version.ok_or(UpdateError::InvalidVersion(
            "version is required".into(),
        ))?;
        if VersionParts::parse(&version).is_none() {
            return Err(UpdateError::InvalidVersion(format!(
                "cannot parse version: {version}"
            )));
        }
        let product_version = self.product_version.unwrap_or_else(|| version.clone());
        Ok(UpdateInfo {
            version,
            product_version,
            url: self.url,
            release_notes: self.release_notes,
        })
    }
}

/// Errors that can occur during update operations.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateError {
    /// The version string could not be parsed.
    InvalidVersion(String),
    /// A state transition was attempted that is not allowed.
    InvalidStateTransition { from: UpdateState, to: UpdateState },
    /// The download URL is missing.
    MissingDownloadUrl,
    /// A network or I/O error description.
    NetworkError(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateError::InvalidVersion(msg) => write!(f, "invalid version: {msg}"),
            UpdateError::InvalidStateTransition { from, to } => {
                write!(f, "invalid state transition from {from} to {to}")
            }
            UpdateError::MissingDownloadUrl => write!(f, "download URL is missing"),
            UpdateError::NetworkError(msg) => write!(f, "network error: {msg}"),
        }
    }
}

impl error::Error for UpdateError {}

pub struct UpdateService {
    state: UpdateState,
    current_version: String,
    available_update: Option<UpdateInfo>,
    channel: UpdateChannel,
    progress: Option<f64>,
    update_history: Vec<UpdateInfo>,
}

impl UpdateService {
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            state: UpdateState::Idle,
            current_version: current_version.into(),
            available_update: None,
            channel: UpdateChannel::Stable,
            progress: None,
            update_history: Vec::new(),
        }
    }

    pub fn get_channel(&self) -> &UpdateChannel {
        &self.channel
    }

    pub fn get_progress(&self) -> Option<f64> {
        self.progress
    }

    pub fn download_progress(&mut self, progress: f64) {
        self.state = UpdateState::Downloading;
        self.progress = Some(progress);
    }

    pub fn apply_update(&mut self) {
        if self.state == UpdateState::Ready {
            self.state = UpdateState::Idle;
            self.progress = None;
        }
    }

    pub fn dismiss_update(&mut self) {
        self.state = UpdateState::Idle;
        self.available_update = None;
        self.progress = None;
    }

    pub fn check_for_update(&mut self, latest: UpdateInfo) -> bool {
        self.state = UpdateState::CheckingForUpdates;
        if self.needs_update(&latest.version) {
            self.state = UpdateState::UpdateAvailable;
            self.available_update = Some(latest);
            true
        } else {
            self.state = UpdateState::Idle;
            false
        }
    }

    pub fn get_state(&self) -> &UpdateState {
        &self.state
    }

    pub fn set_state(&mut self, state: UpdateState) {
        self.state = state;
    }

    pub fn get_available_update(&self) -> Option<&UpdateInfo> {
        self.available_update.as_ref()
    }

    pub fn needs_update(&self, latest_version: &str) -> bool {
        let current = VersionParts::parse(&self.current_version);
        let latest = VersionParts::parse(latest_version);
        match (current, latest) {
            (Some(c), Some(l)) => c.is_older_than(&l),
            _ => latest_version != self.current_version,
        }
    }

    pub fn set_channel(&mut self, channel: UpdateChannel) {
        self.channel = channel;
    }

    pub fn current_version(&self) -> &str {
        &self.current_version
    }

    pub fn current_version_parts(&self) -> Option<VersionParts> {
        VersionParts::parse(&self.current_version)
    }

    /// Attempt to start a download, returning an error if no update is available
    /// or the download URL is missing.
    pub fn start_download(&mut self) -> Result<&str, UpdateError> {
        if self.state != UpdateState::UpdateAvailable {
            return Err(UpdateError::InvalidStateTransition {
                from: self.state.clone(),
                to: UpdateState::Downloading,
            });
        }
        let url = self
            .available_update
            .as_ref()
            .and_then(|u| u.url.as_deref())
            .ok_or(UpdateError::MissingDownloadUrl)?;
        self.state = UpdateState::Downloading;
        self.progress = Some(0.0);
        Ok(url)
    }

    /// Mark the download as complete and transition to `Ready`.
    pub fn finish_download(&mut self) -> Result<(), UpdateError> {
        if self.state != UpdateState::Downloading {
            return Err(UpdateError::InvalidStateTransition {
                from: self.state.clone(),
                to: UpdateState::Ready,
            });
        }
        self.progress = Some(1.0);
        self.state = UpdateState::Ready;
        Ok(())
    }

    /// Returns `true` when the service has an update ready to install.
    pub fn is_ready_to_install(&self) -> bool {
        self.state == UpdateState::Ready
    }

    /// Returns `true` when the service is actively downloading.
    pub fn is_downloading(&self) -> bool {
        self.state == UpdateState::Downloading
    }

    /// Reset the service to idle after an error, preserving current version.
    pub fn reset_from_error(&mut self) {
        if matches!(self.state, UpdateState::Error(_)) {
            self.state = UpdateState::Idle;
            self.available_update = None;
            self.progress = None;
        }
    }

    /// Report an error, transitioning to the error state.
    pub fn report_error(&mut self, message: impl Into<String>) {
        self.state = UpdateState::Error(message.into());
        self.progress = None;
    }

    pub fn reset(&mut self) {
        self.state = UpdateState::Idle;
        self.available_update = None;
        self.progress = None;
    }

    pub fn history(&self) -> &[UpdateInfo] {
        &self.update_history
    }

    pub fn push_history(&mut self, info: UpdateInfo) {
        self.update_history.push(info);
    }
}

impl IntoIterator for UpdateService {
    type Item = UpdateInfo;
    type IntoIter = std::vec::IntoIter<UpdateInfo>;

    fn into_iter(self) -> Self::IntoIter {
        self.update_history.into_iter()
    }
}

impl fmt::Display for UpdateService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UpdateService(v{}, channel={}, state={})",
            self.current_version, self.channel, self.state
        )
    }
}

/// Accumulated statistics for update operations.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl UpdateStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &UpdateStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for UpdateStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UpdateStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UpdateStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for update.
#[derive(Debug, Clone)]
pub struct UpdateValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl UpdateValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for UpdateValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// update_check_interval — configurable update frequency
// ---------------------------------------------------------------------------

/// Predefined update check intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCheckInterval {
    /// Check every hour.
    Hourly,
    /// Check every 4 hours.
    FourHours,
    /// Check every 12 hours.
    TwelveHours,
    /// Check once a day.
    Daily,
    /// Check once a week.
    Weekly,
    /// Never auto-check.
    Never,
    /// Custom interval in seconds.
    Custom(u64),
}

impl UpdateCheckInterval {
    /// Return the interval as seconds.
    pub fn as_secs(&self) -> Option<u64> {
        match self {
            Self::Hourly => Some(3_600),
            Self::FourHours => Some(14_400),
            Self::TwelveHours => Some(43_200),
            Self::Daily => Some(86_400),
            Self::Weekly => Some(604_800),
            Self::Never => None,
            Self::Custom(s) => Some(*s),
        }
    }

    /// Return true if automatic checking is enabled.
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Never)
    }

    /// Parse from a string label (case-insensitive).
    pub fn from_label(label: &str) -> Self {
        match label.to_lowercase().as_str() {
            "hourly" => Self::Hourly,
            "4hours" | "four_hours" | "fourhours" => Self::FourHours,
            "12hours" | "twelve_hours" | "twelvehours" => Self::TwelveHours,
            "daily" => Self::Daily,
            "weekly" => Self::Weekly,
            "never" | "off" | "disabled" => Self::Never,
            _ => label.parse::<u64>().map(Self::Custom).unwrap_or(Self::Daily),
        }
    }
}

impl fmt::Display for UpdateCheckInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hourly => write!(f, "hourly"),
            Self::FourHours => write!(f, "every 4 hours"),
            Self::TwelveHours => write!(f, "every 12 hours"),
            Self::Daily => write!(f, "daily"),
            Self::Weekly => write!(f, "weekly"),
            Self::Never => write!(f, "never"),
            Self::Custom(s) => write!(f, "every {s}s"),
        }
    }
}

/// Determine whether enough time has passed since the last check.
pub fn update_check_interval(
    interval: UpdateCheckInterval,
    last_check_timestamp: u64,
    current_timestamp: u64,
) -> bool {
    match interval.as_secs() {
        None => false,
        Some(secs) => current_timestamp.saturating_sub(last_check_timestamp) >= secs,
    }
}

// ---------------------------------------------------------------------------
// UpdateDiffSummary – summarize changes between versions
// ---------------------------------------------------------------------------

/// Summarizes the differences between two versions.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateDiffSummary {
    pub from_version: String,
    pub to_version: String,
    pub features_added: Vec<String>,
    pub bugs_fixed: Vec<String>,
    pub breaking_changes: Vec<String>,
}

impl UpdateDiffSummary {
    /// Create a new diff summary between two versions.
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from_version: from.into(),
            to_version: to.into(),
            features_added: Vec::new(),
            bugs_fixed: Vec::new(),
            breaking_changes: Vec::new(),
        }
    }

    /// Add a new feature to the summary.
    pub fn add_feature(&mut self, feature: impl Into<String>) -> &mut Self {
        self.features_added.push(feature.into());
        self
    }

    /// Add a bug fix to the summary.
    pub fn add_fix(&mut self, fix: impl Into<String>) -> &mut Self {
        self.bugs_fixed.push(fix.into());
        self
    }

    /// Add a breaking change to the summary.
    pub fn add_breaking(&mut self, change: impl Into<String>) -> &mut Self {
        self.breaking_changes.push(change.into());
        self
    }

    /// Total number of changes.
    pub fn total_changes(&self) -> usize {
        self.features_added.len() + self.bugs_fixed.len() + self.breaking_changes.len()
    }

    /// Whether there are any breaking changes.
    pub fn has_breaking_changes(&self) -> bool {
        !self.breaking_changes.is_empty()
    }

    /// Whether the update is a major version bump.
    pub fn is_major_update(&self) -> bool {
        let from = VersionParts::parse(&self.from_version);
        let to = VersionParts::parse(&self.to_version);
        match (from, to) {
            (Some(f), Some(t)) => t.major > f.major,
            _ => false,
        }
    }
}

impl fmt::Display for UpdateDiffSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} → {}: {} features, {} fixes, {} breaking",
            self.from_version,
            self.to_version,
            self.features_added.len(),
            self.bugs_fixed.len(),
            self.breaking_changes.len()
        )
    }
}

// ---------------------------------------------------------------------------
// UpdateRollbackPlan – plan rollback to a previous version
// ---------------------------------------------------------------------------

/// Describes a plan for rolling back to a previous version.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateRollbackPlan {
    pub current_version: String,
    pub target_version: String,
    pub steps: Vec<String>,
    pub requires_restart: bool,
    pub data_migration_needed: bool,
}

impl UpdateRollbackPlan {
    /// Create a rollback plan from current to target version.
    pub fn new(current: impl Into<String>, target: impl Into<String>) -> Self {
        let current = current.into();
        let target = target.into();
        let cur_parts = VersionParts::parse(&current);
        let tgt_parts = VersionParts::parse(&target);

        let requires_restart = true; // rollbacks always require restart
        let data_migration_needed = match (&cur_parts, &tgt_parts) {
            (Some(c), Some(t)) => c.major != t.major,
            _ => false,
        };

        let mut steps = vec![
            format!("Backup current configuration (v{})", current),
            format!("Download version {}", target),
        ];
        if data_migration_needed {
            steps.push("Run data migration (major version change)".into());
        }
        steps.push(format!("Install version {}", target));
        steps.push("Restart application".into());

        Self {
            current_version: current,
            target_version: target,
            steps,
            requires_restart,
            data_migration_needed,
        }
    }

    /// Number of steps in the plan.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Whether the rollback crosses a major version boundary.
    pub fn is_major_rollback(&self) -> bool {
        self.data_migration_needed
    }

    /// Validate that the target version is actually older than current.
    pub fn is_valid(&self) -> bool {
        let cur = VersionParts::parse(&self.current_version);
        let tgt = VersionParts::parse(&self.target_version);
        match (cur, tgt) {
            (Some(c), Some(t)) => t.is_older_than(&c),
            _ => false,
        }
    }
}

impl fmt::Display for UpdateRollbackPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rollback {} → {} ({} steps)",
            self.current_version,
            self.target_version,
            self.steps.len()
        )
    }
}

// ---------------------------------------------------------------------------
// UpdatePrerequisites – check prerequisites before update
// ---------------------------------------------------------------------------

/// A single prerequisite check.
#[derive(Debug, Clone, PartialEq)]
pub struct Prerequisite {
    pub name: String,
    pub description: String,
    pub satisfied: bool,
}

/// Checks prerequisites before allowing an update to proceed.
#[derive(Debug, Clone)]
pub struct UpdatePrerequisites {
    checks: Vec<Prerequisite>,
}

impl UpdatePrerequisites {
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
        }
    }

    /// Add a prerequisite check.
    pub fn add_check(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        satisfied: bool,
    ) -> &mut Self {
        self.checks.push(Prerequisite {
            name: name.into(),
            description: description.into(),
            satisfied,
        });
        self
    }

    /// Returns true if all prerequisites are satisfied.
    pub fn all_satisfied(&self) -> bool {
        self.checks.iter().all(|c| c.satisfied)
    }

    /// Return the list of unsatisfied prerequisites.
    pub fn unsatisfied(&self) -> Vec<&Prerequisite> {
        self.checks.iter().filter(|c| !c.satisfied).collect()
    }

    /// Total number of prerequisites.
    pub fn count(&self) -> usize {
        self.checks.len()
    }

    /// Number of satisfied prerequisites.
    pub fn satisfied_count(&self) -> usize {
        self.checks.iter().filter(|c| c.satisfied).count()
    }

    /// Check a minimum version prerequisite.
    pub fn require_min_version(&mut self, current: &str, minimum: &str) -> &mut Self {
        let cur = VersionParts::parse(current);
        let min = VersionParts::parse(minimum);
        let satisfied = match (cur, min) {
            (Some(c), Some(m)) => !c.is_older_than(&m),
            _ => false,
        };
        self.add_check(
            "min_version",
            format!("Requires at least version {minimum}"),
            satisfied,
        )
    }

    /// Check a disk space prerequisite (in bytes).
    pub fn require_disk_space(&mut self, available_bytes: u64, required_bytes: u64) -> &mut Self {
        self.add_check(
            "disk_space",
            format!("Requires {required_bytes} bytes free"),
            available_bytes >= required_bytes,
        )
    }
}

impl Default for UpdatePrerequisites {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// VersionParts – comparison and range checking extensions
// ---------------------------------------------------------------------------

impl VersionParts {
    /// Check if this version falls within the given range [min, max] inclusive.
    pub fn in_range(&self, min: &VersionParts, max: &VersionParts) -> bool {
        !self.is_older_than(min) && (self == max || self.is_older_than(max))
    }

    /// Return the number of version increments between two versions
    /// as a simple tuple (major_diff, minor_diff, patch_diff).
    pub fn increments_from(&self, other: &VersionParts) -> (u32, u32, u32) {
        (
            self.major.abs_diff(other.major),
            self.minor.abs_diff(other.minor),
            self.patch.abs_diff(other.patch),
        )
    }

    /// Return the "severity" of the version difference: Major, Minor, or Patch.
    pub fn diff_severity(&self, other: &VersionParts) -> &'static str {
        if self.major != other.major {
            "major"
        } else if self.minor != other.minor {
            "minor"
        } else if self.patch != other.patch {
            "patch"
        } else {
            "none"
        }
    }

    /// Construct from individual components.
    pub fn from_parts(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Return true if this version is newer than the other.
    pub fn is_newer_than(&self, other: &Self) -> bool {
        other.is_older_than(self)
    }

    /// Check if this version satisfies a simple constraint string.
    ///
    /// Supported operators: `=`, `>`, `<`, `>=`, `<=`, `^` (compatible), `~` (patch-level).
    /// - `^1.2.3` matches `>=1.2.3` and `<2.0.0` (same major).
    /// - `~1.2.3` matches `>=1.2.3` and `<1.3.0` (same major.minor).
    pub fn satisfies(&self, constraint: &str) -> bool {
        let constraint = constraint.trim();
        if constraint.is_empty() {
            return false;
        }

        if let Some(rest) = constraint.strip_prefix(">=") {
            return VersionParts::parse(rest.trim())
                .map_or(false, |target| !self.is_older_than(&target));
        }
        if let Some(rest) = constraint.strip_prefix("<=") {
            return VersionParts::parse(rest.trim())
                .map_or(false, |target| !self.is_newer_than(&target));
        }
        if let Some(rest) = constraint.strip_prefix('>') {
            return VersionParts::parse(rest.trim())
                .map_or(false, |target| self.is_newer_than(&target));
        }
        if let Some(rest) = constraint.strip_prefix('<') {
            return VersionParts::parse(rest.trim())
                .map_or(false, |target| self.is_older_than(&target));
        }
        if let Some(rest) = constraint.strip_prefix('^') {
            return VersionParts::parse(rest.trim()).map_or(false, |target| {
                !self.is_older_than(&target) && self.major == target.major
            });
        }
        if let Some(rest) = constraint.strip_prefix('~') {
            return VersionParts::parse(rest.trim()).map_or(false, |target| {
                !self.is_older_than(&target)
                    && self.major == target.major
                    && self.minor == target.minor
            });
        }
        if let Some(rest) = constraint.strip_prefix('=') {
            return VersionParts::parse(rest.trim()).map_or(false, |target| self == &target);
        }
        // bare version treated as exact match
        VersionParts::parse(constraint).map_or(false, |target| self == &target)
    }
}

// ---------------------------------------------------------------------------
// VersionRange – inclusive version range with filtering
// ---------------------------------------------------------------------------

/// An inclusive version range `[min, max]` for filtering and containment checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRange {
    pub min: VersionParts,
    pub max: VersionParts,
}

impl VersionRange {
    /// Create a new inclusive version range.  
    /// Returns `None` if `min > max`.
    pub fn new(min: VersionParts, max: VersionParts) -> Option<Self> {
        if min.is_newer_than(&max) {
            return None;
        }
        Some(Self { min, max })
    }

    /// Returns `true` if `version` falls within `[min, max]`.
    pub fn contains(&self, version: &VersionParts) -> bool {
        !version.is_older_than(&self.min) && !version.is_newer_than(&self.max)
    }

    /// Filter a slice of versions, returning only those within the range.
    pub fn filter<'a>(&self, versions: &'a [VersionParts]) -> Vec<&'a VersionParts> {
        versions.iter().filter(|v| self.contains(v)).collect()
    }

    /// Return the span of major versions covered by this range.
    pub fn major_span(&self) -> u32 {
        self.max.major.saturating_sub(self.min.major) + 1
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.min, self.max)
    }
}

// ---------------------------------------------------------------------------
// UpdateChannel – parsing and priority
// ---------------------------------------------------------------------------

impl UpdateChannel {
    /// Parse a channel name (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "stable" => Some(Self::Stable),
            "insider" | "insiders" => Some(Self::Insider),
            "exploration" | "explore" => Some(Self::Exploration),
            _ => None,
        }
    }

    /// Return a numeric stability priority (lower = more stable).
    pub fn stability_priority(&self) -> u8 {
        match self {
            Self::Stable => 0,
            Self::Insider => 1,
            Self::Exploration => 2,
        }
    }

    /// Returns `true` if `self` is at least as stable as `other`.
    pub fn is_at_least_as_stable_as(&self, other: &Self) -> bool {
        self.stability_priority() <= other.stability_priority()
    }
}

// ---------------------------------------------------------------------------
// UpdateState – valid transitions
// ---------------------------------------------------------------------------

impl UpdateState {
    /// The set of states that are valid successors of this state.
    pub fn valid_transitions(&self) -> Vec<UpdateState> {
        match self {
            UpdateState::Idle => vec![UpdateState::CheckingForUpdates],
            UpdateState::CheckingForUpdates => vec![
                UpdateState::UpdateAvailable,
                UpdateState::Idle,
                UpdateState::Error(String::new()),
            ],
            UpdateState::UpdateAvailable => vec![
                UpdateState::Downloading,
                UpdateState::Idle,
                UpdateState::Error(String::new()),
            ],
            UpdateState::Downloading => vec![
                UpdateState::Ready,
                UpdateState::Error(String::new()),
            ],
            UpdateState::Ready => vec![UpdateState::Idle],
            UpdateState::Error(_) => vec![UpdateState::Idle],
        }
    }

    /// Check whether transitioning from `self` to `target` is valid.
    /// Error states match any `Error(_)` variant regardless of message.
    pub fn can_transition_to(&self, target: &UpdateState) -> bool {
        self.valid_transitions().iter().any(|valid| {
            std::mem::discriminant(valid) == std::mem::discriminant(target)
        })
    }
}

// ---------------------------------------------------------------------------
// UpdateInfo – parsed version helper
// ---------------------------------------------------------------------------

impl UpdateInfo {
    /// Parse the version string into `VersionParts`.
    pub fn parsed_version(&self) -> Option<VersionParts> {
        VersionParts::parse(&self.version)
    }

    /// Return `true` if this update has release notes.
    pub fn has_release_notes(&self) -> bool {
        self.release_notes.as_ref().map_or(false, |n| !n.is_empty())
    }

    /// Return `true` if a download URL is present.
    pub fn has_download_url(&self) -> bool {
        self.url.is_some()
    }
}

// ---------------------------------------------------------------------------
// UpdateService – validated transitions
// ---------------------------------------------------------------------------

impl UpdateService {
    /// Attempt a validated state transition. Returns an error if the
    /// transition is not allowed by the state machine rules.
    pub fn transition_to(&mut self, target: UpdateState) -> Result<(), UpdateError> {
        if !self.state.can_transition_to(&target) {
            return Err(UpdateError::InvalidStateTransition {
                from: self.state.clone(),
                to: target,
            });
        }
        self.state = target;
        Ok(())
    }

    /// Return the latest version from history, if any.
    pub fn latest_in_history(&self) -> Option<&UpdateInfo> {
        self.update_history
            .iter()
            .filter_map(|info| {
                VersionParts::parse(&info.version).map(|parts| (info, parts))
            })
            .max_by(|(_, a), (_, b)| a.cmp(b))
            .map(|(info, _)| info)
    }
}


// ---------------------------------------------------------------------------
// UpdateChannelSelector
// ---------------------------------------------------------------------------

/// Manages channel selection and channel-specific update policies.
#[derive(Debug, Clone)]
pub struct UpdateChannelSelector {
    current: UpdateChannel,
    allowed: Vec<UpdateChannel>,
}

impl UpdateChannelSelector {
    /// Create a selector defaulting to Stable with all channels allowed.
    pub fn new() -> Self {
        Self {
            current: UpdateChannel::Stable,
            allowed: UpdateChannel::all(),
        }
    }

    /// Create a selector restricted to the given channels.
    pub fn with_allowed(allowed: Vec<UpdateChannel>) -> Self {
        let current = allowed.first().cloned().unwrap_or(UpdateChannel::Stable);
        Self { current, allowed }
    }

    /// Get the currently selected channel.
    pub fn current(&self) -> &UpdateChannel {
        &self.current
    }

    /// Try to select a channel. Returns an error if the channel is not allowed.
    pub fn select(&mut self, channel: UpdateChannel) -> Result<(), UpdateError> {
        if !self.allowed.contains(&channel) {
            return Err(UpdateError::InvalidVersion(format!(
                "channel {} is not allowed",
                channel
            )));
        }
        self.current = channel;
        Ok(())
    }

    /// Check whether a channel is in the allowed list.
    pub fn is_allowed(&self, channel: &UpdateChannel) -> bool {
        self.allowed.contains(channel)
    }

    /// Return the allowed channels.
    pub fn allowed_channels(&self) -> &[UpdateChannel] {
        &self.allowed
    }

    /// Whether the current channel receives preview/insider builds.
    pub fn receives_previews(&self) -> bool {
        self.current.is_preview()
    }

    /// Cycle to the next allowed channel, wrapping around.
    pub fn cycle_next(&mut self) {
        if self.allowed.is_empty() {
            return;
        }
        let idx = self.allowed.iter().position(|c| c == &self.current).unwrap_or(0);
        self.current = self.allowed[(idx + 1) % self.allowed.len()].clone();
    }

    /// Display name of the current channel.
    pub fn display_name(&self) -> String {
        format!("{}", self.current)
    }
}

// ---------------------------------------------------------------------------
// UpdateRollback
// ---------------------------------------------------------------------------

/// An entry in the rollback history.
#[derive(Debug, Clone, PartialEq)]
pub struct RollbackEntry {
    pub version: VersionParts,
    pub channel: UpdateChannel,
    pub timestamp_secs: u64,
    pub reason: Option<String>,
}

impl RollbackEntry {
    /// Create a new rollback entry.
    pub fn new(version: VersionParts, channel: UpdateChannel, timestamp_secs: u64) -> Self {
        Self {
            version,
            channel,
            timestamp_secs,
            reason: None,
        }
    }

    /// Attach a rollback reason.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl fmt::Display for RollbackEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{} ({})", self.version, self.channel)?;
        if let Some(reason) = &self.reason {
            write!(f, " — {reason}")?;
        }
        Ok(())
    }
}

/// Manages a list of previous versions that the user can roll back to.
#[derive(Debug, Clone)]
pub struct UpdateRollbackHistory {
    entries: Vec<RollbackEntry>,
    max_entries: usize,
}

impl UpdateRollbackHistory {
    /// Create a history with a maximum capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Record a new rollback entry.
    pub fn push(&mut self, entry: RollbackEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Get all entries, oldest first.
    pub fn entries(&self) -> &[RollbackEntry] {
        &self.entries
    }

    /// Get the most recent entry.
    pub fn latest(&self) -> Option<&RollbackEntry> {
        self.entries.last()
    }

    /// Find entries for a specific channel.
    pub fn for_channel(&self, channel: &UpdateChannel) -> Vec<&RollbackEntry> {
        self.entries.iter().filter(|e| &e.channel == channel).collect()
    }

    /// Check if a specific version has been rolled back before.
    pub fn was_rolled_back(&self, version: &VersionParts) -> bool {
        self.entries.iter().any(|e| &e.version == version)
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Update notification scheduler
// ---------------------------------------------------------------------------

/// When to show update notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPolicy {
    /// Show immediately when an update is available.
    Immediate,
    /// Show after a delay (in seconds).
    Delayed(u64),
    /// Only show when the user opens the update panel.
    OnDemand,
    /// Never show notifications.
    Silent,
}

/// Schedules and manages update notifications.
#[derive(Debug, Clone)]
pub struct UpdateNotificationScheduler {
    policy: NotificationPolicy,
    pending_version: Option<String>,
    dismissed_versions: Vec<String>,
    snooze_until_secs: Option<u64>,
}

impl UpdateNotificationScheduler {
    /// Create a new scheduler with the given policy.
    pub fn new(policy: NotificationPolicy) -> Self {
        Self {
            policy,
            pending_version: None,
            dismissed_versions: Vec::new(),
            snooze_until_secs: None,
        }
    }

    /// Set a pending update version.
    pub fn set_pending(&mut self, version: impl Into<String>) {
        self.pending_version = Some(version.into());
    }

    /// Dismiss the current notification.
    pub fn dismiss(&mut self) {
        if let Some(v) = self.pending_version.take() {
            self.dismissed_versions.push(v);
        }
    }

    /// Snooze notifications until a given timestamp.
    pub fn snooze_until(&mut self, timestamp_secs: u64) {
        self.snooze_until_secs = Some(timestamp_secs);
    }

    /// Check whether a notification should be shown at the given time.
    pub fn should_show(&self, current_time_secs: u64) -> bool {
        if self.policy == NotificationPolicy::Silent {
            return false;
        }
        if self.policy == NotificationPolicy::OnDemand {
            return false;
        }
        if let Some(snooze) = self.snooze_until_secs {
            if current_time_secs < snooze {
                return false;
            }
        }
        if let Some(pending) = &self.pending_version {
            if self.dismissed_versions.contains(pending) {
                return false;
            }
            match self.policy {
                NotificationPolicy::Immediate => true,
                NotificationPolicy::Delayed(delay) => current_time_secs >= delay,
                _ => false,
            }
        } else {
            false
        }
    }

    /// Return the pending version, if any.
    pub fn pending(&self) -> Option<&str> {
        self.pending_version.as_deref()
    }

    /// Return the current policy.
    pub fn policy(&self) -> NotificationPolicy {
        self.policy
    }

    /// Change the policy.
    pub fn set_policy(&mut self, policy: NotificationPolicy) {
        self.policy = policy;
    }
}

// ---------------------------------------------------------------------------
// Update download verifier
// ---------------------------------------------------------------------------

/// Result of verifying a downloaded update artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyResult {
    /// The artifact passed verification.
    Ok,
    /// The checksum did not match.
    ChecksumMismatch { expected: String, actual: String },
    /// The file size was wrong.
    SizeMismatch { expected: u64, actual: u64 },
    /// The signature could not be verified.
    SignatureInvalid(String),
}

impl VerifyResult {
    /// Whether the result indicates success.
    pub fn is_ok(&self) -> bool {
        matches!(self, VerifyResult::Ok)
    }
}

impl fmt::Display for VerifyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyResult::Ok => write!(f, "verification passed"),
            VerifyResult::ChecksumMismatch { expected, actual } => {
                write!(f, "checksum mismatch: expected {expected}, got {actual}")
            }
            VerifyResult::SizeMismatch { expected, actual } => {
                write!(f, "size mismatch: expected {expected}, got {actual}")
            }
            VerifyResult::SignatureInvalid(msg) => write!(f, "invalid signature: {msg}"),
        }
    }
}

/// Verifies downloaded update artifacts against expected metadata.
#[derive(Debug, Clone)]
pub struct UpdateDownloadVerifier {
    expected_checksum: Option<String>,
    expected_size: Option<u64>,
}

impl UpdateDownloadVerifier {
    /// Create a new verifier.
    pub fn new() -> Self {
        Self {
            expected_checksum: None,
            expected_size: None,
        }
    }

    /// Set the expected checksum (hex-encoded hash).
    pub fn expect_checksum(mut self, checksum: impl Into<String>) -> Self {
        self.expected_checksum = Some(checksum.into());
        self
    }

    /// Set the expected file size in bytes.
    pub fn expect_size(mut self, size: u64) -> Self {
        self.expected_size = Some(size);
        self
    }

    /// Verify the given artifact data against expectations.
    pub fn verify(&self, actual_checksum: &str, actual_size: u64) -> VerifyResult {
        if let Some(expected) = &self.expected_size {
            if *expected != actual_size {
                return VerifyResult::SizeMismatch {
                    expected: *expected,
                    actual: actual_size,
                };
            }
        }
        if let Some(expected) = &self.expected_checksum {
            if expected != actual_checksum {
                return VerifyResult::ChecksumMismatch {
                    expected: expected.clone(),
                    actual: actual_checksum.to_string(),
                };
            }
        }
        VerifyResult::Ok
    }

    /// Quick check: does the size match?
    pub fn size_ok(&self, actual_size: u64) -> bool {
        self.expected_size.map_or(true, |s| s == actual_size)
    }
}




// ---------------------------------------------------------------------------
// vsedit-update: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl UpdateXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for UpdateXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct UpdateXRegistry {
    entries: Vec<UpdateXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl UpdateXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: UpdateXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&UpdateXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut UpdateXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<UpdateXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&UpdateXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&UpdateXConfig> {
        let mut sorted: Vec<&UpdateXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&UpdateXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> UpdateXIterator<'_> {
        UpdateXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct UpdateXIterator<'a> {
    inner: std::slice::Iter<'a, UpdateXConfig>,
}

impl<'a> Iterator for UpdateXIterator<'a> {
    type Item = &'a UpdateXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct UpdateXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl UpdateXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct UpdateXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl UpdateXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &UpdateXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &UpdateXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &UpdateXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for UpdateXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct UpdateXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl UpdateXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &UpdateXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &UpdateXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for UpdateXValidator {
    fn default() -> Self {
        Self::new()
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
// xb_ utilities – batch 47
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer47 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer47 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_47(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_47<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_47<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_47(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_47(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 191
// ---------------------------------------------------------------------------

/// Generic object pool `Xc191Pool<T>`.
pub struct Xc191Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc191Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc191PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc191Pool<T> {
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
    pub fn stats(&self) -> Xc191PoolStats {
        Xc191PoolStats {
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

impl<T> Default for Xc191Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc191Scheduler`.
pub struct Xc191Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc191Scheduler {
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

impl Default for Xc191Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_191 hash for the given byte slice.
pub fn xc_191_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_191 convention.
pub fn xc_191_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe60 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe60Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe60PipelineError {
    pub stage: Xe60Stage,
    pub message: String,
}

impl std::fmt::Display for Xe60PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe60Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe60Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError>>>,
    stage_names: Vec<Xe60Stage>,
}

impl Xe60Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe60Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe60Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe60Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe60Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe60Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe60CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe60CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe60Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe60CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe60CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe60Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe60CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_60_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe60CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_60_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe60CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_60_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> {
    Ok(data)
}

pub fn xe_60_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_60_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_60_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_60_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe60PipelineError> {
    Err(Xe60PipelineError {
        stage: Xe60Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_58: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg58Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg58Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg58Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_58: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg58Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg58Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg58Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg58Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 190).
pub struct Xh190SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh190SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 232 as u64,
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

/// A compact bit set supporting boolean operations (variant 190).
pub struct Xh190BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh190BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 190).
pub struct Xi190Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi190Deque<T> {
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
pub struct Xi190Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi190Interval {
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

/// A simple interval tree (variant 190).
pub struct Xi190IntervalTree {
    xi_intervals: Vec<Xi190Interval>,
}

impl Xi190IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi190Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi190Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi190Interval) -> Vec<&Xi190Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi190Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi190Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi190Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi190Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi190Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi190Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 190) ---

/// Disjoint set / union-find for crate 190.
pub struct Xj190UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj190UnionFind {
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

const XJ190_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 190.
pub struct Xj190BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj190BTreeNode<K, V>>>,
    len: usize,
}

struct Xj190BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj190BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj190BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ190_BTREE_ORDER - 1
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
        let mid = XJ190_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj190BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj190BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj190BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj190BTreeNode::xj_new_leaf();
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


// --- xk_190 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk190SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk190SegmentTree {
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
pub struct Xk190DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk190DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_190).
#[derive(Debug, Clone)]
pub struct Xl190Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl190Rope {
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

/// Suffix array for efficient string searching (xl_190).
#[derive(Debug, Clone)]
pub struct Xl190SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl190SuffixArray {
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
pub struct Xm190MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm190MatrixSparse {
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
pub struct Xm190Tokenizer {
    text: String,
}

impl Xm190Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 190.
pub struct Xn190Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn190Fenwick {
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

// ----- AVL tree map — crate 190 -----

#[derive(Debug, Clone)]
struct Xn190AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn190AvlNode<K, V>>>,
    right: Option<Box<Xn190AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 190.
#[derive(Debug, Clone)]
pub struct Xn190AVL<K, V> {
    root: Option<Box<Xn190AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn190AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn190AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn190AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn190AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn190AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn190AvlNode<K, V>>) -> Box<Xn190AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn190AvlNode<K, V>>) -> Box<Xn190AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn190AvlNode<K, V>>) -> Box<Xn190AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn190AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn190AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn190AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn190AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn190AvlNode<K, V>>) -> &Xn190AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn190AvlNode<K, V>>) -> (Box<Xn190AvlNode<K, V>>, Option<Box<Xn190AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn190AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn190AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn190AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn190AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn190AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn190AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn190AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo190RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo190Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo190RBNode<K, V> {
    key: K,
    value: V,
    color: Xo190Color,
    left: Option<Box<Xo190RBNode<K, V>>>,
    right: Option<Box<Xo190RBNode<K, V>>>,
}

/// A red-black tree map for crate 190.
#[derive(Debug, Clone)]
pub struct Xo190RedBlack<K, V> {
    root: Option<Box<Xo190RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo190RedBlack<K, V> {
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
            r.color = Xo190Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo190RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo190RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo190RBNode {
                    key, value, color: Xo190Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo190RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo190Color::Red)
    }

    fn xo_balance(mut h: Box<Xo190RBNode<K, V>>) -> Box<Xo190RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo190Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo190RBNode<K, V>>) -> Box<Xo190RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo190Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo190RBNode<K, V>>) -> Box<Xo190RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo190Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo190RBNode<K, V>>) {
        h.color = Xo190Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo190Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo190Color::Black; }
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
            r.color = Xo190Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo190RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo190RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo190RBNode<K, V>) -> (K, V, Option<Box<Xo190RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo190RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo190Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo190RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo190ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 190.
#[derive(Debug, Clone)]
pub struct Xo190ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo190ConsistentHash {
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
            let vkey = format!("{}#xo190#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo190#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 190).
#[derive(Debug)]
pub struct Xp190SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp190Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp190Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp190Node<K, V>>>,
    xp_right: Option<Box<Xp190Node<K, V>>>,
}

impl<K: Ord, V> Xp190Node<K, V> {
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

impl<K: Ord, V> Default for Xp190SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp190SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp190Node<K, V>>>, key: &K) -> Option<Box<Xp190Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp190Node<K, V>>) -> Box<Xp190Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp190Node<K, V>>) -> Box<Xp190Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp190Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp190Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp190Node::xp_new(key, val));
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


// --------------- Xq190Treap ---------------

use std::cmp::Ordering as Xq190Ord;

struct Xq190TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq190TreapNode<K, V>>>,
    right: Option<Box<Xq190TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq190Treap<K, V> {
    root: Option<Box<Xq190TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq190TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_190_size<K, V>(node: &Option<Box<Xq190TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_190_update_size<K, V>(node: &mut Xq190TreapNode<K, V>) {
    node.size = 1 + xq_190_size(&node.left) + xq_190_size(&node.right);
}

fn xq_190_rotate_right<K, V>(mut node: Box<Xq190TreapNode<K, V>>) -> Box<Xq190TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_190_update_size(&mut node);
    left.right = Some(node);
    xq_190_update_size(&mut left);
    left
}

fn xq_190_rotate_left<K, V>(mut node: Box<Xq190TreapNode<K, V>>) -> Box<Xq190TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_190_update_size(&mut node);
    right.left = Some(node);
    xq_190_update_size(&mut right);
    right
}

fn xq_190_insert_node<K: Ord, V>(
    node: Option<Box<Xq190TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq190TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq190TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq190Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq190Ord::Less => {
                let (new_left, old) = xq_190_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_190_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_190_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq190Ord::Greater => {
                let (new_right, old) = xq_190_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_190_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_190_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_190_remove_node<K: Ord, V>(
    node: Option<Box<Xq190TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq190TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq190Ord::Less => {
                let (new_left, old) = xq_190_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_190_update_size(&mut n);
                (Some(n), old)
            }
            Xq190Ord::Greater => {
                let (new_right, old) = xq_190_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_190_update_size(&mut n);
                (Some(n), old)
            }
            Xq190Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_190_rotate_right(n);
                    let (new_right, old) = xq_190_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_190_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_190_rotate_left(n);
                    let (new_left, old) = xq_190_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_190_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_190_find_min<K, V>(node: &Option<Box<Xq190TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_190_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_190_find_max<K, V>(node: &Option<Box<Xq190TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_190_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_190_rank<K: Ord, V>(node: &Option<Box<Xq190TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq190Ord::Less => xq_190_rank(&n.left, key),
            Xq190Ord::Equal => xq_190_size(&n.left),
            Xq190Ord::Greater => 1 + xq_190_size(&n.left) + xq_190_rank(&n.right, key),
        },
    }
}

fn xq_190_kth<K, V>(node: &Option<Box<Xq190TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_190_size(&n.left);
        if k < left_size {
            xq_190_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_190_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_190_in_order<K: Clone, V>(node: &Option<Box<Xq190TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_190_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_190_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq190Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 190 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_190_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq190Ord::Equal => return Some(&n.value),
                Xq190Ord::Less => cur = &n.left,
                Xq190Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_190_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_190_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_190_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_190_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_190_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_190_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_190_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq190VEBTree ---------------

pub struct Xq190VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq190VEBTree>>,
    clusters: Vec<Option<Box<Xq190VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq190VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq190VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq190VEBTree::xq_new(self.sqrt_lo)));
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
pub struct Xr190KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr190KDPoint {
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
pub struct Xr190BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr190KDNode {
    xr_point: Xr190KDPoint,
    xr_left: Option<Box<Xr190KDNode>>,
    xr_right: Option<Box<Xr190KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr190KDTree {
    xr_root: Option<Box<Xr190KDNode>>,
    xr_size: usize,
}

impl Xr190KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr190KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr190KDNode>>,
        point: Xr190KDPoint,
        depth: usize,
    ) -> Box<Xr190KDNode> {
        match node {
            None => Box::new(Xr190KDNode {
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
    pub fn xr_nearest_neighbor(&self, query: &Xr190KDPoint) -> Option<Xr190KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr190KDNode>,
        query: &Xr190KDPoint,
        depth: usize,
        best: &mut Xr190KDPoint,
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
    ) -> Vec<Xr190KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr190KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr190KDPoint>,
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
    pub fn xr_all_points(&self) -> Vec<Xr190KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr190KDNode>>, pts: &mut Vec<Xr190KDPoint>) {
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

    fn xr_depth_rec(node: &Option<Box<Xr190KDNode>>) -> usize {
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
    pub fn xr_bounding_box(&self) -> Option<Xr190BoundingBox> {
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
        Some(Xr190BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_update_needed() {
        let mut svc = UpdateService::new("1.0.0");
        let info = UpdateInfo {
            version: "1.0.0".into(),
            product_version: "1.0.0".into(),
            url: None,
            release_notes: None,
        };
        assert!(!svc.check_for_update(info));
        assert_eq!(*svc.get_state(), UpdateState::Idle);
    }

    #[test]
    fn update_available() {
        let mut svc = UpdateService::new("1.0.0");
        let info = UpdateInfo {
            version: "2.0.0".into(),
            product_version: "2.0.0".into(),
            url: Some("https://example.com".into()),
            release_notes: Some("New features".into()),
        };
        assert!(svc.check_for_update(info));
        assert_eq!(*svc.get_state(), UpdateState::UpdateAvailable);
        assert!(svc.get_available_update().is_some());
    }

    #[test]
    fn needs_update_comparison() {
        let svc = UpdateService::new("1.0.0");
        assert!(svc.needs_update("2.0.0"));
        assert!(!svc.needs_update("1.0.0"));
    }

    #[test]
    fn semver_needs_update() {
        let svc = UpdateService::new("1.2.3");
        assert!(svc.needs_update("1.2.4"));
        assert!(svc.needs_update("1.3.0"));
        assert!(svc.needs_update("2.0.0"));
        assert!(!svc.needs_update("1.2.3"));
        assert!(!svc.needs_update("1.2.2"));
        assert!(!svc.needs_update("1.1.9"));
        assert!(!svc.needs_update("0.9.9"));
    }

    #[test]
    fn version_parts_parse() {
        let v = VersionParts::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(VersionParts::parse("bad").is_none());
        assert!(VersionParts::parse("1.2").is_none());
    }

    #[test]
    fn update_channel_display() {
        assert_eq!(UpdateChannel::Stable.to_string(), "Stable");
        assert_eq!(UpdateChannel::Insider.to_string(), "Insider");
        assert_eq!(UpdateChannel::Exploration.to_string(), "Exploration");
    }

    #[test]
    fn update_state_display() {
        assert_eq!(UpdateState::Idle.to_string(), "Idle");
        assert_eq!(UpdateState::Downloading.to_string(), "Downloading");
        assert_eq!(
            UpdateState::Error("fail".into()).to_string(),
            "Error: fail"
        );
    }

    #[test]
    fn download_progress_tracking() {
        let mut svc = UpdateService::new("1.0.0");
        assert_eq!(svc.get_progress(), None);
        svc.download_progress(0.5);
        assert_eq!(*svc.get_state(), UpdateState::Downloading);
        assert_eq!(svc.get_progress(), Some(0.5));
    }

    #[test]
    fn apply_update_from_ready() {
        let mut svc = UpdateService::new("1.0.0");
        svc.set_state(UpdateState::Ready);
        svc.apply_update();
        assert_eq!(*svc.get_state(), UpdateState::Idle);
    }

    #[test]
    fn apply_update_ignored_if_not_ready() {
        let mut svc = UpdateService::new("1.0.0");
        svc.set_state(UpdateState::Downloading);
        svc.apply_update();
        assert_eq!(*svc.get_state(), UpdateState::Downloading);
    }

    #[test]
    fn dismiss_update_resets() {
        let mut svc = UpdateService::new("1.0.0");
        let info = UpdateInfo {
            version: "2.0.0".into(),
            product_version: "2.0.0".into(),
            url: None,
            release_notes: None,
        };
        svc.check_for_update(info);
        svc.dismiss_update();
        assert_eq!(*svc.get_state(), UpdateState::Idle);
        assert!(svc.get_available_update().is_none());
    }

    #[test]
    fn default_channel_is_stable() {
        let svc = UpdateService::new("1.0.0");
        assert_eq!(*svc.get_channel(), UpdateChannel::Stable);
    }

    #[test]
    fn version_parts_display() {
        let v = VersionParts { major: 3, minor: 11, patch: 7 };
        assert_eq!(v.to_string(), "3.11.7");
    }

    #[test]
    fn version_parts_ordering() {
        let v1 = VersionParts::parse("1.2.3").unwrap();
        let v2 = VersionParts::parse("1.3.0").unwrap();
        let v3 = VersionParts::parse("2.0.0").unwrap();
        assert!(v1 < v2);
        assert!(v2 < v3);
        assert_eq!(v1, VersionParts::parse("1.2.3").unwrap());
    }

    #[test]
    fn version_bump_methods() {
        let v = VersionParts::parse("1.2.3").unwrap();
        assert_eq!(v.bump_patch(), VersionParts { major: 1, minor: 2, patch: 4 });
        assert_eq!(v.bump_minor(), VersionParts { major: 1, minor: 3, patch: 0 });
        assert_eq!(v.bump_major(), VersionParts { major: 2, minor: 0, patch: 0 });
    }

    #[test]
    fn version_as_numeric() {
        let v = VersionParts::parse("2.5.9").unwrap();
        assert_eq!(v.as_numeric(), 2_005_009);
    }

    #[test]
    fn version_is_prerelease() {
        assert!(VersionParts::parse("0.1.0").unwrap().is_prerelease());
        assert!(!VersionParts::parse("1.0.0").unwrap().is_prerelease());
    }

    #[test]
    fn update_info_builder_success() {
        let info = UpdateInfoBuilder::new()
            .version("1.2.3")
            .url("https://example.com/update")
            .release_notes("Bug fixes")
            .build()
            .unwrap();
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.product_version, "1.2.3");
        assert_eq!(info.url.as_deref(), Some("https://example.com/update"));
    }

    #[test]
    fn update_info_builder_missing_version() {
        let result = UpdateInfoBuilder::new().build();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UpdateError::InvalidVersion(_)));
    }

    #[test]
    fn update_info_builder_bad_version() {
        let result = UpdateInfoBuilder::new().version("not-a-version").build();
        assert!(matches!(result.unwrap_err(), UpdateError::InvalidVersion(_)));
    }

    #[test]
    fn update_info_display() {
        let info = UpdateInfo {
            version: "2.0.0".into(),
            product_version: "2.0.0".into(),
            url: Some("https://example.com".into()),
            release_notes: None,
        };
        assert_eq!(info.to_string(), "v2.0.0 (https://example.com)");

        let info_no_url = UpdateInfo {
            version: "1.0.0".into(),
            product_version: "1.0.0".into(),
            url: None,
            release_notes: None,
        };
        assert_eq!(info_no_url.to_string(), "v1.0.0");
    }

    #[test]
    fn start_download_success() {
        let mut svc = UpdateService::new("1.0.0");
        let info = UpdateInfo {
            version: "2.0.0".into(),
            product_version: "2.0.0".into(),
            url: Some("https://example.com/dl".into()),
            release_notes: None,
        };
        svc.check_for_update(info);
        let url = svc.start_download().unwrap();
        assert_eq!(url, "https://example.com/dl");
        assert!(svc.is_downloading());
        assert_eq!(svc.get_progress(), Some(0.0));
    }

    #[test]
    fn start_download_wrong_state() {
        let mut svc = UpdateService::new("1.0.0");
        let result = svc.start_download();
        assert!(matches!(
            result.unwrap_err(),
            UpdateError::InvalidStateTransition { .. }
        ));
    }

    #[test]
    fn start_download_missing_url() {
        let mut svc = UpdateService::new("1.0.0");
        let info = UpdateInfo {
            version: "2.0.0".into(),
            product_version: "2.0.0".into(),
            url: None,
            release_notes: None,
        };
        svc.check_for_update(info);
        assert!(matches!(
            svc.start_download().unwrap_err(),
            UpdateError::MissingDownloadUrl
        ));
    }

    #[test]
    fn finish_download_transitions_to_ready() {
        let mut svc = UpdateService::new("1.0.0");
        let info = UpdateInfo {
            version: "2.0.0".into(),
            product_version: "2.0.0".into(),
            url: Some("https://example.com".into()),
            release_notes: None,
        };
        svc.check_for_update(info);
        svc.start_download().unwrap();
        svc.finish_download().unwrap();
        assert!(svc.is_ready_to_install());
        assert_eq!(svc.get_progress(), Some(1.0));
    }

    #[test]
    fn report_and_reset_error() {
        let mut svc = UpdateService::new("1.0.0");
        svc.report_error("timeout");
        assert_eq!(*svc.get_state(), UpdateState::Error("timeout".into()));
        svc.reset_from_error();
        assert_eq!(*svc.get_state(), UpdateState::Idle);
    }

    #[test]
    fn set_channel() {
        let mut svc = UpdateService::new("1.0.0");
        svc.set_channel(UpdateChannel::Insider);
        assert_eq!(*svc.get_channel(), UpdateChannel::Insider);
    }

    #[test]
    fn current_version_parts() {
        let svc = UpdateService::new("3.2.1");
        let parts = svc.current_version_parts().unwrap();
        assert_eq!(parts, VersionParts { major: 3, minor: 2, patch: 1 });
    }

    #[test]
    fn update_service_display() {
        let svc = UpdateService::new("1.0.0");
        assert_eq!(
            svc.to_string(),
            "UpdateService(v1.0.0, channel=Stable, state=Idle)"
        );
    }

    #[test]
    fn update_error_display() {
        let e = UpdateError::MissingDownloadUrl;
        assert_eq!(e.to_string(), "download URL is missing");
        let e2 = UpdateError::NetworkError("timeout".into());
        assert_eq!(e2.to_string(), "network error: timeout");
    }

    #[test]
    fn update_error_is_std_error() {
        let e: Box<dyn std::error::Error> =
            Box::new(UpdateError::InvalidVersion("bad".into()));
        assert!(e.to_string().contains("bad"));
    }

    #[test]
    fn update_stats_new_defaults() {
        let stats = UpdateStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn update_stats_record_success() {
        let mut stats = UpdateStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn update_stats_record_failure() {
        let mut stats = UpdateStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn update_stats_reset() {
        let mut stats = UpdateStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn update_stats_merge() {
        let mut a = UpdateStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = UpdateStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn update_stats_display() {
        let mut stats = UpdateStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn update_stats_default() {
        let stats = UpdateStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn update_validator_accepts_valid_name() {
        let v = UpdateValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn update_validator_rejects_empty() {
        let v = UpdateValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn update_validator_rejects_too_long() {
        let v = UpdateValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn update_validator_forbidden_prefix() {
        let v = UpdateValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn update_validator_allowed_chars() {
        let v = UpdateValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn update_validator_range() {
        let v = UpdateValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn update_sanitize_removes_control() {
        let result = UpdateValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn update_truncate_short_string() {
        assert_eq!(UpdateValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn update_truncate_long_string() {
        let result = UpdateValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn update_is_ascii_printable() {
        assert!(UpdateValidator::is_ascii_printable("Hello World 123"));
        assert!(!UpdateValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- update_check_interval tests ----------------------------------------

    #[test]
    fn interval_as_secs() {
        assert_eq!(UpdateCheckInterval::Hourly.as_secs(), Some(3_600));
        assert_eq!(UpdateCheckInterval::Daily.as_secs(), Some(86_400));
        assert_eq!(UpdateCheckInterval::Weekly.as_secs(), Some(604_800));
        assert_eq!(UpdateCheckInterval::Never.as_secs(), None);
        assert_eq!(UpdateCheckInterval::Custom(120).as_secs(), Some(120));
    }

    #[test]
    fn interval_is_enabled() {
        assert!(UpdateCheckInterval::Hourly.is_enabled());
        assert!(!UpdateCheckInterval::Never.is_enabled());
    }

    #[test]
    fn interval_from_label() {
        assert_eq!(UpdateCheckInterval::from_label("hourly"), UpdateCheckInterval::Hourly);
        assert_eq!(UpdateCheckInterval::from_label("DAILY"), UpdateCheckInterval::Daily);
        assert_eq!(UpdateCheckInterval::from_label("never"), UpdateCheckInterval::Never);
        assert_eq!(UpdateCheckInterval::from_label("off"), UpdateCheckInterval::Never);
        assert_eq!(UpdateCheckInterval::from_label("3600"), UpdateCheckInterval::Custom(3600));
    }

    #[test]
    fn check_interval_enough_time() {
        assert!(update_check_interval(UpdateCheckInterval::Hourly, 0, 3_600));
        assert!(!update_check_interval(UpdateCheckInterval::Hourly, 0, 3_599));
    }

    #[test]
    fn check_interval_never() {
        assert!(!update_check_interval(UpdateCheckInterval::Never, 0, 999_999));
    }

    #[test]
    fn interval_display() {
        assert_eq!(format!("{}", UpdateCheckInterval::Daily), "daily");
        assert_eq!(format!("{}", UpdateCheckInterval::Custom(60)), "every 60s");
    }

    #[test]
    fn version_distance_to() {
        let v1 = VersionParts::parse("1.2.3").unwrap();
        let v2 = VersionParts::parse("2.4.1").unwrap();
        assert_eq!(v1.distance_to(&v2), (1, 2, -2));
        assert_eq!(v2.distance_to(&v1), (-1, -2, 2));
        assert_eq!(v1.distance_to(&v1), (0, 0, 0));
    }

    #[test]
    fn version_is_compatible_with() {
        let v1 = VersionParts::parse("1.2.3").unwrap();
        let v2 = VersionParts::parse("1.9.0").unwrap();
        let v3 = VersionParts::parse("2.0.0").unwrap();
        assert!(v1.is_compatible_with(&v2));
        assert!(!v1.is_compatible_with(&v3));
    }

    #[test]
    fn update_channel_all_and_preview() {
        let all = UpdateChannel::all();
        assert_eq!(all.len(), 3);
        assert!(!UpdateChannel::Stable.is_preview());
        assert!(UpdateChannel::Insider.is_preview());
        assert!(UpdateChannel::Exploration.is_preview());
    }

    #[test]
    fn update_state_terminal_and_error() {
        assert!(!UpdateState::Idle.is_terminal());
        assert!(!UpdateState::CheckingForUpdates.is_terminal());
        assert!(!UpdateState::Downloading.is_terminal());
        assert!(UpdateState::Ready.is_terminal());
        assert!(UpdateState::Error("oops".into()).is_terminal());
        assert!(!UpdateState::Idle.is_error());
        assert!(UpdateState::Error("fail".into()).is_error());
    }

    #[test]
    fn update_service_reset() {
        let mut svc = UpdateService::new("1.0.0");
        svc.download_progress(0.5);
        svc.reset();
        assert_eq!(*svc.get_state(), UpdateState::Idle);
        assert_eq!(svc.get_progress(), None);
        assert!(svc.get_available_update().is_none());
    }

    #[test]
    fn update_service_history_and_into_iter() {
        let mut svc = UpdateService::new("1.0.0");
        assert!(svc.history().is_empty());
        let info1 = UpdateInfo {
            version: "1.1.0".into(),
            product_version: "1.1.0".into(),
            url: None,
            release_notes: None,
        };
        let info2 = UpdateInfo {
            version: "1.2.0".into(),
            product_version: "1.2.0".into(),
            url: None,
            release_notes: None,
        };
        svc.push_history(info1.clone());
        svc.push_history(info2.clone());
        assert_eq!(svc.history().len(), 2);
        assert_eq!(svc.history()[0], info1);
        let versions: Vec<String> = svc.into_iter().map(|i| i.version).collect();
        assert_eq!(versions, vec!["1.1.0", "1.2.0"]);
    }

    // -- UpdateDiffSummary tests -------------------------------------------

    #[test]
    fn diff_summary_basic() {
        let mut summary = UpdateDiffSummary::new("1.0.0", "1.1.0");
        summary.add_feature("New editor tabs");
        summary.add_fix("Fixed crash on startup");
        assert_eq!(summary.total_changes(), 2);
        assert!(!summary.has_breaking_changes());
        assert!(!summary.is_major_update());
    }

    #[test]
    fn diff_summary_major_update_with_breaking() {
        let mut summary = UpdateDiffSummary::new("1.0.0", "2.0.0");
        summary.add_breaking("API changed");
        assert!(summary.has_breaking_changes());
        assert!(summary.is_major_update());
    }

    #[test]
    fn diff_summary_display() {
        let mut s = UpdateDiffSummary::new("1.0.0", "1.1.0");
        s.add_feature("feat1");
        s.add_fix("fix1");
        let display = format!("{s}");
        assert!(display.contains("1.0.0 → 1.1.0"));
        assert!(display.contains("1 features"));
    }

    // -- UpdateRollbackPlan tests ------------------------------------------

    #[test]
    fn rollback_plan_valid() {
        let plan = UpdateRollbackPlan::new("2.0.0", "1.5.0");
        assert!(plan.is_valid());
        assert!(plan.requires_restart);
        assert!(plan.step_count() >= 4); // includes migration step for major change
        assert!(plan.is_major_rollback());
    }

    #[test]
    fn rollback_plan_same_major() {
        let plan = UpdateRollbackPlan::new("1.5.0", "1.3.0");
        assert!(plan.is_valid());
        assert!(!plan.is_major_rollback());
    }

    #[test]
    fn rollback_plan_invalid_newer_target() {
        let plan = UpdateRollbackPlan::new("1.0.0", "2.0.0");
        assert!(!plan.is_valid());
    }

    #[test]
    fn rollback_plan_display() {
        let plan = UpdateRollbackPlan::new("2.0.0", "1.0.0");
        let s = format!("{plan}");
        assert!(s.contains("Rollback 2.0.0 → 1.0.0"));
    }

    // -- UpdatePrerequisites tests -----------------------------------------

    #[test]
    fn prerequisites_all_satisfied() {
        let mut prereqs = UpdatePrerequisites::new();
        prereqs.require_min_version("1.5.0", "1.0.0");
        prereqs.require_disk_space(1_000_000, 500_000);
        assert!(prereqs.all_satisfied());
        assert_eq!(prereqs.satisfied_count(), 2);
    }

    #[test]
    fn prerequisites_version_too_old() {
        let mut prereqs = UpdatePrerequisites::new();
        prereqs.require_min_version("0.9.0", "1.0.0");
        assert!(!prereqs.all_satisfied());
        assert_eq!(prereqs.unsatisfied().len(), 1);
        assert_eq!(prereqs.unsatisfied()[0].name, "min_version");
    }

    #[test]
    fn prerequisites_disk_space_insufficient() {
        let mut prereqs = UpdatePrerequisites::new();
        prereqs.require_disk_space(100, 500);
        assert!(!prereqs.all_satisfied());
    }

    // -- VersionParts extension tests --------------------------------------

    #[test]
    fn version_in_range() {
        let v = VersionParts::from_parts(1, 5, 0);
        let min = VersionParts::from_parts(1, 0, 0);
        let max = VersionParts::from_parts(2, 0, 0);
        assert!(v.in_range(&min, &max));
        assert!(!VersionParts::from_parts(0, 9, 0).in_range(&min, &max));
    }

    #[test]
    fn version_diff_severity() {
        let v1 = VersionParts::from_parts(1, 0, 0);
        let v2 = VersionParts::from_parts(2, 0, 0);
        assert_eq!(v1.diff_severity(&v2), "major");
        let v3 = VersionParts::from_parts(1, 1, 0);
        assert_eq!(v1.diff_severity(&v3), "minor");
        let v4 = VersionParts::from_parts(1, 0, 1);
        assert_eq!(v1.diff_severity(&v4), "patch");
        assert_eq!(v1.diff_severity(&v1), "none");
    }

    #[test]
    fn version_increments_from() {
        let v1 = VersionParts::from_parts(1, 2, 3);
        let v2 = VersionParts::from_parts(3, 0, 1);
        assert_eq!(v1.increments_from(&v2), (2, 2, 2));
    }

    #[test]
    fn version_is_newer_than() {
        let v1 = VersionParts::from_parts(1, 0, 0);
        let v2 = VersionParts::from_parts(2, 0, 0);
        assert!(v2.is_newer_than(&v1));
        assert!(!v1.is_newer_than(&v2));
    }

    // -- VersionParts::satisfies constraint tests ----------------------------

    #[test]
    fn satisfies_exact_match() {
        let v = VersionParts::from_parts(1, 2, 3);
        assert!(v.satisfies("1.2.3"));
        assert!(v.satisfies("=1.2.3"));
        assert!(!v.satisfies("1.2.4"));
        assert!(!v.satisfies("=1.2.4"));
    }

    #[test]
    fn satisfies_greater_less() {
        let v = VersionParts::from_parts(1, 5, 0);
        assert!(v.satisfies(">1.0.0"));
        assert!(!v.satisfies(">1.5.0"));
        assert!(v.satisfies(">=1.5.0"));
        assert!(v.satisfies("<2.0.0"));
        assert!(!v.satisfies("<1.5.0"));
        assert!(v.satisfies("<=1.5.0"));
    }

    #[test]
    fn satisfies_caret() {
        let v13 = VersionParts::from_parts(1, 9, 0);
        assert!(v13.satisfies("^1.2.3"));
        let v20 = VersionParts::from_parts(2, 0, 0);
        assert!(!v20.satisfies("^1.2.3"));
        let v_old = VersionParts::from_parts(1, 0, 0);
        assert!(!v_old.satisfies("^1.2.3"));
    }

    #[test]
    fn satisfies_tilde() {
        let v = VersionParts::from_parts(1, 2, 9);
        assert!(v.satisfies("~1.2.3"));
        let v_next_minor = VersionParts::from_parts(1, 3, 0);
        assert!(!v_next_minor.satisfies("~1.2.3"));
    }

    #[test]
    fn satisfies_empty_and_invalid() {
        let v = VersionParts::from_parts(1, 0, 0);
        assert!(!v.satisfies(""));
        assert!(!v.satisfies(">not_a_version"));
    }

    // -- VersionRange tests -------------------------------------------------

    #[test]
    fn version_range_creation() {
        let min = VersionParts::from_parts(1, 0, 0);
        let max = VersionParts::from_parts(2, 0, 0);
        let range = VersionRange::new(min.clone(), max.clone()).unwrap();
        assert_eq!(range.min, min);
        assert_eq!(range.max, max);
    }

    #[test]
    fn version_range_rejects_inverted() {
        let high = VersionParts::from_parts(3, 0, 0);
        let low = VersionParts::from_parts(1, 0, 0);
        assert!(VersionRange::new(high, low).is_none());
    }

    #[test]
    fn version_range_contains() {
        let range = VersionRange::new(
            VersionParts::from_parts(1, 0, 0),
            VersionParts::from_parts(2, 0, 0),
        )
        .unwrap();
        assert!(range.contains(&VersionParts::from_parts(1, 5, 0)));
        assert!(range.contains(&VersionParts::from_parts(1, 0, 0))); // inclusive min
        assert!(range.contains(&VersionParts::from_parts(2, 0, 0))); // inclusive max
        assert!(!range.contains(&VersionParts::from_parts(0, 9, 0)));
        assert!(!range.contains(&VersionParts::from_parts(2, 0, 1)));
    }

    #[test]
    fn version_range_filter() {
        let range = VersionRange::new(
            VersionParts::from_parts(1, 0, 0),
            VersionParts::from_parts(1, 5, 0),
        )
        .unwrap();
        let versions = vec![
            VersionParts::from_parts(0, 9, 0),
            VersionParts::from_parts(1, 0, 0),
            VersionParts::from_parts(1, 3, 0),
            VersionParts::from_parts(1, 5, 0),
            VersionParts::from_parts(2, 0, 0),
        ];
        let filtered = range.filter(&versions);
        assert_eq!(filtered.len(), 3);
        assert_eq!(*filtered[0], VersionParts::from_parts(1, 0, 0));
        assert_eq!(*filtered[2], VersionParts::from_parts(1, 5, 0));
    }

    #[test]
    fn version_range_major_span() {
        let range = VersionRange::new(
            VersionParts::from_parts(1, 0, 0),
            VersionParts::from_parts(3, 0, 0),
        )
        .unwrap();
        assert_eq!(range.major_span(), 3);
    }

    #[test]
    fn version_range_display() {
        let range = VersionRange::new(
            VersionParts::from_parts(1, 0, 0),
            VersionParts::from_parts(2, 0, 0),
        )
        .unwrap();
        assert_eq!(range.to_string(), "[1.0.0, 2.0.0]");
    }

    // -- UpdateChannel::from_name and priority tests -------------------------

    #[test]
    fn update_channel_from_name() {
        assert_eq!(UpdateChannel::from_name("stable"), Some(UpdateChannel::Stable));
        assert_eq!(UpdateChannel::from_name("INSIDER"), Some(UpdateChannel::Insider));
        assert_eq!(UpdateChannel::from_name("Insiders"), Some(UpdateChannel::Insider));
        assert_eq!(UpdateChannel::from_name("exploration"), Some(UpdateChannel::Exploration));
        assert_eq!(UpdateChannel::from_name("explore"), Some(UpdateChannel::Exploration));
        assert_eq!(UpdateChannel::from_name("unknown"), None);
    }

    #[test]
    fn update_channel_stability_ordering() {
        assert!(UpdateChannel::Stable.stability_priority() < UpdateChannel::Insider.stability_priority());
        assert!(UpdateChannel::Insider.stability_priority() < UpdateChannel::Exploration.stability_priority());
        assert!(UpdateChannel::Stable.is_at_least_as_stable_as(&UpdateChannel::Insider));
        assert!(!UpdateChannel::Exploration.is_at_least_as_stable_as(&UpdateChannel::Stable));
        assert!(UpdateChannel::Insider.is_at_least_as_stable_as(&UpdateChannel::Insider));
    }

    // -- UpdateState transition tests ----------------------------------------

    #[test]
    fn state_valid_transitions() {
        assert!(UpdateState::Idle.can_transition_to(&UpdateState::CheckingForUpdates));
        assert!(!UpdateState::Idle.can_transition_to(&UpdateState::Ready));
        assert!(UpdateState::CheckingForUpdates.can_transition_to(&UpdateState::UpdateAvailable));
        assert!(UpdateState::CheckingForUpdates.can_transition_to(&UpdateState::Idle));
        assert!(UpdateState::CheckingForUpdates.can_transition_to(&UpdateState::Error("x".into())));
        assert!(UpdateState::Downloading.can_transition_to(&UpdateState::Ready));
        assert!(!UpdateState::Downloading.can_transition_to(&UpdateState::Idle));
        assert!(UpdateState::Ready.can_transition_to(&UpdateState::Idle));
        assert!(!UpdateState::Ready.can_transition_to(&UpdateState::Downloading));
        assert!(UpdateState::Error("e".into()).can_transition_to(&UpdateState::Idle));
    }

    // -- UpdateInfo helper tests --------------------------------------------

    #[test]
    fn update_info_parsed_version() {
        let info = UpdateInfo {
            version: "3.2.1".into(),
            product_version: "3.2.1".into(),
            url: None,
            release_notes: None,
        };
        let parts = info.parsed_version().unwrap();
        assert_eq!(parts, VersionParts::from_parts(3, 2, 1));
    }

    #[test]
    fn update_info_has_release_notes_and_url() {
        let info = UpdateInfo {
            version: "1.0.0".into(),
            product_version: "1.0.0".into(),
            url: Some("https://example.com".into()),
            release_notes: Some("notes".into()),
        };
        assert!(info.has_release_notes());
        assert!(info.has_download_url());

        let empty_notes = UpdateInfo {
            version: "1.0.0".into(),
            product_version: "1.0.0".into(),
            url: None,
            release_notes: Some("".into()),
        };
        assert!(!empty_notes.has_release_notes());
        assert!(!empty_notes.has_download_url());
    }

    // -- UpdateService::transition_to tests ---------------------------------

    #[test]
    fn service_transition_to_valid() {
        let mut svc = UpdateService::new("1.0.0");
        assert!(svc.transition_to(UpdateState::CheckingForUpdates).is_ok());
        assert_eq!(*svc.get_state(), UpdateState::CheckingForUpdates);
        assert!(svc.transition_to(UpdateState::UpdateAvailable).is_ok());
    }

    #[test]
    fn service_transition_to_invalid() {
        let mut svc = UpdateService::new("1.0.0");
        let result = svc.transition_to(UpdateState::Ready);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            UpdateError::InvalidStateTransition { .. }
        ));
        assert_eq!(*svc.get_state(), UpdateState::Idle);
    }

    // -- UpdateService::latest_in_history tests -----------------------------

    #[test]
    fn service_latest_in_history() {
        let mut svc = UpdateService::new("1.0.0");
        assert!(svc.latest_in_history().is_none());

        svc.push_history(UpdateInfo {
            version: "1.1.0".into(),
            product_version: "1.1.0".into(),
            url: None,
            release_notes: None,
        });
        svc.push_history(UpdateInfo {
            version: "1.3.0".into(),
            product_version: "1.3.0".into(),
            url: None,
            release_notes: None,
        });
        svc.push_history(UpdateInfo {
            version: "1.2.0".into(),
            product_version: "1.2.0".into(),
            url: None,
            release_notes: None,
        });
        let latest = svc.latest_in_history().unwrap();
        assert_eq!(latest.version, "1.3.0");
    }

    // -----------------------------------------------------------------------
    // UpdateChannelSelector tests
    // -----------------------------------------------------------------------

    #[test]
    fn channel_selector_default() {
        let sel = UpdateChannelSelector::new();
        assert_eq!(*sel.current(), UpdateChannel::Stable);
        assert!(!sel.receives_previews());
    }

    #[test]
    fn channel_selector_select() {
        let mut sel = UpdateChannelSelector::new();
        sel.select(UpdateChannel::Insider).unwrap();
        assert_eq!(*sel.current(), UpdateChannel::Insider);
        assert!(sel.receives_previews());
    }

    #[test]
    fn channel_selector_disallowed() {
        let mut sel = UpdateChannelSelector::with_allowed(vec![UpdateChannel::Stable]);
        let err = sel.select(UpdateChannel::Insider);
        assert!(err.is_err());
    }

    #[test]
    fn channel_selector_cycle() {
        let mut sel = UpdateChannelSelector::with_allowed(vec![
            UpdateChannel::Stable,
            UpdateChannel::Insider,
        ]);
        assert_eq!(*sel.current(), UpdateChannel::Stable);
        sel.cycle_next();
        assert_eq!(*sel.current(), UpdateChannel::Insider);
        sel.cycle_next();
        assert_eq!(*sel.current(), UpdateChannel::Stable);
    }

    // -----------------------------------------------------------------------
    // UpdateRollbackHistory tests
    // -----------------------------------------------------------------------

    #[test]
    fn rollback_push_and_query() {
        let mut hist = UpdateRollbackHistory::new(5);
        assert!(hist.is_empty());
        hist.push(RollbackEntry::new(
            VersionParts { major: 1, minor: 0, patch: 0 },
            UpdateChannel::Stable,
            1000,
        ));
        assert_eq!(hist.len(), 1);
        assert!(!hist.is_empty());
        assert!(hist.was_rolled_back(&VersionParts { major: 1, minor: 0, patch: 0 }));
        assert!(!hist.was_rolled_back(&VersionParts { major: 2, minor: 0, patch: 0 }));
    }

    #[test]
    fn rollback_max_entries() {
        let mut hist = UpdateRollbackHistory::new(2);
        hist.push(RollbackEntry::new(VersionParts { major: 1, minor: 0, patch: 0 }, UpdateChannel::Stable, 1));
        hist.push(RollbackEntry::new(VersionParts { major: 1, minor: 1, patch: 0 }, UpdateChannel::Stable, 2));
        hist.push(RollbackEntry::new(VersionParts { major: 1, minor: 2, patch: 0 }, UpdateChannel::Stable, 3));
        assert_eq!(hist.len(), 2);
        assert!(!hist.was_rolled_back(&VersionParts { major: 1, minor: 0, patch: 0 }));
    }

    #[test]
    fn rollback_for_channel() {
        let mut hist = UpdateRollbackHistory::new(10);
        hist.push(RollbackEntry::new(VersionParts { major: 1, minor: 0, patch: 0 }, UpdateChannel::Stable, 1));
        hist.push(RollbackEntry::new(VersionParts { major: 2, minor: 0, patch: 0 }, UpdateChannel::Insider, 2));
        assert_eq!(hist.for_channel(&UpdateChannel::Stable).len(), 1);
        assert_eq!(hist.for_channel(&UpdateChannel::Insider).len(), 1);
    }

    #[test]
    fn rollback_entry_display() {
        let entry = RollbackEntry::new(
            VersionParts { major: 1, minor: 2, patch: 3 },
            UpdateChannel::Stable,
            0,
        ).with_reason("buggy release");
        let s = format!("{entry}");
        assert!(s.contains("v1.2.3"));
        assert!(s.contains("buggy release"));
    }

    // -----------------------------------------------------------------------
    // UpdateNotificationScheduler tests
    // -----------------------------------------------------------------------

    #[test]
    fn notification_immediate() {
        let mut sched = UpdateNotificationScheduler::new(NotificationPolicy::Immediate);
        sched.set_pending("2.0.0");
        assert!(sched.should_show(0));
    }

    #[test]
    fn notification_silent() {
        let mut sched = UpdateNotificationScheduler::new(NotificationPolicy::Silent);
        sched.set_pending("2.0.0");
        assert!(!sched.should_show(0));
    }

    #[test]
    fn notification_delayed() {
        let mut sched = UpdateNotificationScheduler::new(NotificationPolicy::Delayed(100));
        sched.set_pending("2.0.0");
        assert!(!sched.should_show(50));
        assert!(sched.should_show(100));
    }

    #[test]
    fn notification_dismiss() {
        let mut sched = UpdateNotificationScheduler::new(NotificationPolicy::Immediate);
        sched.set_pending("2.0.0");
        sched.dismiss();
        sched.set_pending("2.0.0");
        assert!(!sched.should_show(0));
    }

    #[test]
    fn notification_snooze() {
        let mut sched = UpdateNotificationScheduler::new(NotificationPolicy::Immediate);
        sched.set_pending("2.0.0");
        sched.snooze_until(500);
        assert!(!sched.should_show(200));
        assert!(sched.should_show(500));
    }

    // -----------------------------------------------------------------------
    // UpdateDownloadVerifier tests
    // -----------------------------------------------------------------------

    #[test]
    fn verifier_ok() {
        let v = UpdateDownloadVerifier::new()
            .expect_checksum("abc123")
            .expect_size(1024);
        assert!(v.verify("abc123", 1024).is_ok());
    }

    #[test]
    fn verifier_checksum_mismatch() {
        let v = UpdateDownloadVerifier::new().expect_checksum("abc123");
        let result = v.verify("wrong", 100);
        assert!(!result.is_ok());
        assert!(format!("{result}").contains("checksum mismatch"));
    }

    #[test]
    fn verifier_size_mismatch() {
        let v = UpdateDownloadVerifier::new().expect_size(1024);
        let result = v.verify("any", 512);
        assert!(!result.is_ok());
    }

    #[test]
    fn verifier_no_expectations() {
        let v = UpdateDownloadVerifier::new();
        assert!(v.verify("anything", 999).is_ok());
    }



    #[test]
    fn update_x_config_new() {
        let c = UpdateXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn update_x_config_builder() {
        let c = UpdateXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn update_x_config_display() {
        let c = UpdateXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn update_x_registry_insert_get() {
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn update_x_registry_duplicate() {
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("a")).unwrap();
        assert!(reg.insert(UpdateXConfig::new("a")).is_err());
    }

    #[test]
    fn update_x_registry_remove() {
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("a")).unwrap();
        reg.insert(UpdateXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn update_x_registry_active_entries() {
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("a")).unwrap();
        reg.insert(UpdateXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn update_x_registry_by_weight() {
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(UpdateXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn update_x_registry_tags() {
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(UpdateXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn update_x_registry_total_weight() {
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(UpdateXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn update_x_registry_iterator() {
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("a")).unwrap();
        reg.insert(UpdateXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn update_x_cache_put_get() {
        let mut cache = UpdateXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn update_x_cache_eviction() {
        let mut cache = UpdateXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn update_x_cache_lru_order() {
        let mut cache = UpdateXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn update_x_cache_most_least_recent() {
        let mut cache = UpdateXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn update_x_formatter_entry() {
        let e = UpdateXConfig::new("k").with_value("v");
        let fmt = UpdateXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn update_x_formatter_summary() {
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("a").with_weight(5)).unwrap();
        let fmt = UpdateXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn update_x_validator_valid() {
        let v = UpdateXValidator::new();
        let c = UpdateXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn update_x_validator_empty_key() {
        let v = UpdateXValidator::new();
        let c = UpdateXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn update_x_validator_require_value() {
        let v = UpdateXValidator::new().require_value(true);
        let c = UpdateXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn update_x_validator_allowed_tags() {
        let v = UpdateXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = UpdateXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn update_x_validator_validate_all() {
        let v = UpdateXValidator::new();
        let mut reg = UpdateXRegistry::new();
        reg.insert(UpdateXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
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


    #[test]
    fn xb_ring_buffer_47_push_and_len() {
        let mut rb = super::XbRingBuffer47::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_47_overwrite() {
        let mut rb = super::XbRingBuffer47::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_47_get_out_of_bounds() {
        let rb = super::XbRingBuffer47::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_47_drain_all() {
        let mut rb = super::XbRingBuffer47::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_47_peek_front_back() {
        let mut rb = super::XbRingBuffer47::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_47_clear() {
        let mut rb = super::XbRingBuffer47::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_47_capacity() {
        let rb = super::XbRingBuffer47::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_47_basic() {
        let h = super::xb_fnv1a_47(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_47(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_47_different_inputs() {
        let h1 = super::xb_fnv1a_47(b"abc");
        let h2 = super::xb_fnv1a_47(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_47_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_47(&data);
        let dec = super::xb_rle_decode_47(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_47_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_47(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_47(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_47_values() {
        assert!((super::xb_clamp_47(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_47(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_47(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_47_values() {
        assert!((super::xb_lerp_47(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_47(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_47(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_47_wrap_around_twice() {
        let mut rb = super::XbRingBuffer47::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 191 ----

    #[test]
    fn xc_191_pool_new_empty() {
        let pool: super::Xc191Pool<i32> = super::Xc191Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_191_pool_release_acquire() {
        let mut pool = super::Xc191Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_191_pool_acquire_empty() {
        let mut pool: super::Xc191Pool<i32> = super::Xc191Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_191_pool_full() {
        let mut pool = super::Xc191Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_191_pool_drain() {
        let mut pool = super::Xc191Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_191_pool_stats() {
        let mut pool = super::Xc191Pool::new(8);
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
    fn xc_191_pool_clear() {
        let mut pool = super::Xc191Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_191_pool_shrink() {
        let mut pool = super::Xc191Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_191_pool_default() {
        let pool: super::Xc191Pool<String> = super::Xc191Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_191_pool_extend() {
        let mut pool = super::Xc191Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_191_pool_retain() {
        let mut pool = super::Xc191Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_191_scheduler_round_robin() {
        let mut sched = super::Xc191Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_191_scheduler_empty() {
        let mut sched = super::Xc191Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_191_scheduler_reset() {
        let mut sched = super::Xc191Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_191_scheduler_add_remove() {
        let mut sched = super::Xc191Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_191_scheduler_targets() {
        let sched = super::Xc191Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_191_hash_empty() {
        assert_eq!(super::xc_191_hash(b""), 5381);
    }

    #[test]
    fn xc_191_hash_data() {
        let h = super::xc_191_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_191_hash(b"hello"), h);
    }

    #[test]
    fn xc_191_reverse_str() {
        assert_eq!(super::xc_191_reverse("abc"), "cba");
        assert_eq!(super::xc_191_reverse(""), "");
    }


    #[test]
    fn xe_60_pipeline_empty() {
        let p = super::Xe60Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_60_pipeline_parse_stage() {
        let p = super::Xe60Pipeline::new()
            .add_parse(super::xe_60_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_60_pipeline_transform_double() {
        let p = super::Xe60Pipeline::new()
            .add_transform(super::xe_60_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_60_pipeline_validate_reverse() {
        let p = super::Xe60Pipeline::new()
            .add_validate(super::xe_60_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_60_pipeline_emit_filter() {
        let p = super::Xe60Pipeline::new()
            .add_emit(super::xe_60_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_60_pipeline_multi_stage() {
        let p = super::Xe60Pipeline::new()
            .add_parse(super::xe_60_pipeline_identity)
            .add_transform(super::xe_60_pipeline_double)
            .add_validate(super::xe_60_pipeline_reverse)
            .add_emit(super::xe_60_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_60_pipeline_error_propagation() {
        let p = super::Xe60Pipeline::new()
            .add_parse(super::xe_60_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe60Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_60_pipeline_compose() {
        let p1 = super::Xe60Pipeline::new()
            .add_parse(super::xe_60_pipeline_identity);
        let p2 = super::Xe60Pipeline::new()
            .add_transform(super::xe_60_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_60_pipeline_error_display() {
        let e = super::Xe60PipelineError {
            stage: super::Xe60Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_60_cache_put_get() {
        let mut c = super::Xe60Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_60_cache_miss() {
        let mut c: super::Xe60Cache<&str, i32> = super::Xe60Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_60_cache_ttl_expiry() {
        let mut c = super::Xe60Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_60_cache_evict() {
        let mut c = super::Xe60Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_60_cache_capacity() {
        let mut c = super::Xe60Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_60_cache_stats() {
        let mut c = super::Xe60Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_60_cache_clear() {
        let mut c = super::Xe60Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_58 graph tests ------------------------------------------------

    #[test]
    fn xg_58_graph_empty() {
        let g = super::Xg58Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_58_graph_add_node() {
        let mut g = super::Xg58Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_58_graph_add_edge() {
        let mut g = super::Xg58Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_58_graph_neighbors() {
        let mut g = super::Xg58Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_58_graph_has_path() {
        let mut g = super::Xg58Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_58_graph_self_path() {
        let g = super::Xg58Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_58_graph_topo_sort() {
        let mut g = super::Xg58Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_58_graph_cycle_detect_false() {
        let mut g = super::Xg58Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_58_graph_cycle_detect_true() {
        let mut g = super::Xg58Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_58 heap tests -------------------------------------------------

    #[test]
    fn xg_58_heap_empty() {
        let h: super::Xg58Heap<i32> = super::Xg58Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_58_heap_push_pop() {
        let mut h = super::Xg58Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_58_heap_peek() {
        let mut h = super::Xg58Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_58_heap_drain_sorted() {
        let mut h = super::Xg58Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_58_heap_merge() {
        let mut a = super::Xg58Heap::new();
        let mut b = super::Xg58Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_58_heap_default() {
        let h: super::Xg58Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_58_graph_default() {
        let g: super::Xg58Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh190_skip_insert_contains() {
        let mut sl = super::Xh190SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh190_skip_remove() {
        let mut sl = super::Xh190SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh190_skip_len() {
        let mut sl = super::Xh190SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh190_skip_range_query() {
        let mut sl = super::Xh190SkipList::xh_new(4);
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
    fn xh190_skip_floor_ceiling() {
        let mut sl = super::Xh190SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh190_skip_rank() {
        let mut sl = super::Xh190SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh190_skip_empty() {
        let sl = super::Xh190SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh190_skip_duplicates() {
        let mut sl = super::Xh190SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh190_bitset_set_test() {
        let mut bs = super::Xh190BitSet::xh_new(256);
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
    fn xh190_bitset_clear_count() {
        let mut bs = super::Xh190BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh190_bitset_and_or_xor() {
        let mut a = super::Xh190BitSet::xh_new(128);
        let mut b = super::Xh190BitSet::xh_new(128);
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
    fn xh190_bitset_iter_ones() {
        let mut bs = super::Xh190BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh190_bitset_first_last() {
        let mut bs = super::Xh190BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh190_bitset_empty() {
        let bs = super::Xh190BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi190_deque_push_pop_back() {
        let mut dq = super::Xi190Deque::xi_new(4);
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
    fn xi190_deque_push_pop_front() {
        let mut dq = super::Xi190Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi190_deque_mixed_ops() {
        let mut dq = super::Xi190Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi190_deque_get_and_split() {
        let mut dq = super::Xi190Deque::xi_new(8);
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
    fn xi190_deque_rotate_left() {
        let mut dq = super::Xi190Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi190_deque_rotate_right() {
        let mut dq = super::Xi190Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi190_deque_grow() {
        let mut dq = super::Xi190Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi190_deque_empty() {
        let dq = super::Xi190Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi190_interval_tree_insert_query() {
        let mut tree = super::Xi190IntervalTree::xi_new();
        tree.xi_insert(super::Xi190Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi190Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi190Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi190_interval_tree_overlap() {
        let mut tree = super::Xi190IntervalTree::xi_new();
        tree.xi_insert(super::Xi190Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi190Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi190Interval::xi_new(12, 20));
        let q = super::Xi190Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi190_interval_tree_remove() {
        let mut tree = super::Xi190IntervalTree::xi_new();
        tree.xi_insert(super::Xi190Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi190Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi190_interval_tree_gaps() {
        let mut tree = super::Xi190IntervalTree::xi_new();
        tree.xi_insert(super::Xi190Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi190Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi190Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi190Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi190Interval::xi_new(8, 10));
    }

    #[test]
    fn xi190_interval_tree_merge() {
        let mut tree = super::Xi190IntervalTree::xi_new();
        tree.xi_insert(super::Xi190Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi190Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi190Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi190Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi190Interval::xi_new(10, 15));
    }

    #[test]
    fn xi190_interval_tree_all() {
        let mut tree = super::Xi190IntervalTree::xi_new();
        tree.xi_insert(super::Xi190Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi190Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi190_interval_tree_empty() {
        let tree = super::Xi190IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi190_interval_tree_contains_point() {
        let iv = super::Xi190Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 190) ---

    #[test]
    fn xj_190_uf_make_and_find() {
        let mut uf = super::Xj190UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_190_uf_union_connected() {
        let mut uf = super::Xj190UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_190_uf_component_count() {
        let mut uf = super::Xj190UnionFind::xj_new();
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
    fn xj_190_uf_component_size() {
        let mut uf = super::Xj190UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_190_uf_largest_component() {
        let mut uf = super::Xj190UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_190_uf_many_elements() {
        let mut uf = super::Xj190UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_190_uf_separate_components() {
        let mut uf = super::Xj190UnionFind::xj_new();
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
    fn xj_190_uf_path_compression() {
        let mut uf = super::Xj190UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_190_bt_insert_get() {
        let mut bt = super::Xj190BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_190_bt_contains_len() {
        let mut bt = super::Xj190BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_190_bt_replace() {
        let mut bt = super::Xj190BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_190_bt_remove() {
        let mut bt = super::Xj190BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_190_bt_keys_values() {
        let mut bt = super::Xj190BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_190_bt_range() {
        let mut bt = super::Xj190BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_190_bt_min_max() {
        let mut bt = super::Xj190BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_190_bt_many_inserts() {
        let mut bt = super::Xj190BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_190 segment tree tests ---

    #[test]
    fn xk_190_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk190SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_190_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk190SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_190_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk190SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_190_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk190SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_190_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk190SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_190_st_single_element() {
        let data = vec![42];
        let st = super::Xk190SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_190_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk190SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_190_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk190SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_190 disjoint intervals tests ---

    #[test]
    fn xk_190_di_add_and_count() {
        let mut di = super::Xk190DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_190_di_merge_overlap() {
        let mut di = super::Xk190DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_190_di_contains() {
        let mut di = super::Xk190DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_190_di_remove() {
        let mut di = super::Xk190DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_190_di_covered_length() {
        let mut di = super::Xk190DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_190_di_gaps() {
        let mut di = super::Xk190DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_190_di_merge_adjacent() {
        let mut di = super::Xk190DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_190_di_empty() {
        let di = super::Xk190DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_190_rope_new_empty() {
        let rope = super::Xl190Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_190_rope_from_str() {
        let rope = super::Xl190Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_190_rope_insert_at() {
        let mut rope = super::Xl190Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_190_rope_delete_range() {
        let mut rope = super::Xl190Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_190_rope_char_at() {
        let rope = super::Xl190Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_190_rope_split_concat() {
        let rope = super::Xl190Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_190_rope_line_count() {
        let rope = super::Xl190Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_190_rope_line_at() {
        let rope = super::Xl190Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_190_sa_build_and_search() {
        let sa = super::Xl190SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_190_sa_count() {
        let sa = super::Xl190SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_190_sa_longest_repeated() {
        let sa = super::Xl190SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_190_sa_all_positions() {
        let sa = super::Xl190SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_190_sa_len() {
        let sa = super::Xl190SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_190_sa_empty() {
        let sa = super::Xl190SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_190_rope_slice() {
        let rope = super::Xl190Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_190_sa_search_start() {
        let sa = super::Xl190SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_190_sparse_set_get() {
        let mut m = super::Xm190MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_190_sparse_row_col() {
        let mut m = super::Xm190MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_190_sparse_transpose() {
        let mut m = super::Xm190MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_190_sparse_multiply_vec() {
        let mut m = super::Xm190MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_190_sparse_nnz_density() {
        let mut m = super::Xm190MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_190_sparse_clear() {
        let mut m = super::Xm190MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_190_sparse_overwrite_zero() {
        let mut m = super::Xm190MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_190_tokenizer_basic() {
        let t = super::Xm190Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_190_tokenizer_count() {
        let t = super::Xm190Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_190_tokenizer_unique() {
        let t = super::Xm190Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_190_tokenizer_frequency() {
        let t = super::Xm190Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_190_tokenizer_delimiter() {
        let t = super::Xm190Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_190_tokenizer_whitespace() {
        let t = super::Xm190Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_190_tokenizer_empty() {
        let t = super::Xm190Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 190 ----

    #[test]
    fn xn_190_fenwick_prefix_sum() {
        let mut ft = super::Xn190Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_190_fenwick_range_sum() {
        let mut ft = super::Xn190Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_190_fenwick_point_query() {
        let mut ft = super::Xn190Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_190_fenwick_len() {
        let ft = super::Xn190Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_190_fenwick_multiple_updates() {
        let mut ft = super::Xn190Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_190_fenwick_single_element() {
        let mut ft = super::Xn190Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_190_fenwick_find_kth() {
        let mut ft = super::Xn190Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_190_fenwick_negative_delta() {
        let mut ft = super::Xn190Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 190 ----

    #[test]
    fn xn_190_avl_insert_get() {
        let mut m = super::Xn190AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_190_avl_remove() {
        let mut m = super::Xn190AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_190_avl_in_order() {
        let mut m = super::Xn190AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_190_avl_min_max() {
        let mut m = super::Xn190AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_190_avl_floor_ceiling() {
        let mut m = super::Xn190AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_190_avl_height_balanced() {
        let mut m = super::Xn190AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_190_avl_overwrite() {
        let mut m = super::Xn190AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_190_avl_empty() {
        let m: super::Xn190AVL<i32, i32> = super::Xn190AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo190RedBlack tests ---

    #[test]
    fn xo_190_rb_insert_and_get() {
        let mut tree = super::Xo190RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_190_rb_len_and_empty() {
        let mut tree = super::Xo190RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_190_rb_min_max() {
        let mut tree = super::Xo190RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_190_rb_contains() {
        let mut tree = super::Xo190RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_190_rb_remove() {
        let mut tree = super::Xo190RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_190_rb_in_order() {
        let mut tree = super::Xo190RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_190_rb_black_height() {
        let mut tree = super::Xo190RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_190_rb_overwrite() {
        let mut tree = super::Xo190RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo190ConsistentHash tests ---

    #[test]
    fn xo_190_ch_add_and_count() {
        let mut ring = super::Xo190ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_190_ch_remove_node() {
        let mut ring = super::Xo190ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_190_ch_get_node() {
        let mut ring = super::Xo190ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_190_ch_empty_ring() {
        let ring = super::Xo190ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_190_ch_distribution() {
        let mut ring = super::Xo190ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_190_ch_rebalance() {
        let mut ring = super::Xo190ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_190_ch_virtual_nodes() {
        let mut ring = super::Xo190ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_190_ch_consistent_lookup() {
        let mut ring = super::Xo190ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_190_splay_insert_get() {
        let mut t = super::Xp190SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_190_splay_remove() {
        let mut t = super::Xp190SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_190_splay_count_increases() {
        let mut t = super::Xp190SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_190_splay_depth() {
        let mut t = super::Xp190SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_190_splay_len_empty() {
        let t = super::Xp190SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_190_splay_min_max() {
        let mut t = super::Xp190SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_190_splay_overwrite() {
        let mut t = super::Xp190SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_190_splay_remove_missing() {
        let mut t = super::Xp190SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_190 treap tests ----
    #[test]
    fn xq_190_treap_empty() {
        let t = super::Xq190Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_190_treap_insert_get() {
        let mut t = super::Xq190Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_190_treap_overwrite() {
        let mut t = super::Xq190Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_190_treap_remove() {
        let mut t = super::Xq190Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_190_treap_min_max() {
        let mut t = super::Xq190Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_190_treap_rank() {
        let mut t = super::Xq190Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_190_treap_kth() {
        let mut t = super::Xq190Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_190_treap_in_order() {
        let mut t = super::Xq190Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_190 VEB tree tests ----
    #[test]
    fn xq_190_veb_empty() {
        let v = super::Xq190VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_190_veb_insert_contains() {
        let mut v = super::Xq190VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_190_veb_min_max() {
        let mut v = super::Xq190VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_190_veb_delete() {
        let mut v = super::Xq190VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_190_veb_successor() {
        let mut v = super::Xq190VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_190_veb_predecessor() {
        let mut v = super::Xq190VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_190_veb_count() {
        let mut v = super::Xq190VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_190_veb_duplicate_insert() {
        let mut v = super::Xq190VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_190_kdtree_empty() {
        let tree = super::Xr190KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_190_kdtree_insert_one() {
        let mut tree = super::Xr190KDTree::xr_new();
        tree.xr_insert(super::Xr190KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_190_kdtree_insert_multiple() {
        let mut tree = super::Xr190KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr190KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_190_kdtree_nearest_neighbor() {
        let mut tree = super::Xr190KDTree::xr_new();
        tree.xr_insert(super::Xr190KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr190KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr190KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_190_kdtree_nn_empty() {
        let tree = super::Xr190KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr190KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_190_kdtree_range_search() {
        let mut tree = super::Xr190KDTree::xr_new();
        tree.xr_insert(super::Xr190KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr190KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr190KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_190_kdtree_range_empty() {
        let mut tree = super::Xr190KDTree::xr_new();
        tree.xr_insert(super::Xr190KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_190_kdtree_all_points() {
        let mut tree = super::Xr190KDTree::xr_new();
        tree.xr_insert(super::Xr190KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr190KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_190_kdtree_depth() {
        let mut tree = super::Xr190KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr190KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_190_kdtree_bounding_box() {
        let mut tree = super::Xr190KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr190KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr190KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}
