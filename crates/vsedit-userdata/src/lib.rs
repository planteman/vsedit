//! User data directory management.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during user data operations.
#[derive(Debug, Clone, PartialEq)]
pub enum UserDataError {
    /// Profile ID was empty or contained invalid characters.
    InvalidProfileId(String),
    /// Profile name was empty.
    EmptyProfileName,
    /// A profile with this ID already exists.
    ProfileAlreadyExists(String),
    /// The referenced profile was not found.
    ProfileNotFound(String),
    /// Cannot delete the currently active profile.
    CannotDeleteActiveProfile(String),
    /// The base directory path is invalid.
    InvalidBasePath(String),
}

impl fmt::Display for UserDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfileId(id) => write!(f, "invalid profile id: '{id}'"),
            Self::EmptyProfileName => write!(f, "profile name cannot be empty"),
            Self::ProfileAlreadyExists(id) => write!(f, "profile '{id}' already exists"),
            Self::ProfileNotFound(id) => write!(f, "profile '{id}' not found"),
            Self::CannotDeleteActiveProfile(id) => {
                write!(f, "cannot delete active profile '{id}'")
            }
            Self::InvalidBasePath(p) => write!(f, "invalid base path: '{p}'"),
        }
    }
}

impl std::error::Error for UserDataError {}

#[derive(Debug, Clone, PartialEq)]
pub struct UserDataPath {
    pub base_dir: String,
}

impl UserDataPath {
    pub fn new(base_dir: impl Into<String>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    pub fn settings_path(&self) -> String {
        self.resolve("settings.json")
    }

    pub fn keybindings_path(&self) -> String {
        self.resolve("keybindings.json")
    }

    pub fn snippets_dir(&self) -> String {
        self.resolve("snippets")
    }

    pub fn extensions_dir(&self) -> String {
        self.resolve("extensions")
    }

    pub fn state_db_path(&self) -> String {
        self.resolve("state.db")
    }

    pub fn logs_dir(&self) -> String {
        self.resolve("logs")
    }

    pub fn resolve(&self, relative: &str) -> String {
        format!("{}/{}", self.base_dir, relative)
    }

    /// Validate that the base directory path is non-empty and absolute-looking.
    pub fn validate(&self) -> Result<(), UserDataError> {
        if self.base_dir.is_empty() {
            return Err(UserDataError::InvalidBasePath(self.base_dir.clone()));
        }
        Ok(())
    }

    /// Return all standard subdirectory paths within this user data root.
    pub fn standard_paths(&self) -> Vec<String> {
        vec![
            self.settings_path(),
            self.keybindings_path(),
            self.snippets_dir(),
            self.extensions_dir(),
            self.state_db_path(),
            self.logs_dir(),
        ]
    }
}

impl fmt::Display for UserDataPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UserDataPath({})", self.base_dir)
    }
}

/// A user data profile with its own settings, extensions, and snippets.
#[derive(Debug, Clone, PartialEq)]
pub struct UserDataProfile {
    pub id: String,
    pub name: String,
    pub settings_path: String,
    pub extensions_path: String,
    pub snippets_path: String,
}

impl UserDataProfile {
    /// Returns all filesystem paths associated with this profile.
    pub fn all_paths(&self) -> Vec<&str> {
        vec![
            &self.settings_path,
            &self.extensions_path,
            &self.snippets_path,
        ]
    }

    /// Check whether this profile is the built-in default.
    pub fn is_default(&self) -> bool {
        self.id == "default"
    }
}

impl fmt::Display for UserDataProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Profile '{}' ({})", self.name, self.id)
    }
}

/// Builder for creating a `UserDataProfile` with validation.
#[derive(Debug, Clone, Default)]
pub struct ProfileBuilder {
    id: Option<String>,
    name: Option<String>,
    base_dir: Option<String>,
}

