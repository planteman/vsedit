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

// ---------------------------------------------------------------------------
// Settings import/export
// ---------------------------------------------------------------------------

/// Describes settings to export from user data.
#[derive(Debug, Clone, PartialEq)]
pub struct UserDataExport {
    pub version: u32,
    pub profiles: Vec<ExportedProfile>,
    pub global_settings: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportedProfile {
    pub id: String,
    pub name: String,
    pub settings: HashMap<String, String>,
}

impl UserDataExport {
    pub fn new(version: u32) -> Self {
        Self {
            version,
            profiles: Vec::new(),
            global_settings: HashMap::new(),
        }
    }

    pub fn add_profile(&mut self, profile: ExportedProfile) {
        self.profiles.push(profile);
    }

    pub fn add_global_setting(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.global_settings.insert(key.into(), value.into());
    }

    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty() && self.global_settings.is_empty()
    }

    pub fn total_settings_count(&self) -> usize {
        let profile_settings: usize = self.profiles.iter().map(|p| p.settings.len()).sum();
        profile_settings + self.global_settings.len()
    }
}

/// Describes settings to import into user data.
#[derive(Debug, Clone, PartialEq)]
pub struct UserDataImport {
    pub version: u32,
    pub profiles: Vec<ExportedProfile>,
    pub global_settings: HashMap<String, String>,
    pub merge_strategy: ImportMergeStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMergeStrategy {
    Replace,
    MergeKeepLocal,
    MergeKeepRemote,
}

impl UserDataImport {
    pub fn from_export(export: UserDataExport, strategy: ImportMergeStrategy) -> Self {
        Self {
            version: export.version,
            profiles: export.profiles,
            global_settings: export.global_settings,
            merge_strategy: strategy,
        }
    }

    pub fn validate(&self) -> Result<(), UserDataError> {
        if self.version == 0 {
            return Err(UserDataError::InvalidBasePath(
                "import version must be greater than 0".into(),
            ));
        }
        if self.profiles.is_empty() && self.global_settings.is_empty() {
            return Err(UserDataError::InvalidBasePath(
                "import must contain at least one profile or global setting".into(),
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Backup descriptor
// ---------------------------------------------------------------------------

/// Descriptor for a backup archive of user data.
#[derive(Debug, Clone, PartialEq)]
pub struct UserDataBackup {
    pub timestamp: u64,
    pub paths_included: Vec<String>,
    pub total_size_bytes: usize,
    pub profile_ids: Vec<String>,
    pub backup_label: Option<String>,
}

impl UserDataBackup {
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.backup_label = Some(label.into());
        self
    }

    pub fn includes_profile(&self, id: &str) -> bool {
        self.profile_ids.iter().any(|p| p == id)
    }

    pub fn path_count(&self) -> usize {
        self.paths_included.len()
    }
}

/// Create a backup descriptor from a `UserDataService`, capturing all profile paths.
pub fn user_data_backup(service: &UserDataService, timestamp: u64) -> UserDataBackup {
    let profile_ids: Vec<String> = service.list_profiles().iter().map(|s| s.to_string()).collect();
    let mut paths_included = service.ensure_dirs_exist();
    let mut total_size_bytes = 0;
    for id in &profile_ids {
        if let Some(profile) = service.get_profile(id) {
            for p in profile.all_paths() {
                paths_included.push(p.to_string());
                total_size_bytes += p.len();
            }
        }
    }
    UserDataBackup {
        timestamp,
        paths_included,
        total_size_bytes,
        profile_ids,
        backup_label: None,
    }
}

// ---------------------------------------------------------------------------
// Platform-specific path resolution
// ---------------------------------------------------------------------------

/// Represents platform-specific locations for user data.
#[derive(Debug, Clone, PartialEq)]
pub struct UserDataLocation {
    pub config_dir: String,
    pub data_dir: String,
    pub cache_dir: String,
    pub log_dir: String,
}

impl UserDataLocation {
    pub fn new(
        config: impl Into<String>,
        data: impl Into<String>,
        cache: impl Into<String>,
        log: impl Into<String>,
    ) -> Self {
        Self {
            config_dir: config.into(),
            data_dir: data.into(),
            cache_dir: cache.into(),
            log_dir: log.into(),
        }
    }

    pub fn for_linux(app_name: &str) -> Self {
        Self {
            config_dir: format!("~/.config/{app_name}"),
            data_dir: format!("~/.local/share/{app_name}"),
            cache_dir: format!("~/.cache/{app_name}"),
            log_dir: format!("~/.local/share/{app_name}/logs"),
        }
    }

    pub fn for_macos(app_name: &str) -> Self {
        Self {
            config_dir: format!("~/Library/Application Support/{app_name}"),
            data_dir: format!("~/Library/Application Support/{app_name}"),
            cache_dir: format!("~/Library/Caches/{app_name}"),
            log_dir: format!("~/Library/Logs/{app_name}"),
        }
    }

    pub fn for_windows(app_name: &str) -> Self {
        Self {
            config_dir: format!("%APPDATA%/{app_name}"),
            data_dir: format!("%APPDATA%/{app_name}"),
            cache_dir: format!("%APPDATA%/{app_name}/cache"),
            log_dir: format!("%APPDATA%/{app_name}/logs"),
        }
    }

    pub fn resolve(&self, relative: &str) -> String {
        format!("{}/{relative}", self.config_dir)
    }

    pub fn all_dirs(&self) -> Vec<&str> {
        vec![&self.config_dir, &self.data_dir, &self.cache_dir, &self.log_dir]
    }
}


// ---------------------------------------------------------------------------
// UserPreferences - key-value user preferences store
// ---------------------------------------------------------------------------

/// A key-value store for user preferences.
#[derive(Debug, Clone, Default)]
pub struct UserPreferences {
    values: HashMap<String, String>,
}

impl UserPreferences {
    /// Create an empty preferences store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a preference value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    /// Set a preference value. Returns the previous value if any.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.values.insert(key.into(), value.into())
    }

    /// Remove a preference by key. Returns the removed value if any.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.values.remove(key)
    }

    /// List all preference keys.
    pub fn keys(&self) -> Vec<&str> {
        self.values.keys().map(|s| s.as_str()).collect()
    }

    /// Number of stored preferences.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns true if no preferences are stored.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Check if a key exists.
    pub fn has_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// Get a value with a default fallback.
    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.values
            .get(key)
            .map(|s| s.clone())
            .unwrap_or_else(|| default.to_string())
    }

    /// Merge another set of preferences, overwriting existing keys.
    pub fn merge(&mut self, other: &UserPreferences) {
        for (k, v) in &other.values {
            self.values.insert(k.clone(), v.clone());
        }
    }
}

impl fmt::Display for UserPreferences {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UserPreferences({} keys)", self.len())
    }
}

// ---------------------------------------------------------------------------
// PreferencesMigration - upgrade preferences across versions
// ---------------------------------------------------------------------------

/// Describes a migration rule for upgrading preferences between versions.
#[derive(Debug, Clone)]
pub struct PreferencesMigration {
    pub from_version: u32,
    pub to_version: u32,
    renames: Vec<(String, String)>,
    removals: Vec<String>,
    defaults: Vec<(String, String)>,
}

impl PreferencesMigration {
    /// Create a new migration rule.
    pub fn new(from_version: u32, to_version: u32) -> Self {
        Self {
            from_version,
            to_version,
            renames: Vec::new(),
            removals: Vec::new(),
            defaults: Vec::new(),
        }
    }

