//! Update mechanism.

use std::fmt;
use std::error;

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateChannel {
    Stable,
    Insider,
    Exploration,
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
}

impl UpdateService {
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            state: UpdateState::Idle,
            current_version: current_version.into(),
            available_update: None,
            channel: UpdateChannel::Stable,
            progress: None,
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
}