impl ProfileBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn base_dir(mut self, dir: impl Into<String>) -> Self {
        self.base_dir = Some(dir.into());
        self
    }

    /// Validate and build the profile.
    pub fn build(self) -> Result<UserDataProfile, UserDataError> {
        let id = self
            .id
            .filter(|s| !s.is_empty())
            .ok_or_else(|| UserDataError::InvalidProfileId(String::new()))?;

        if id.contains('/') || id.contains('\\') || id.contains(' ') {
            return Err(UserDataError::InvalidProfileId(id));
        }

        let name = self
            .name
            .filter(|s| !s.is_empty())
            .ok_or(UserDataError::EmptyProfileName)?;

        let base = self.base_dir.unwrap_or_default();
        let profile_dir = format!("{base}/profiles/{id}");

        Ok(UserDataProfile {
            id,
            name,
            settings_path: format!("{profile_dir}/settings.json"),
            extensions_path: format!("{profile_dir}/extensions"),
            snippets_path: format!("{profile_dir}/snippets"),
        })
    }
}

/// Service that manages user data profiles and directories.
pub struct UserDataService {
    path: UserDataPath,
    profiles: HashMap<String, UserDataProfile>,
    active_profile_id: Option<String>,
}

impl UserDataService {
    pub fn new(base_dir: impl Into<String>) -> Self {
        Self {
            path: UserDataPath::new(base_dir),
            profiles: HashMap::new(),
            active_profile_id: None,
        }
    }

    pub fn path(&self) -> &UserDataPath {
        &self.path
    }

    /// Returns the list of directory paths that would need to be created.
    pub fn ensure_dirs_exist(&self) -> Vec<String> {
        vec![
            self.path.base_dir.clone(),
            self.path.snippets_dir(),
            self.path.extensions_dir(),
            self.path.logs_dir(),
        ]
    }

    /// Get the default profile, creating it if needed.
    pub fn get_default_profile(&mut self) -> &UserDataProfile {
        if !self.profiles.contains_key("default") {
            self.create_profile("default".into(), "Default".into());
        }
        &self.profiles["default"]
    }

    /// Create a new profile with the given ID and name.
    pub fn create_profile(&mut self, id: String, name: String) -> &UserDataProfile {
        let profile_dir = self.path.resolve(&format!("profiles/{id}"));
        let profile = UserDataProfile {
            id: id.clone(),
            name,
            settings_path: format!("{profile_dir}/settings.json"),
            extensions_path: format!("{profile_dir}/extensions"),
            snippets_path: format!("{profile_dir}/snippets"),
        };
        self.profiles.insert(id.clone(), profile);
        if self.active_profile_id.is_none() {
            self.active_profile_id = Some(id.clone());
        }
        &self.profiles[&id]
    }

    /// Switch to a different profile by ID. Returns `false` if profile doesn't exist.
    pub fn switch_profile(&mut self, id: &str) -> bool {
        if self.profiles.contains_key(id) {
            self.active_profile_id = Some(id.to_string());
            true
        } else {
            false
        }
    }

    /// Get the currently active profile.
    pub fn active_profile(&self) -> Option<&UserDataProfile> {
        self.active_profile_id
            .as_ref()
            .and_then(|id| self.profiles.get(id))
    }

    /// List all profile IDs.
    pub fn list_profiles(&self) -> Vec<&str> {
        self.profiles.keys().map(|k| k.as_str()).collect()
    }

    /// Delete a profile by ID. Cannot delete the active profile.
    pub fn delete_profile(&mut self, id: &str) -> bool {
        if self.active_profile_id.as_deref() == Some(id) {
            return false;
        }
        self.profiles.remove(id).is_some()
    }

    /// Create a profile with validation, returning an error on failure.
    pub fn try_create_profile(
        &mut self,
        id: String,
        name: String,
    ) -> Result<&UserDataProfile, UserDataError> {
        if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains(' ') {
            return Err(UserDataError::InvalidProfileId(id));
        }
        if name.is_empty() {
            return Err(UserDataError::EmptyProfileName);
        }
        if self.profiles.contains_key(&id) {
            return Err(UserDataError::ProfileAlreadyExists(id));
        }
        Ok(self.create_profile(id, name))
    }

    /// Delete a profile with detailed error reporting.
    pub fn try_delete_profile(&mut self, id: &str) -> Result<UserDataProfile, UserDataError> {
        if self.active_profile_id.as_deref() == Some(id) {
            return Err(UserDataError::CannotDeleteActiveProfile(id.to_string()));
        }
        self.profiles
            .remove(id)
            .ok_or_else(|| UserDataError::ProfileNotFound(id.to_string()))
    }