    /// Add a key rename.
    pub fn rename(mut self, old_key: impl Into<String>, new_key: impl Into<String>) -> Self {
        self.renames.push((old_key.into(), new_key.into()));
        self
    }

    /// Add a key removal.
    pub fn remove_key(mut self, key: impl Into<String>) -> Self {
        self.removals.push(key.into());
        self
    }

    /// Add a default value for a new key.
    pub fn add_default(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.defaults.push((key.into(), value.into()));
        self
    }

    /// Apply this migration to a set of preferences.
    pub fn migrate(&self, prefs: &mut UserPreferences) {
        for (old, new) in &self.renames {
            if let Some(val) = prefs.remove(old) {
                prefs.set(new.clone(), val);
            }
        }
        for key in &self.removals {
            prefs.remove(key);
        }
        for (key, value) in &self.defaults {
            if !prefs.has_key(key) {
                prefs.set(key.clone(), value.clone());
            }
        }
    }

    /// Number of operations in this migration.
    pub fn operation_count(&self) -> usize {
        self.renames.len() + self.removals.len() + self.defaults.len()
    }
}

impl fmt::Display for PreferencesMigration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Migration(v{} -> v{}, {} ops)",
            self.from_version, self.to_version, self.operation_count()
        )
    }
}

// ---------------------------------------------------------------------------
// UserDataExporter - backup/restore user data
// ---------------------------------------------------------------------------

/// Exports and imports user preferences as a simple key=value string format.
#[derive(Debug, Clone, Default)]
pub struct UserDataExporter;

impl UserDataExporter {
    /// Export preferences to a string.
    pub fn export_to_string(prefs: &UserPreferences) -> String {
        let mut lines: Vec<String> = prefs
            .keys()
            .iter()
            .map(|k| {
                let v = prefs.get(k).unwrap_or("");
                format!("{k}={v}")
            })
            .collect();
        lines.sort();
        lines.join("\n")
    }

    /// Import preferences from a string.
    pub fn import_from_string(data: &str) -> UserPreferences {
        let mut prefs = UserPreferences::new();
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                prefs.set(key.trim(), value.trim());
            }
        }
        prefs
    }
}

// ---------------------------------------------------------------------------
// DataIntegrityChecker - compute and verify checksums
// ---------------------------------------------------------------------------

/// Computes and verifies simple checksums on data strings.
#[derive(Debug, Clone, Default)]
pub struct DataIntegrityChecker;

impl DataIntegrityChecker {
    /// Compute a simple checksum (sum of bytes mod 2^32).
    pub fn compute_checksum(data: &str) -> u32 {
        data.as_bytes().iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32))
    }

    /// Verify that data matches an expected checksum.
    pub fn verify(data: &str, expected: u32) -> bool {
        Self::compute_checksum(data) == expected
    }
}

// ---------------------------------------------------------------------------
// UserDataPath utilities – additional path resolution helpers
// ---------------------------------------------------------------------------

impl UserDataPath {
    /// Return the path for a workspace-specific storage directory.
    pub fn workspace_storage_path(&self, workspace_id: &str) -> String {
        self.resolve(&format!("workspaceStorage/{workspace_id}"))
    }

    /// Return the path for a global storage directory.
    pub fn global_storage_path(&self) -> String {
        self.resolve("globalStorage")
    }

    /// Return the path for cached data (e.g. downloaded extensions).
    pub fn cache_path(&self) -> String {
        self.resolve("CachedData")
    }

    /// Return the path for crash reports.
    pub fn crash_reports_path(&self) -> String {
        self.resolve("crash-reports")
    }

    /// Check if a relative sub-path is contained in the standard paths.
    pub fn is_standard_subpath(&self, path: &str) -> bool {
        self.standard_paths().iter().any(|p| p.ends_with(path))
    }
}

/// Classify a file path as belonging to a particular user data category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserDataCategory {
    Settings,
    Keybindings,
    Snippets,
    Extensions,
    Logs,
    StateDb,
    WorkspaceStorage,
    GlobalStorage,
    CachedData,
    Unknown,
}

impl fmt::Display for UserDataCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Settings => "settings",
            Self::Keybindings => "keybindings",
            Self::Snippets => "snippets",
            Self::Extensions => "extensions",
            Self::Logs => "logs",
            Self::StateDb => "state-db",
            Self::WorkspaceStorage => "workspace-storage",
            Self::GlobalStorage => "global-storage",
            Self::CachedData => "cached-data",
            Self::Unknown => "unknown",
        };
        write!(f, "{label}")
    }
}

/// Classify a path within the user data directory.
pub fn classify_user_data_path(path: &str) -> UserDataCategory {
    if path.contains("settings.json") {
        UserDataCategory::Settings
    } else if path.contains("keybindings.json") {
        UserDataCategory::Keybindings
    } else if path.contains("snippets") {
        UserDataCategory::Snippets
    } else if path.contains("extensions") {
        UserDataCategory::Extensions
    } else if path.contains("logs") {
        UserDataCategory::Logs
    } else if path.contains("state.db") {
        UserDataCategory::StateDb
    } else if path.contains("workspaceStorage") {
        UserDataCategory::WorkspaceStorage
    } else if path.contains("globalStorage") {
        UserDataCategory::GlobalStorage
    } else if path.contains("CachedData") {
        UserDataCategory::CachedData
    } else {
        UserDataCategory::Unknown
    }
}

/// Validate a profile ID string. Must be non-empty, alphanumeric/dash/underscore only.
pub fn validate_profile_id(id: &str) -> Result<(), UserDataError> {
    if id.is_empty() {
        return Err(UserDataError::InvalidProfileId(id.to_string()));
    }
    if !id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(UserDataError::InvalidProfileId(id.to_string()));
    }
    Ok(())
}
// -- UserDataSync with conflict resolution -----------------------------------

