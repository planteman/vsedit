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


}