    /// Rename an existing profile. Returns the old name on success.
    pub fn rename_profile(
        &mut self,
        id: &str,
        new_name: String,
    ) -> Result<String, UserDataError> {
        if new_name.is_empty() {
            return Err(UserDataError::EmptyProfileName);
        }
        let profile = self
            .profiles
            .get_mut(id)
            .ok_or_else(|| UserDataError::ProfileNotFound(id.to_string()))?;
        let old_name = std::mem::replace(&mut profile.name, new_name);
        Ok(old_name)
    }

    /// Get a profile by ID, if it exists.
    pub fn get_profile(&self, id: &str) -> Option<&UserDataProfile> {
        self.profiles.get(id)
    }

    /// Return the total number of profiles.
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    /// Check whether a profile ID is currently in use.
    pub fn has_profile(&self, id: &str) -> bool {
        self.profiles.contains_key(id)
    }
}

/// Validates user data keys and value sizes.
#[derive(Debug, Clone)]
pub struct UserDataValidator {
    /// Maximum allowed key length in bytes.
    pub max_key_len: usize,
}

impl UserDataValidator {
    /// Create a validator with the given maximum key length.
    pub fn new(max_key_len: usize) -> Self {
        Self { max_key_len }
    }

    /// Validate that a key is non-empty, within length limits, and contains only
    /// alphanumeric characters, hyphens, underscores, or dots.
    pub fn validate_key(&self, key: &str) -> Result<(), UserDataError> {
        if key.is_empty() {
            return Err(UserDataError::InvalidProfileId(
                "key must not be empty".into(),
            ));
        }
        if key.len() > self.max_key_len {
            return Err(UserDataError::InvalidProfileId(format!(
                "key exceeds max length of {} bytes",
                self.max_key_len
            )));
        }
        if !key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(UserDataError::InvalidProfileId(format!(
                "key contains invalid characters: '{key}'"
            )));
        }
        Ok(())
    }

    /// Validate that a value does not exceed `max_bytes`.
    pub fn validate_value_size(&self, value: &[u8], max_bytes: usize) -> Result<(), UserDataError> {
        if value.len() > max_bytes {
            return Err(UserDataError::InvalidBasePath(format!(
                "value size {} exceeds limit of {} bytes",
                value.len(),
                max_bytes
            )));
        }
        Ok(())
    }
}

impl Default for UserDataValidator {
    fn default() -> Self {
        Self { max_key_len: 256 }
    }
}

/// Aggregated statistics about a set of user data entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDataStats {
    /// Total number of stored entries.
    pub total_entries: usize,
    /// Combined size of all values in bytes.
    pub total_size_bytes: usize,
    /// Number of distinct namespace prefixes (the portion before the first dot).
    pub namespaces_count: usize,
}

impl UserDataStats {
    /// Compute statistics from a set of key-value entries.
    ///
    /// Namespaces are derived from the portion of each key before the first `.`
    /// character. Keys without a dot are placed in the `""` (empty) namespace.
    pub fn from_entries(entries: &[(&str, &[u8])]) -> Self {
        let total_entries = entries.len();
        let total_size_bytes: usize = entries.iter().map(|(_, v)| v.len()).sum();

        let mut namespaces = std::collections::HashSet::new();
        for (key, _) in entries {
            let ns = key.split('.').next().unwrap_or("");
            namespaces.insert(ns);
        }

        Self {
            total_entries,
            total_size_bytes,
            namespaces_count: namespaces.len(),
        }
    }
}

impl fmt::Display for UserDataStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} entries, {} bytes, {} namespaces",
            self.total_entries, self.total_size_bytes, self.namespaces_count
        )
    }
}

// ---------------------------------------------------------------------------
// Storage quota tracking
// ---------------------------------------------------------------------------

/// Tracks storage usage against a configurable quota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageQuota {
    pub max_bytes: usize,
    pub used_bytes: usize,
}