/// Conflict resolution strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    LocalWins,
    RemoteWins,
    MostRecent,
}

impl fmt::Display for ConflictStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictStrategy::LocalWins => f.write_str("local-wins"),
            ConflictStrategy::RemoteWins => f.write_str("remote-wins"),
            ConflictStrategy::MostRecent => f.write_str("most-recent"),
        }
    }
}

/// Represents a sync conflict between local and remote data.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncConflict {
    pub key: String,
    pub local_value: String,
    pub remote_value: String,
    pub local_timestamp: u64,
    pub remote_timestamp: u64,
}

impl SyncConflict {
    /// Resolve using the given strategy.
    pub fn resolve(&self, strategy: ConflictStrategy) -> &str {
        match strategy {
            ConflictStrategy::LocalWins => &self.local_value,
            ConflictStrategy::RemoteWins => &self.remote_value,
            ConflictStrategy::MostRecent => {
                if self.local_timestamp >= self.remote_timestamp {
                    &self.local_value
                } else {
                    &self.remote_value
                }
            }
        }
    }
}

impl fmt::Display for SyncConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Conflict(key={}, local_ts={}, remote_ts={})", self.key, self.local_timestamp, self.remote_timestamp)
    }
}

// -- UserDataExport to archive format ----------------------------------------

/// Represents a single file entry for export.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportEntry {
    pub relative_path: String,
    pub content: String,
    pub size_bytes: usize,
}

/// An export manifest describing what was exported.
#[derive(Debug, Clone)]
pub struct ExportManifest {
    pub profile_id: String,
    pub entries: Vec<ExportEntry>,
    pub total_size: usize,
}

impl ExportManifest {
    pub fn new(profile_id: &str) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            entries: Vec::new(),
            total_size: 0,
        }
    }

    pub fn add_entry(&mut self, path: &str, content: &str) {
        let size = content.len();
        self.entries.push(ExportEntry {
            relative_path: path.to_string(),
            content: content.to_string(),
            size_bytes: size,
        });
        self.total_size += size;
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl fmt::Display for ExportManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Export(profile={}, {} files, {} bytes)", self.profile_id, self.entries.len(), self.total_size)
    }
}

// -- UserDataImport with validation ------------------------------------------

/// Validation result for an import entry.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportValidation {
    Valid,
    InvalidPath(String),
    TooLarge { path: String, size: usize, max: usize },
    DuplicatePath(String),
}

impl fmt::Display for ImportValidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportValidation::Valid => f.write_str("valid"),
            ImportValidation::InvalidPath(p) => write!(f, "invalid path: {p}"),
            ImportValidation::TooLarge { path, size, max } => {
                write!(f, "{path}: {size} bytes exceeds limit of {max}")
            }
            ImportValidation::DuplicatePath(p) => write!(f, "duplicate path: {p}"),
        }
    }
}

/// Validate import entries.
pub fn validate_import(entries: &[ExportEntry], max_size: usize) -> Vec<ImportValidation> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        if entry.relative_path.is_empty() || entry.relative_path.contains("..") {
            results.push(ImportValidation::InvalidPath(entry.relative_path.clone()));
        } else if entry.size_bytes > max_size {
            results.push(ImportValidation::TooLarge {
                path: entry.relative_path.clone(),
                size: entry.size_bytes,
                max: max_size,
            });
        } else if !seen.insert(&entry.relative_path) {
            results.push(ImportValidation::DuplicatePath(entry.relative_path.clone()));
        }
    }
    results
}

// -- User data size tracking -------------------------------------------------

/// Track cumulative size of user data items.
#[derive(Debug, Default)]
pub struct UserDataSizeTracker {
    items: HashMap<String, usize>,
}

impl UserDataSizeTracker {
    pub fn new() -> Self {
        Self { items: HashMap::new() }
    }

    pub fn record(&mut self, key: &str, size: usize) {
        self.items.insert(key.to_string(), size);
    }

    pub fn remove(&mut self, key: &str) {
        self.items.remove(key);
    }

    pub fn total_size(&self) -> usize {
        self.items.values().sum()
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn largest_item(&self) -> Option<(&str, usize)> {
        self.items.iter().max_by_key(|&(_, &v)| v).map(|(k, &v)| (k.as_str(), v))
    }

    /// Return keys exceeding the given size limit.
    pub fn items_exceeding(&self, limit: usize) -> Vec<&str> {
        self.items.iter().filter(|&(_, &v)| v > limit).map(|(k, _)| k.as_str()).collect()
    }
}

impl fmt::Display for UserDataSizeTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SizeTracker({} items, {} bytes total)", self.item_count(), self.total_size())
    }
}


// === Userdata Import Wizard ===

/// Userdata Import Wizard implementation.
#[derive(Debug, Clone)]
pub struct UserdataImportWizard {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: UserdataImportWizardStats,
}

