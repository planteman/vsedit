//! Update mechanism.

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    CheckingForUpdates,
    UpdateAvailable,
    Downloading,
    Ready,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub product_version: String,
    pub url: Option<String>,
    pub release_notes: Option<String>,
}

pub struct UpdateService {
    state: UpdateState,
    current_version: String,
    available_update: Option<UpdateInfo>,
}

impl UpdateService {
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            state: UpdateState::Idle,
            current_version: current_version.into(),
            available_update: None,
        }
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
        latest_version != self.current_version
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
}