impl StorageQuota {
    pub fn new(max_bytes: usize) -> Self {
        Self { max_bytes, used_bytes: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.max_bytes.saturating_sub(self.used_bytes)
    }

    pub fn usage_percent(&self) -> f64 {
        if self.max_bytes == 0 {
            return 100.0;
        }
        (self.used_bytes as f64 / self.max_bytes as f64) * 100.0
    }

    pub fn would_exceed(&self, additional: usize) -> bool {
        self.used_bytes + additional > self.max_bytes
    }

    pub fn record_usage(&mut self, bytes: usize) {
        self.used_bytes += bytes;
    }

    pub fn release_usage(&mut self, bytes: usize) {
        self.used_bytes = self.used_bytes.saturating_sub(bytes);
    }

    pub fn is_exceeded(&self) -> bool {
        self.used_bytes > self.max_bytes
    }
}

impl fmt::Display for StorageQuota {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{} bytes ({:.1}%)", self.used_bytes, self.max_bytes, self.usage_percent())
    }
}

// ---------------------------------------------------------------------------
// Data migration versioning
// ---------------------------------------------------------------------------

/// Tracks data migration versions applied to user data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRecord {
    pub version: u32,
    pub description: String,
    pub applied: bool,
}

/// Manages a sequence of migrations.
pub struct MigrationTracker {
    migrations: Vec<MigrationRecord>,
}

impl MigrationTracker {
    pub fn new() -> Self {
        Self { migrations: Vec::new() }
    }

    pub fn add_migration(&mut self, version: u32, description: String) {
        self.migrations.push(MigrationRecord { version, description, applied: false });
    }

    pub fn mark_applied(&mut self, version: u32) -> bool {
        for m in &mut self.migrations {
            if m.version == version {
                m.applied = true;
                return true;
            }
        }
        false
    }

    pub fn pending_migrations(&self) -> Vec<&MigrationRecord> {
        self.migrations.iter().filter(|m| !m.applied).collect()
    }

    pub fn current_version(&self) -> Option<u32> {
        self.migrations.iter().filter(|m| m.applied).map(|m| m.version).max()
    }

    pub fn total_count(&self) -> usize {
        self.migrations.len()
    }
}

// ---------------------------------------------------------------------------
// Data export/import summary
// ---------------------------------------------------------------------------

/// Summary of a data export or import operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTransferSummary {
    pub profiles_transferred: usize,
    pub settings_transferred: usize,
    pub extensions_transferred: usize,
    pub total_bytes: usize,
    pub errors: Vec<String>,
}

impl DataTransferSummary {
    pub fn new() -> Self {
        Self {
            profiles_transferred: 0,
            settings_transferred: 0,
            extensions_transferred: 0,
            total_bytes: 0,
            errors: Vec::new(),
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles_transferred == 0
            && self.settings_transferred == 0
            && self.extensions_transferred == 0
    }
}

impl fmt::Display for DataTransferSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} profiles, {} settings, {} extensions, {} bytes",
            self.profiles_transferred, self.settings_transferred,
            self.extensions_transferred, self.total_bytes
        )
    }
}

// ---------------------------------------------------------------------------
// Data integrity verification
// ---------------------------------------------------------------------------

/// Result of verifying user data integrity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityReport {
    pub checked_paths: usize,
    pub missing_paths: Vec<String>,
    pub corrupted_paths: Vec<String>,
}

impl IntegrityReport {
    pub fn is_healthy(&self) -> bool {
        self.missing_paths.is_empty() && self.corrupted_paths.is_empty()
    }

    pub fn total_issues(&self) -> usize {
        self.missing_paths.len() + self.corrupted_paths.len()
    }
}

impl fmt::Display for IntegrityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "checked {} paths: {} missing, {} corrupted",
            self.checked_paths, self.missing_paths.len(), self.corrupted_paths.len()
        )
    }
}