/// Statistics for UserdataImportWizard.
#[derive(Debug, Clone, Default)]
pub struct UserdataImportWizardStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl UserdataImportWizardStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl UserdataImportWizard {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: UserdataImportWizardStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &UserdataImportWizardStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for UserdataImportWizard {
    fn default() -> Self {
        Self::new()
    }
}

// === Userdata Size Calculator ===

/// Priority level for UserdataSizeCalculator items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UserdataSizeCalculatorPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl UserdataSizeCalculatorPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for UserdataSizeCalculatorPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Userdata Size Calculator implementation.
#[derive(Debug, Clone)]
pub struct UserdataSizeCalculator {
    items: Vec<UserdataSizeCalculatorItem>,
    max_items: usize,
    default_priority: UserdataSizeCalculatorPriority,
}

/// A single item in UserdataSizeCalculator.
#[derive(Debug, Clone)]
pub struct UserdataSizeCalculatorItem {
    pub id: String,
    pub label: String,
    pub priority: UserdataSizeCalculatorPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl UserdataSizeCalculatorItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: UserdataSizeCalculatorPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: UserdataSizeCalculatorPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl UserdataSizeCalculator {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: UserdataSizeCalculatorPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: UserdataSizeCalculatorItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<UserdataSizeCalculatorItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&UserdataSizeCalculatorItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: UserdataSizeCalculatorPriority) -> Vec<&UserdataSizeCalculatorItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&UserdataSizeCalculatorItem> {
        let mut sorted: Vec<&UserdataSizeCalculatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&UserdataSizeCalculatorItem> {
        let mut sorted: Vec<&UserdataSizeCalculatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&UserdataSizeCalculatorItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: UserdataSizeCalculatorPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> UserdataSizeCalculatorPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &UserdataSizeCalculatorItem> {
        self.items.iter()
    }
}

impl Default for UserdataSizeCalculator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-userdata: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserdataXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl UserdataXConfig {
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

impl std::fmt::Display for UserdataXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct UserdataXRegistry {
    entries: Vec<UserdataXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl UserdataXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: UserdataXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&UserdataXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut UserdataXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<UserdataXConfig> {
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

    pub fn active_entries(&self) -> Vec<&UserdataXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&UserdataXConfig> {
        let mut sorted: Vec<&UserdataXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&UserdataXConfig> {
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

    pub fn iter(&self) -> UserdataXIterator<'_> {
        UserdataXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct UserdataXIterator<'a> {
    inner: std::slice::Iter<'a, UserdataXConfig>,
}

impl<'a> Iterator for UserdataXIterator<'a> {
    type Item = &'a UserdataXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct UserdataXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl UserdataXCache {
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
pub struct UserdataXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl UserdataXFormatter {
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

    pub fn format_entry(&self, entry: &UserdataXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &UserdataXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &UserdataXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for UserdataXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct UserdataXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl UserdataXValidator {
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

    pub fn validate(&self, entry: &UserdataXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &UserdataXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for UserdataXValidator {
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
// xb_ utilities – batch 92
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer92 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer92 {
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
pub fn xb_fnv1a_92(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_92<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_92<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_92(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_92(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 194
// ---------------------------------------------------------------------------

/// Generic object pool `Xc194Pool<T>`.
pub struct Xc194Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc194Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc194PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc194Pool<T> {
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
    pub fn stats(&self) -> Xc194PoolStats {
        Xc194PoolStats {
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

impl<T> Default for Xc194Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc194Scheduler`.
pub struct Xc194Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc194Scheduler {
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

impl Default for Xc194Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_194 hash for the given byte slice.
pub fn xc_194_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_194 convention.
pub fn xc_194_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe105 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe105Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe105PipelineError {
    pub stage: Xe105Stage,
    pub message: String,
}

impl std::fmt::Display for Xe105PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe105Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe105Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError>>>,
    stage_names: Vec<Xe105Stage>,
}

impl Xe105Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe105Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe105Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe105Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe105Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> {
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

    pub fn compose(mut self, other: Xe105Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe105CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe105CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe105Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe105CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe105CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe105Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe105CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_105_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe105CacheEntry {
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

    fn xe_105_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe105CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_105_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> {
    Ok(data)
}

pub fn xe_105_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_105_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_105_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_105_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe105PipelineError> {
    Err(Xe105PipelineError {
        stage: Xe105Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_103: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg103Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg103Graph {
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

impl Default for Xg103Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_103: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg103Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg103Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg103Heap<T>) {
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

impl<T: Ord> Default for Xg103Heap<T> {
    fn default() -> Self { Self::new() }
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

    // -----------------------------------------------------------------------
    // UserDataExport / UserDataImport tests
    // -----------------------------------------------------------------------

    #[test]
    fn export_new_is_empty() {
        let export = UserDataExport::new(1);
        assert!(export.is_empty());
        assert_eq!(export.profile_count(), 0);
        assert_eq!(export.total_settings_count(), 0);
        assert_eq!(export.version, 1);
    }

    #[test]
    fn export_add_profile() {
        let mut export = UserDataExport::new(2);
        let profile = ExportedProfile {
            id: "work".into(),
            name: "Work".into(),
            settings: HashMap::from([("theme".into(), "dark".into())]),
        };
        export.add_profile(profile);
        assert_eq!(export.profile_count(), 1);
        assert!(!export.is_empty());
        assert_eq!(export.total_settings_count(), 1);
    }

    #[test]
    fn export_add_global_setting() {
        let mut export = UserDataExport::new(1);
        export.add_global_setting("font-size", "14");
        export.add_global_setting("locale", "en");
        assert!(!export.is_empty());
        assert_eq!(export.total_settings_count(), 2);
        assert_eq!(export.profile_count(), 0);
    }

    #[test]
    fn export_total_settings_mixed() {
        let mut export = UserDataExport::new(1);
        export.add_global_setting("k1", "v1");
        let p = ExportedProfile {
            id: "p1".into(),
            name: "P1".into(),
            settings: HashMap::from([("a".into(), "b".into()), ("c".into(), "d".into())]),
        };
        export.add_profile(p);
        assert_eq!(export.total_settings_count(), 3);
    }

    #[test]
    fn export_multiple_profiles() {
        let mut export = UserDataExport::new(1);
        for i in 0..5 {
            export.add_profile(ExportedProfile {
                id: format!("p{i}"),
                name: format!("Profile {i}"),
                settings: HashMap::new(),
            });
        }
        assert_eq!(export.profile_count(), 5);
        assert!(!export.is_empty());
    }

    #[test]
    fn export_clone_eq() {
        let mut export = UserDataExport::new(3);
        export.add_global_setting("x", "y");
        let cloned = export.clone();
        assert_eq!(export, cloned);
    }

    #[test]
    fn import_from_export_replace() {
        let mut export = UserDataExport::new(1);
        export.add_global_setting("k", "v");
        let import = UserDataImport::from_export(export, ImportMergeStrategy::Replace);
        assert_eq!(import.version, 1);
        assert_eq!(import.merge_strategy, ImportMergeStrategy::Replace);
        assert!(!import.global_settings.is_empty());
    }

    #[test]
    fn import_from_export_merge_keep_local() {
        let export = UserDataExport::new(2);
        let import = UserDataImport::from_export(export, ImportMergeStrategy::MergeKeepLocal);
        assert_eq!(import.merge_strategy, ImportMergeStrategy::MergeKeepLocal);
    }

    #[test]
    fn import_validate_version_zero() {
        let import = UserDataImport {
            version: 0,
            profiles: vec![],
            global_settings: HashMap::new(),
            merge_strategy: ImportMergeStrategy::Replace,
        };
        assert!(import.validate().is_err());
    }

    #[test]
    fn import_validate_empty_content() {
        let import = UserDataImport {
            version: 1,
            profiles: vec![],
            global_settings: HashMap::new(),
            merge_strategy: ImportMergeStrategy::Replace,
        };
        assert!(import.validate().is_err());
    }

    #[test]
    fn import_validate_success_with_profiles() {
        let import = UserDataImport {
            version: 1,
            profiles: vec![ExportedProfile {
                id: "a".into(),
                name: "A".into(),
                settings: HashMap::new(),
            }],
            global_settings: HashMap::new(),
            merge_strategy: ImportMergeStrategy::MergeKeepRemote,
        };
        assert!(import.validate().is_ok());
    }

    #[test]
    fn import_validate_success_with_globals() {
        let import = UserDataImport {
            version: 5,
            profiles: vec![],
            global_settings: HashMap::from([("k".into(), "v".into())]),
            merge_strategy: ImportMergeStrategy::Replace,
        };
        assert!(import.validate().is_ok());
    }

    // -----------------------------------------------------------------------
    // UserDataBackup tests
    // -----------------------------------------------------------------------

    #[test]
    fn backup_empty_service() {
        let svc = UserDataService::new("/base");
        let backup = user_data_backup(&svc, 1000);
        assert_eq!(backup.timestamp, 1000);
        assert!(backup.profile_ids.is_empty());
        assert!(backup.backup_label.is_none());
    }

    #[test]
    fn backup_with_profiles() {
        let mut svc = UserDataService::new("/base");
        svc.create_profile("dev".into(), "Dev".into());
        svc.create_profile("test".into(), "Test".into());
        let backup = user_data_backup(&svc, 2000);
        assert_eq!(backup.profile_ids.len(), 2);
        assert!(backup.includes_profile("dev"));
        assert!(backup.includes_profile("test"));
        assert!(!backup.includes_profile("missing"));
    }

    #[test]
    fn backup_with_label() {
        let svc = UserDataService::new("/base");
        let backup = user_data_backup(&svc, 500).with_label("nightly");
        assert_eq!(backup.backup_label, Some("nightly".into()));
    }

    #[test]
    fn backup_path_count() {
        let mut svc = UserDataService::new("/base");
        svc.create_profile("x".into(), "X".into());
        let backup = user_data_backup(&svc, 0);
        assert!(backup.path_count() > 0);
    }

    #[test]
    fn backup_includes_service_dirs() {
        let svc = UserDataService::new("/mydata");
        let backup = user_data_backup(&svc, 100);
        assert!(backup.paths_included.iter().any(|p| p.contains("/mydata")));
    }

    #[test]
    fn backup_total_size_increases_with_profiles() {
        let mut svc = UserDataService::new("/base");
        let b1 = user_data_backup(&svc, 0);
        svc.create_profile("big".into(), "Big Profile".into());
        let b2 = user_data_backup(&svc, 1);
        assert!(b2.total_size_bytes > b1.total_size_bytes);
    }

    #[test]
    fn backup_timestamp_preserved() {
        let svc = UserDataService::new("/d");
        let backup = user_data_backup(&svc, u64::MAX);
        assert_eq!(backup.timestamp, u64::MAX);
    }

    #[test]
    fn backup_clone_eq() {
        let svc = UserDataService::new("/d");
        let backup = user_data_backup(&svc, 42);
        let cloned = backup.clone();
        assert_eq!(backup, cloned);
    }

    // -----------------------------------------------------------------------
    // UserDataLocation tests
    // -----------------------------------------------------------------------

    #[test]
    fn location_new() {
        let loc = UserDataLocation::new("/cfg", "/dat", "/cch", "/log");
        assert_eq!(loc.config_dir, "/cfg");
        assert_eq!(loc.data_dir, "/dat");
        assert_eq!(loc.cache_dir, "/cch");
        assert_eq!(loc.log_dir, "/log");
    }

    #[test]
    fn location_for_linux() {
        let loc = UserDataLocation::for_linux("vsedit");
        assert_eq!(loc.config_dir, "~/.config/vsedit");
        assert_eq!(loc.data_dir, "~/.local/share/vsedit");
        assert_eq!(loc.cache_dir, "~/.cache/vsedit");
        assert_eq!(loc.log_dir, "~/.local/share/vsedit/logs");
    }

    #[test]
    fn location_for_macos() {
        let loc = UserDataLocation::for_macos("vsedit");
        assert_eq!(loc.config_dir, "~/Library/Application Support/vsedit");
        assert_eq!(loc.cache_dir, "~/Library/Caches/vsedit");
        assert_eq!(loc.log_dir, "~/Library/Logs/vsedit");
    }

    #[test]
    fn location_for_windows() {
        let loc = UserDataLocation::for_windows("vsedit");
        assert_eq!(loc.config_dir, "%APPDATA%/vsedit");
        assert_eq!(loc.cache_dir, "%APPDATA%/vsedit/cache");
        assert_eq!(loc.log_dir, "%APPDATA%/vsedit/logs");
    }

    #[test]
    fn location_resolve() {
        let loc = UserDataLocation::for_linux("myapp");
        assert_eq!(loc.resolve("settings.json"), "~/.config/myapp/settings.json");
    }

    #[test]
    fn location_all_dirs() {
        let loc = UserDataLocation::new("/a", "/b", "/c", "/d");
        let dirs = loc.all_dirs();
        assert_eq!(dirs.len(), 4);
        assert!(dirs.contains(&"/a"));
        assert!(dirs.contains(&"/b"));
        assert!(dirs.contains(&"/c"));
        assert!(dirs.contains(&"/d"));
    }

    #[test]
    fn location_clone_eq() {
        let loc = UserDataLocation::for_linux("test");
        let cloned = loc.clone();
        assert_eq!(loc, cloned);
    }

    #[test]
    fn location_resolve_nested() {
        let loc = UserDataLocation::for_windows("app");
        assert_eq!(loc.resolve("profiles/work/settings.json"), "%APPDATA%/app/profiles/work/settings.json");
    }

    #[test]
    fn user_preferences_basic_ops() {
        let mut prefs = UserPreferences::new();
        assert!(prefs.is_empty());
        prefs.set("theme", "dark");
        prefs.set("fontSize", "14");
        assert_eq!(prefs.len(), 2);
        assert!(prefs.has_key("theme"));
        assert_eq!(prefs.get("theme"), Some("dark"));
        assert_eq!(prefs.get_or("missing", "default"), "default");
        prefs.remove("theme");
        assert!(!prefs.has_key("theme"));
    }

    #[test]
    fn user_preferences_merge() {
        let mut a = UserPreferences::new();
        a.set("x", "1");
        a.set("y", "2");
        let mut b = UserPreferences::new();
        b.set("y", "3");
        b.set("z", "4");
        a.merge(&b);
        assert_eq!(a.get("y"), Some("3"));
        assert_eq!(a.get("z"), Some("4"));
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn preferences_migration_apply() {
        let mut prefs = UserPreferences::new();
        prefs.set("old_key", "value");
        prefs.set("deprecated", "x");
        let migration = PreferencesMigration::new(1, 2)
            .rename("old_key", "new_key")
            .remove_key("deprecated")
            .add_default("new_setting", "on");
        migration.migrate(&mut prefs);
        assert!(!prefs.has_key("old_key"));
        assert_eq!(prefs.get("new_key"), Some("value"));
        assert!(!prefs.has_key("deprecated"));
        assert_eq!(prefs.get("new_setting"), Some("on"));
    }

    #[test]
    fn user_data_exporter_round_trip() {
        let mut prefs = UserPreferences::new();
        prefs.set("a", "1");
        prefs.set("b", "2");
        let exported = UserDataExporter::export_to_string(&prefs);
        let imported = UserDataExporter::import_from_string(&exported);
        assert_eq!(imported.get("a"), Some("1"));
        assert_eq!(imported.get("b"), Some("2"));
        assert_eq!(imported.len(), 2);
    }

    #[test]
    fn user_data_exporter_ignores_comments() {
        let data = "# comment\nkey=value\n\n# another\nfoo=bar";
        let prefs = UserDataExporter::import_from_string(data);
        assert_eq!(prefs.len(), 2);
        assert_eq!(prefs.get("key"), Some("value"));
    }

    #[test]
    fn data_integrity_checker_verify() {
        let data = "hello world";
        let checksum = DataIntegrityChecker::compute_checksum(data);
        assert!(DataIntegrityChecker::verify(data, checksum));
        assert!(!DataIntegrityChecker::verify("different", checksum));
    }

    #[test]
    fn workspace_storage_path() {
        let p = UserDataPath::new("/home/user/.config/vsedit");
        assert_eq!(
            p.workspace_storage_path("abc123"),
            "/home/user/.config/vsedit/workspaceStorage/abc123"
        );
    }

    #[test]
    fn global_storage_path() {
        let p = UserDataPath::new("/base");
        assert_eq!(p.global_storage_path(), "/base/globalStorage");
    }

    #[test]
    fn cache_path() {
        let p = UserDataPath::new("/base");
        assert_eq!(p.cache_path(), "/base/CachedData");
    }

    #[test]
    fn crash_reports_path() {
        let p = UserDataPath::new("/base");
        assert_eq!(p.crash_reports_path(), "/base/crash-reports");
    }

    #[test]
    fn is_standard_subpath_true() {
        let p = UserDataPath::new("/base");
        assert!(p.is_standard_subpath("settings.json"));
        assert!(p.is_standard_subpath("logs"));
    }

    #[test]
    fn is_standard_subpath_false() {
        let p = UserDataPath::new("/base");
        assert!(!p.is_standard_subpath("randomfile.txt"));
    }

    #[test]
    fn classify_settings_path() {
        assert_eq!(
            classify_user_data_path("/base/settings.json"),
            UserDataCategory::Settings
        );
    }

    #[test]
    fn classify_extensions_path() {
        assert_eq!(
            classify_user_data_path("/base/extensions/my-ext"),
            UserDataCategory::Extensions
        );
    }

    #[test]
    fn classify_unknown_path() {
        assert_eq!(
            classify_user_data_path("/random/path"),
            UserDataCategory::Unknown
        );
    }

    #[test]
    fn classify_workspace_storage_path() {
        assert_eq!(
            classify_user_data_path("/base/workspaceStorage/abc"),
            UserDataCategory::WorkspaceStorage
        );
    }

    #[test]
    fn user_data_category_display() {
        assert_eq!(format!("{}", UserDataCategory::Settings), "settings");
        assert_eq!(format!("{}", UserDataCategory::Unknown), "unknown");
    }

    #[test]
    fn validate_profile_id_valid() {
        assert!(validate_profile_id("my-profile").is_ok());
        assert!(validate_profile_id("default").is_ok());
        assert!(validate_profile_id("test_123").is_ok());
    }

    #[test]
    fn validate_profile_id_empty() {
        assert!(validate_profile_id("").is_err());
    }

    #[test]
    fn validate_profile_id_bad_chars() {
        assert!(validate_profile_id("has spaces").is_err());
        assert!(validate_profile_id("has/slash").is_err());
    }

    // -- SyncConflict tests ---------------------------------------------------

    #[test]
    fn conflict_local_wins() {
        let conflict = SyncConflict {
            key: "k".into(),
            local_value: "local".into(),
            remote_value: "remote".into(),
            local_timestamp: 100,
            remote_timestamp: 200,
        };
        assert_eq!(conflict.resolve(ConflictStrategy::LocalWins), "local");
    }

    #[test]
    fn conflict_remote_wins() {
        let conflict = SyncConflict {
            key: "k".into(),
            local_value: "local".into(),
            remote_value: "remote".into(),
            local_timestamp: 100,
            remote_timestamp: 200,
        };
        assert_eq!(conflict.resolve(ConflictStrategy::RemoteWins), "remote");
    }

    #[test]
    fn conflict_most_recent() {
        let conflict = SyncConflict {
            key: "k".into(),
            local_value: "local".into(),
            remote_value: "remote".into(),
            local_timestamp: 300,
            remote_timestamp: 200,
        };
        assert_eq!(conflict.resolve(ConflictStrategy::MostRecent), "local");
    }

    #[test]
    fn conflict_display() {
        let conflict = SyncConflict {
            key: "setting".into(),
            local_value: "a".into(),
            remote_value: "b".into(),
            local_timestamp: 1,
            remote_timestamp: 2,
        };
        let s = conflict.to_string();
        assert!(s.contains("setting"));
    }

    // -- ExportManifest tests -------------------------------------------------

    #[test]
    fn export_manifest_tracks_size() {
        let mut manifest = ExportManifest::new("default");
        manifest.add_entry("settings.json", "{\"a\":1}");
        manifest.add_entry("keybindings.json", "[]");
        assert_eq!(manifest.entry_count(), 2);
        assert_eq!(manifest.total_size, 7 + 2);
    }

    #[test]
    fn export_manifest_display() {
        let manifest = ExportManifest::new("test");
        let s = manifest.to_string();
        assert!(s.contains("test"));
        assert!(s.contains("0 files"));
    }

    // -- ImportValidation tests -----------------------------------------------

    #[test]
    fn validate_import_valid() {
        let entries = vec![ExportEntry { relative_path: "settings.json".into(), content: "{}".into(), size_bytes: 2 }];
        let results = validate_import(&entries, 1000);
        assert!(results.is_empty());
    }

    #[test]
    fn validate_import_invalid_path() {
        let entries = vec![ExportEntry { relative_path: "../escape".into(), content: "x".into(), size_bytes: 1 }];
        let results = validate_import(&entries, 1000);
        assert_eq!(results.len(), 1);
        assert!(matches!(&results[0], ImportValidation::InvalidPath(_)));
    }

    #[test]
    fn validate_import_too_large() {
        let entries = vec![ExportEntry { relative_path: "big.json".into(), content: "x".into(), size_bytes: 2000 }];
        let results = validate_import(&entries, 1000);
        assert_eq!(results.len(), 1);
        assert!(matches!(&results[0], ImportValidation::TooLarge { .. }));
    }

    // -- UserDataSizeTracker tests --------------------------------------------

    #[test]
    fn size_tracker_total() {
        let mut tracker = UserDataSizeTracker::new();
        tracker.record("a", 100);
        tracker.record("b", 200);
        assert_eq!(tracker.total_size(), 300);
        assert_eq!(tracker.item_count(), 2);
    }

    #[test]
    fn size_tracker_largest() {
        let mut tracker = UserDataSizeTracker::new();
        tracker.record("small", 10);
        tracker.record("big", 500);
        let (key, size) = tracker.largest_item().unwrap();
        assert_eq!(key, "big");
        assert_eq!(size, 500);
    }

    #[test]
    fn size_tracker_exceeding() {
        let mut tracker = UserDataSizeTracker::new();
        tracker.record("small", 10);
        tracker.record("big", 500);
        let exceeding = tracker.items_exceeding(100);
        assert_eq!(exceeding, vec!["big"]);
    }

    #[test]
    fn size_tracker_display() {
        let tracker = UserDataSizeTracker::new();
        let s = tracker.to_string();
        assert!(s.contains("0 items"));
    }

    #[test]
    fn conflict_strategy_display() {
        assert_eq!(ConflictStrategy::LocalWins.to_string(), "local-wins");
        assert_eq!(ConflictStrategy::MostRecent.to_string(), "most-recent");
    }

    #[test]
    fn userdataImportWizard_new() {
        let s = UserdataImportWizard::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn userdataImportWizard_add_contains() {
        let mut s = UserdataImportWizard::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn userdataImportWizard_add_duplicate() {
        let mut s = UserdataImportWizard::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn userdataImportWizard_remove() {
        let mut s = UserdataImportWizard::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn userdataImportWizard_capacity() {
        let s = UserdataImportWizard::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn userdataImportWizard_search() {
        let mut s = UserdataImportWizard::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn userdataImportWizard_stats() {
        let mut s = UserdataImportWizard::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn userdataSizeCalculator_new() {
        let m = UserdataSizeCalculator::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn userdataSizeCalculator_add_find() {
        let mut m = UserdataSizeCalculator::new();
        m.add(UserdataSizeCalculatorItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn userdataSizeCalculator_priority_filter() {
        let mut m = UserdataSizeCalculator::new();
        m.add(UserdataSizeCalculatorItem::new("a", "A").with_priority(UserdataSizeCalculatorPriority::High));
        m.add(UserdataSizeCalculatorItem::new("b", "B").with_priority(UserdataSizeCalculatorPriority::Low));
        m.add(UserdataSizeCalculatorItem::new("c", "C").with_priority(UserdataSizeCalculatorPriority::High));
        assert_eq!(m.by_priority(UserdataSizeCalculatorPriority::High).len(), 2);
    }

    #[test]
    fn userdataSizeCalculator_remove() {
        let mut m = UserdataSizeCalculator::new();
        m.add(UserdataSizeCalculatorItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn userdataSizeCalculator_search() {
        let mut m = UserdataSizeCalculator::new();
        m.add(UserdataSizeCalculatorItem::new("id1", "Hello World"));
        m.add(UserdataSizeCalculatorItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn userdataSizeCalculator_total_weight() {
        let mut m = UserdataSizeCalculator::new();
        m.add(UserdataSizeCalculatorItem::new("a", "A").with_priority(UserdataSizeCalculatorPriority::Critical));
        m.add(UserdataSizeCalculatorItem::new("b", "B").with_priority(UserdataSizeCalculatorPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn userdataSizeCalculator_capacity_limit() {
        let mut m = UserdataSizeCalculator::new().with_max_items(2);
        m.add(UserdataSizeCalculatorItem::new("1", "one"));
        m.add(UserdataSizeCalculatorItem::new("2", "two"));
        assert!(!m.add(UserdataSizeCalculatorItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn userdataSizeCalculator_sorted_by_priority() {
        let mut m = UserdataSizeCalculator::new();
        m.add(UserdataSizeCalculatorItem::new("lo", "Low").with_priority(UserdataSizeCalculatorPriority::Low));
        m.add(UserdataSizeCalculatorItem::new("hi", "High").with_priority(UserdataSizeCalculatorPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn userdataSizeCalculator_item_metadata() {
        let mut item = UserdataSizeCalculatorItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn userdataImportWizard_enabled_toggle() {
        let mut s = UserdataImportWizard::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn userdataSizeCalculator_priority_display() {
        assert_eq!(format!("{}", UserdataSizeCalculatorPriority::High), "high");
        assert_eq!(format!("{}", UserdataSizeCalculatorPriority::Low), "low");
    }


    #[test]
    fn userdata_x_config_new() {
        let c = UserdataXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn userdata_x_config_builder() {
        let c = UserdataXConfig::new("k")
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
    fn userdata_x_config_display() {
        let c = UserdataXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn userdata_x_registry_insert_get() {
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn userdata_x_registry_duplicate() {
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("a")).unwrap();
        assert!(reg.insert(UserdataXConfig::new("a")).is_err());
    }

    #[test]
    fn userdata_x_registry_remove() {
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("a")).unwrap();
        reg.insert(UserdataXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn userdata_x_registry_active_entries() {
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("a")).unwrap();
        reg.insert(UserdataXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn userdata_x_registry_by_weight() {
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(UserdataXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn userdata_x_registry_tags() {
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(UserdataXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn userdata_x_registry_total_weight() {
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(UserdataXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn userdata_x_registry_iterator() {
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("a")).unwrap();
        reg.insert(UserdataXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn userdata_x_cache_put_get() {
        let mut cache = UserdataXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn userdata_x_cache_eviction() {
        let mut cache = UserdataXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn userdata_x_cache_lru_order() {
        let mut cache = UserdataXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn userdata_x_cache_most_least_recent() {
        let mut cache = UserdataXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn userdata_x_formatter_entry() {
        let e = UserdataXConfig::new("k").with_value("v");
        let fmt = UserdataXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn userdata_x_formatter_summary() {
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("a").with_weight(5)).unwrap();
        let fmt = UserdataXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn userdata_x_validator_valid() {
        let v = UserdataXValidator::new();
        let c = UserdataXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn userdata_x_validator_empty_key() {
        let v = UserdataXValidator::new();
        let c = UserdataXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn userdata_x_validator_require_value() {
        let v = UserdataXValidator::new().require_value(true);
        let c = UserdataXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn userdata_x_validator_allowed_tags() {
        let v = UserdataXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = UserdataXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn userdata_x_validator_validate_all() {
        let v = UserdataXValidator::new();
        let mut reg = UserdataXRegistry::new();
        reg.insert(UserdataXConfig::new("ok")).unwrap();
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
    fn xb_ring_buffer_92_push_and_len() {
        let mut rb = super::XbRingBuffer92::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_92_overwrite() {
        let mut rb = super::XbRingBuffer92::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_92_get_out_of_bounds() {
        let rb = super::XbRingBuffer92::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_92_drain_all() {
        let mut rb = super::XbRingBuffer92::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_92_peek_front_back() {
        let mut rb = super::XbRingBuffer92::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_92_clear() {
        let mut rb = super::XbRingBuffer92::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_92_capacity() {
        let rb = super::XbRingBuffer92::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_92_basic() {
        let h = super::xb_fnv1a_92(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_92(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_92_different_inputs() {
        let h1 = super::xb_fnv1a_92(b"abc");
        let h2 = super::xb_fnv1a_92(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_92_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_92(&data);
        let dec = super::xb_rle_decode_92(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_92_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_92(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_92(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_92_values() {
        assert!((super::xb_clamp_92(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_92(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_92(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_92_values() {
        assert!((super::xb_lerp_92(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_92(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_92(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_92_wrap_around_twice() {
        let mut rb = super::XbRingBuffer92::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 194 ----

    #[test]
    fn xc_194_pool_new_empty() {
        let pool: super::Xc194Pool<i32> = super::Xc194Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_194_pool_release_acquire() {
        let mut pool = super::Xc194Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_194_pool_acquire_empty() {
        let mut pool: super::Xc194Pool<i32> = super::Xc194Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_194_pool_full() {
        let mut pool = super::Xc194Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_194_pool_drain() {
        let mut pool = super::Xc194Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_194_pool_stats() {
        let mut pool = super::Xc194Pool::new(8);
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
    fn xc_194_pool_clear() {
        let mut pool = super::Xc194Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_194_pool_shrink() {
        let mut pool = super::Xc194Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_194_pool_default() {
        let pool: super::Xc194Pool<String> = super::Xc194Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_194_pool_extend() {
        let mut pool = super::Xc194Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_194_pool_retain() {
        let mut pool = super::Xc194Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_194_scheduler_round_robin() {
        let mut sched = super::Xc194Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_194_scheduler_empty() {
        let mut sched = super::Xc194Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_194_scheduler_reset() {
        let mut sched = super::Xc194Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_194_scheduler_add_remove() {
        let mut sched = super::Xc194Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_194_scheduler_targets() {
        let sched = super::Xc194Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_194_hash_empty() {
        assert_eq!(super::xc_194_hash(b""), 5381);
    }

    #[test]
    fn xc_194_hash_data() {
        let h = super::xc_194_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_194_hash(b"hello"), h);
    }

    #[test]
    fn xc_194_reverse_str() {
        assert_eq!(super::xc_194_reverse("abc"), "cba");
        assert_eq!(super::xc_194_reverse(""), "");
    }


    #[test]
    fn xe_105_pipeline_empty() {
        let p = super::Xe105Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_105_pipeline_parse_stage() {
        let p = super::Xe105Pipeline::new()
            .add_parse(super::xe_105_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_105_pipeline_transform_double() {
        let p = super::Xe105Pipeline::new()
            .add_transform(super::xe_105_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_105_pipeline_validate_reverse() {
        let p = super::Xe105Pipeline::new()
            .add_validate(super::xe_105_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_105_pipeline_emit_filter() {
        let p = super::Xe105Pipeline::new()
            .add_emit(super::xe_105_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_105_pipeline_multi_stage() {
        let p = super::Xe105Pipeline::new()
            .add_parse(super::xe_105_pipeline_identity)
            .add_transform(super::xe_105_pipeline_double)
            .add_validate(super::xe_105_pipeline_reverse)
            .add_emit(super::xe_105_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_105_pipeline_error_propagation() {
        let p = super::Xe105Pipeline::new()
            .add_parse(super::xe_105_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe105Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_105_pipeline_compose() {
        let p1 = super::Xe105Pipeline::new()
            .add_parse(super::xe_105_pipeline_identity);
        let p2 = super::Xe105Pipeline::new()
            .add_transform(super::xe_105_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_105_pipeline_error_display() {
        let e = super::Xe105PipelineError {
            stage: super::Xe105Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_105_cache_put_get() {
        let mut c = super::Xe105Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_105_cache_miss() {
        let mut c: super::Xe105Cache<&str, i32> = super::Xe105Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_105_cache_ttl_expiry() {
        let mut c = super::Xe105Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_105_cache_evict() {
        let mut c = super::Xe105Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_105_cache_capacity() {
        let mut c = super::Xe105Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_105_cache_stats() {
        let mut c = super::Xe105Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_105_cache_clear() {
        let mut c = super::Xe105Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_103 graph tests ------------------------------------------------

    #[test]
    fn xg_103_graph_empty() {
        let g = super::Xg103Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_103_graph_add_node() {
        let mut g = super::Xg103Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_103_graph_add_edge() {
        let mut g = super::Xg103Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_103_graph_neighbors() {
        let mut g = super::Xg103Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_103_graph_has_path() {
        let mut g = super::Xg103Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_103_graph_self_path() {
        let g = super::Xg103Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_103_graph_topo_sort() {
        let mut g = super::Xg103Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_103_graph_cycle_detect_false() {
        let mut g = super::Xg103Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_103_graph_cycle_detect_true() {
        let mut g = super::Xg103Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_103 heap tests -------------------------------------------------

    #[test]
    fn xg_103_heap_empty() {
        let h: super::Xg103Heap<i32> = super::Xg103Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_103_heap_push_pop() {
        let mut h = super::Xg103Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_103_heap_peek() {
        let mut h = super::Xg103Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_103_heap_drain_sorted() {
        let mut h = super::Xg103Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_103_heap_merge() {
        let mut a = super::Xg103Heap::new();
        let mut b = super::Xg103Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_103_heap_default() {
        let h: super::Xg103Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_103_graph_default() {
        let g: super::Xg103Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }

}
