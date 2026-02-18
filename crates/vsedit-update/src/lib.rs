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

}