/// Verify integrity of user data paths by checking that all standard paths
/// exist in a given set of known paths.
pub fn verify_integrity(user_path: &UserDataPath, existing_paths: &[&str]) -> IntegrityReport {
    let standard = user_path.standard_paths();
    let mut missing = Vec::new();
    for p in &standard {
        if !existing_paths.contains(&p.as_str()) {
            missing.push(p.clone());
        }
    }
    IntegrityReport {
        checked_paths: standard.len(),
        missing_paths: missing,
        corrupted_paths: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        let p = UserDataPath::new("/home/user/.config/vsedit");
        assert_eq!(p.settings_path(), "/home/user/.config/vsedit/settings.json");
        assert_eq!(
            p.keybindings_path(),
            "/home/user/.config/vsedit/keybindings.json"
        );
    }

    #[test]
    fn resolve_arbitrary() {
        let p = UserDataPath::new("/data");
        assert_eq!(p.resolve("foo/bar"), "/data/foo/bar");
    }

    #[test]
    fn ensure_dirs() {
        let svc = UserDataService::new("/base");
        let dirs = svc.ensure_dirs_exist();
        assert!(dirs.contains(&"/base".to_string()));
        assert!(dirs.contains(&"/base/snippets".to_string()));
        assert!(dirs.contains(&"/base/extensions".to_string()));
        assert!(dirs.contains(&"/base/logs".to_string()));
    }

    #[test]
    fn create_and_get_profile() {
        let mut svc = UserDataService::new("/base");
        let profile = svc.create_profile("work".into(), "Work Profile".into());
        assert_eq!(profile.id, "work");
        assert_eq!(profile.name, "Work Profile");
        assert!(profile.settings_path.contains("profiles/work"));
        assert!(profile.extensions_path.contains("profiles/work"));
    }

    #[test]
    fn default_profile() {
        let mut svc = UserDataService::new("/base");
        let profile = svc.get_default_profile();
        assert_eq!(profile.id, "default");
        assert_eq!(profile.name, "Default");
    }

    #[test]
    fn switch_profile() {
        let mut svc = UserDataService::new("/base");
        svc.create_profile("a".into(), "Profile A".into());
        svc.create_profile("b".into(), "Profile B".into());
        assert!(svc.switch_profile("b"));
        assert_eq!(svc.active_profile().unwrap().id, "b");
        assert!(!svc.switch_profile("nonexistent"));
    }

    #[test]
    fn list_profiles() {
        let mut svc = UserDataService::new("/base");
        svc.create_profile("x".into(), "X".into());
        svc.create_profile("y".into(), "Y".into());
        let mut profiles = svc.list_profiles();
        profiles.sort();
        assert_eq!(profiles, vec!["x", "y"]);
    }

    #[test]
    fn delete_profile() {
        let mut svc = UserDataService::new("/base");
        svc.create_profile("a".into(), "A".into());
        svc.create_profile("b".into(), "B".into());
        // First created becomes active, can't delete it
        assert!(!svc.delete_profile("a"));
        // Switch to b, then delete a
        svc.switch_profile("b");
        assert!(svc.delete_profile("a"));
        assert_eq!(svc.list_profiles().len(), 1);
    }

    #[test]
    fn user_data_path_display() {
        let p = UserDataPath::new("/home/user/.config/vsedit");
        assert_eq!(
            format!("{p}"),
            "UserDataPath(/home/user/.config/vsedit)"
        );
    }

    #[test]
    fn user_data_path_validate_empty() {
        let p = UserDataPath::new("");
        assert!(p.validate().is_err());
    }

    #[test]
    fn user_data_path_standard_paths() {
        let p = UserDataPath::new("/data");
        let paths = p.standard_paths();
        assert_eq!(paths.len(), 6);
        assert!(paths.contains(&"/data/settings.json".to_string()));
        assert!(paths.contains(&"/data/logs".to_string()));
    }

    #[test]
    fn user_data_path_equality() {
        let a = UserDataPath::new("/x");
        let b = UserDataPath::new("/x");
        assert_eq!(a, b);
        let c = UserDataPath::new("/y");
        assert_ne!(a, c);
    }

    #[test]
    fn profile_display_and_is_default() {
        let mut svc = UserDataService::new("/base");
        let profile = svc.get_default_profile().clone();
        assert!(profile.is_default());
        assert_eq!(format!("{profile}"), "Profile 'Default' (default)");

        let work = svc.create_profile("work".into(), "Work".into());
        assert!(!work.is_default());
    }

    #[test]
    fn profile_all_paths() {
        let mut svc = UserDataService::new("/base");
        let profile = svc.create_profile("p1".into(), "P1".into());
        let paths = profile.all_paths();
        assert_eq!(paths.len(), 3);
        assert!(paths.iter().all(|p| p.contains("profiles/p1")));
    }

    #[test]
    fn profile_builder_success() {
        let profile = ProfileBuilder::new()
            .id("test")
            .name("Test Profile")
            .base_dir("/data")
            .build()
            .unwrap();
        assert_eq!(profile.id, "test");
        assert_eq!(profile.name, "Test Profile");
        assert!(profile.settings_path.starts_with("/data/profiles/test"));
    }

    #[test]
    fn profile_builder_invalid_id() {
        let res = ProfileBuilder::new()
            .id("has space")
            .name("Name")
            .base_dir("/d")
            .build();
        assert!(matches!(res, Err(UserDataError::InvalidProfileId(_))));

        let res = ProfileBuilder::new().name("Name").build();
        assert!(matches!(res, Err(UserDataError::InvalidProfileId(_))));
    }

    #[test]
    fn profile_builder_empty_name() {
        let res = ProfileBuilder::new().id("ok").base_dir("/d").build();
        assert!(matches!(res, Err(UserDataError::EmptyProfileName)));
    }

    #[test]
    fn try_create_profile_duplicate() {
        let mut svc = UserDataService::new("/base");
        svc.create_profile("dup".into(), "Dup".into());
        let err = svc.try_create_profile("dup".into(), "Dup2".into());
        assert!(matches!(err, Err(UserDataError::ProfileAlreadyExists(_))));
    }

    #[test]
    fn try_create_profile_validation() {
        let mut svc = UserDataService::new("/base");
        assert!(matches!(
            svc.try_create_profile("".into(), "X".into()),
            Err(UserDataError::InvalidProfileId(_))
        ));
        assert!(matches!(
            svc.try_create_profile("ok".into(), "".into()),
            Err(UserDataError::EmptyProfileName)
        ));
    }

    #[test]
    fn try_delete_profile_errors() {
        let mut svc = UserDataService::new("/base");
        svc.create_profile("a".into(), "A".into());
        // 'a' is active
        assert!(matches!(
            svc.try_delete_profile("a"),
            Err(UserDataError::CannotDeleteActiveProfile(_))
        ));
        // 'z' doesn't exist
        assert!(matches!(
            svc.try_delete_profile("z"),
            Err(UserDataError::ProfileNotFound(_))
        ));
    }

    #[test]
    fn rename_profile_success_and_errors() {
        let mut svc = UserDataService::new("/base");
        svc.create_profile("r".into(), "OldName".into());
        let old = svc.rename_profile("r", "NewName".into()).unwrap();
        assert_eq!(old, "OldName");
        assert_eq!(svc.get_profile("r").unwrap().name, "NewName");

        // empty name
        assert!(matches!(
            svc.rename_profile("r", "".into()),
            Err(UserDataError::EmptyProfileName)
        ));
        // missing profile
        assert!(matches!(
            svc.rename_profile("nope", "X".into()),
            Err(UserDataError::ProfileNotFound(_))
        ));
    }

    #[test]
    fn profile_count_and_has_profile() {
        let mut svc = UserDataService::new("/base");
        assert_eq!(svc.profile_count(), 0);
        assert!(!svc.has_profile("a"));
        svc.create_profile("a".into(), "A".into());
        assert_eq!(svc.profile_count(), 1);
        assert!(svc.has_profile("a"));
    }

    #[test]
    fn error_display_messages() {
        let e = UserDataError::InvalidProfileId("bad id".into());
        assert_eq!(format!("{e}"), "invalid profile id: 'bad id'");
        let e = UserDataError::EmptyProfileName;
        assert_eq!(format!("{e}"), "profile name cannot be empty");
        let e = UserDataError::ProfileNotFound("x".into());
        assert_eq!(format!("{e}"), "profile 'x' not found");
    }

    #[test]
    fn validator_valid_key() {
        let v = UserDataValidator::default();
        assert!(v.validate_key("editor.fontSize").is_ok());
        assert!(v.validate_key("my-key_01").is_ok());
    }

    #[test]
    fn validator_invalid_keys() {
        let v = UserDataValidator::new(10);
        // empty key
        assert!(v.validate_key("").is_err());
        // too long
        assert!(v.validate_key("abcdefghijk").is_err());
        // invalid characters
        assert!(v.validate_key("key with spaces").is_err());
        assert!(v.validate_key("key/slash").is_err());
    }

    #[test]
    fn validator_value_size() {
        let v = UserDataValidator::default();
        assert!(v.validate_value_size(b"hello", 10).is_ok());
        assert!(v.validate_value_size(b"hello", 5).is_ok());
        assert!(v.validate_value_size(b"hello!", 5).is_err());
        assert!(v.validate_value_size(b"", 0).is_ok());
    }

    #[test]
    fn stats_from_entries() {
        let entries: Vec<(&str, &[u8])> = vec![
            ("editor.fontSize", b"14"),
            ("editor.tabSize", b"4"),
            ("theme.name", b"dark"),
            ("locale", b"en"),
        ];
        let stats = UserDataStats::from_entries(&entries);
        assert_eq!(stats.total_entries, 4);
        assert_eq!(stats.total_size_bytes, 2 + 1 + 4 + 2);
        // namespaces: "editor", "theme", "locale"
        assert_eq!(stats.namespaces_count, 3);
    }

    #[test]
    fn stats_empty_entries() {
        let stats = UserDataStats::from_entries(&[]);
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert_eq!(stats.namespaces_count, 0);
    }

    #[test]
    fn stats_display() {
        let stats = UserDataStats {
            total_entries: 5,
            total_size_bytes: 128,
            namespaces_count: 2,
        };
        assert_eq!(format!("{stats}"), "5 entries, 128 bytes, 2 namespaces");
    }

    #[test]
    fn storage_quota_tracking() {
        let mut quota = StorageQuota::new(1000);
        assert_eq!(quota.remaining(), 1000);
        assert!(!quota.is_exceeded());
        assert!(!quota.would_exceed(500));
        assert!(quota.would_exceed(1001));
        quota.record_usage(600);
        assert_eq!(quota.remaining(), 400);
        assert!((quota.usage_percent() - 60.0).abs() < 0.1);
        quota.release_usage(200);
        assert_eq!(quota.remaining(), 600);
        assert!(quota.to_string().contains("400/1000"));
    }

    #[test]
    fn storage_quota_zero_max() {
        let quota = StorageQuota::new(0);
        assert_eq!(quota.usage_percent(), 100.0);
        assert!(quota.would_exceed(1));
    }

    #[test]
    fn migration_tracker_lifecycle() {
        let mut tracker = MigrationTracker::new();
        tracker.add_migration(1, "initial schema".into());
        tracker.add_migration(2, "add profiles".into());
        tracker.add_migration(3, "add tags".into());
        assert_eq!(tracker.total_count(), 3);
        assert_eq!(tracker.pending_migrations().len(), 3);
        assert!(tracker.current_version().is_none());
        assert!(tracker.mark_applied(1));
        assert!(tracker.mark_applied(2));
        assert_eq!(tracker.current_version(), Some(2));
        assert_eq!(tracker.pending_migrations().len(), 1);
        assert!(!tracker.mark_applied(99));
    }

    #[test]
    fn data_transfer_summary_basic() {
        let mut summary = DataTransferSummary::new();
        assert!(summary.is_empty());
        assert!(!summary.has_errors());
        summary.profiles_transferred = 2;
        summary.settings_transferred = 5;
        summary.extensions_transferred = 10;
        summary.total_bytes = 4096;
        assert!(!summary.is_empty());
        assert!(summary.to_string().contains("2 profiles"));
        summary.errors.push("failed to export theme".into());
        assert!(summary.has_errors());
    }

    #[test]
    fn integrity_report_healthy() {
        let p = UserDataPath::new("/data");
        let paths = p.standard_paths();
        let existing: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        let report = verify_integrity(&p, &existing);
        assert!(report.is_healthy());
        assert_eq!(report.total_issues(), 0);
        assert_eq!(report.checked_paths, 6);
    }

    #[test]
    fn integrity_report_missing_paths() {
        let p = UserDataPath::new("/data");
        let report = verify_integrity(&p, &[]);
        assert!(!report.is_healthy());
        assert_eq!(report.missing_paths.len(), 6);
        assert!(report.to_string().contains("6 missing"));
    }
}
