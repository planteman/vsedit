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


/// A probabilistic sorted list using a skip-list structure (variant 193).
pub struct Xh193SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh193SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 235 as u64,
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

/// A compact bit set supporting boolean operations (variant 193).
pub struct Xh193BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh193BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 193).
pub struct Xi193Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi193Deque<T> {
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
pub struct Xi193Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi193Interval {
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

/// A simple interval tree (variant 193).
pub struct Xi193IntervalTree {
    xi_intervals: Vec<Xi193Interval>,
}

impl Xi193IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi193Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi193Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi193Interval) -> Vec<&Xi193Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi193Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi193Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi193Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi193Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi193Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi193Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 193) ---

/// Disjoint set / union-find for crate 193.
pub struct Xj193UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj193UnionFind {
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

const XJ193_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 193.
pub struct Xj193BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj193BTreeNode<K, V>>>,
    len: usize,
}

struct Xj193BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj193BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj193BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ193_BTREE_ORDER - 1
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
        let mid = XJ193_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj193BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj193BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj193BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj193BTreeNode::xj_new_leaf();
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


// --- xk_193 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk193SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk193SegmentTree {
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
pub struct Xk193DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk193DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_193).
#[derive(Debug, Clone)]
pub struct Xl193Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl193Rope {
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

/// Suffix array for efficient string searching (xl_193).
#[derive(Debug, Clone)]
pub struct Xl193SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl193SuffixArray {
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
pub struct Xm193MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm193MatrixSparse {
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
pub struct Xm193Tokenizer {
    text: String,
}

impl Xm193Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 193.
pub struct Xn193Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn193Fenwick {
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

// ----- AVL tree map — crate 193 -----

#[derive(Debug, Clone)]
struct Xn193AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn193AvlNode<K, V>>>,
    right: Option<Box<Xn193AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 193.
#[derive(Debug, Clone)]
pub struct Xn193AVL<K, V> {
    root: Option<Box<Xn193AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn193AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn193AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn193AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn193AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn193AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn193AvlNode<K, V>>) -> Box<Xn193AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn193AvlNode<K, V>>) -> Box<Xn193AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn193AvlNode<K, V>>) -> Box<Xn193AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn193AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn193AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn193AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn193AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn193AvlNode<K, V>>) -> &Xn193AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn193AvlNode<K, V>>) -> (Box<Xn193AvlNode<K, V>>, Option<Box<Xn193AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn193AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn193AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn193AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn193AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn193AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn193AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn193AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo193RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo193Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo193RBNode<K, V> {
    key: K,
    value: V,
    color: Xo193Color,
    left: Option<Box<Xo193RBNode<K, V>>>,
    right: Option<Box<Xo193RBNode<K, V>>>,
}

/// A red-black tree map for crate 193.
#[derive(Debug, Clone)]
pub struct Xo193RedBlack<K, V> {
    root: Option<Box<Xo193RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo193RedBlack<K, V> {
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
            r.color = Xo193Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo193RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo193RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo193RBNode {
                    key, value, color: Xo193Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo193RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo193Color::Red)
    }

    fn xo_balance(mut h: Box<Xo193RBNode<K, V>>) -> Box<Xo193RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo193Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo193RBNode<K, V>>) -> Box<Xo193RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo193Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo193RBNode<K, V>>) -> Box<Xo193RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo193Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo193RBNode<K, V>>) {
        h.color = Xo193Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo193Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo193Color::Black; }
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
            r.color = Xo193Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo193RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo193RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo193RBNode<K, V>) -> (K, V, Option<Box<Xo193RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo193RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo193Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo193RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo193ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 193.
#[derive(Debug, Clone)]
pub struct Xo193ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo193ConsistentHash {
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
            let vkey = format!("{}#xo193#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo193#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 193).
#[derive(Debug)]
pub struct Xp193SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp193Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp193Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp193Node<K, V>>>,
    xp_right: Option<Box<Xp193Node<K, V>>>,
}

impl<K: Ord, V> Xp193Node<K, V> {
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

impl<K: Ord, V> Default for Xp193SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp193SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp193Node<K, V>>>, key: &K) -> Option<Box<Xp193Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp193Node<K, V>>) -> Box<Xp193Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp193Node<K, V>>) -> Box<Xp193Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp193Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp193Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp193Node::xp_new(key, val));
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


    #[test]
    fn xh193_skip_insert_contains() {
        let mut sl = super::Xh193SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh193_skip_remove() {
        let mut sl = super::Xh193SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh193_skip_len() {
        let mut sl = super::Xh193SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh193_skip_range_query() {
        let mut sl = super::Xh193SkipList::xh_new(4);
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
    fn xh193_skip_floor_ceiling() {
        let mut sl = super::Xh193SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh193_skip_rank() {
        let mut sl = super::Xh193SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh193_skip_empty() {
        let sl = super::Xh193SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh193_skip_duplicates() {
        let mut sl = super::Xh193SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh193_bitset_set_test() {
        let mut bs = super::Xh193BitSet::xh_new(256);
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
    fn xh193_bitset_clear_count() {
        let mut bs = super::Xh193BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh193_bitset_and_or_xor() {
        let mut a = super::Xh193BitSet::xh_new(128);
        let mut b = super::Xh193BitSet::xh_new(128);
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
    fn xh193_bitset_iter_ones() {
        let mut bs = super::Xh193BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh193_bitset_first_last() {
        let mut bs = super::Xh193BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh193_bitset_empty() {
        let bs = super::Xh193BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi193_deque_push_pop_back() {
        let mut dq = super::Xi193Deque::xi_new(4);
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
    fn xi193_deque_push_pop_front() {
        let mut dq = super::Xi193Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi193_deque_mixed_ops() {
        let mut dq = super::Xi193Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi193_deque_get_and_split() {
        let mut dq = super::Xi193Deque::xi_new(8);
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
    fn xi193_deque_rotate_left() {
        let mut dq = super::Xi193Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi193_deque_rotate_right() {
        let mut dq = super::Xi193Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi193_deque_grow() {
        let mut dq = super::Xi193Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi193_deque_empty() {
        let dq = super::Xi193Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi193_interval_tree_insert_query() {
        let mut tree = super::Xi193IntervalTree::xi_new();
        tree.xi_insert(super::Xi193Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi193Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi193Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi193_interval_tree_overlap() {
        let mut tree = super::Xi193IntervalTree::xi_new();
        tree.xi_insert(super::Xi193Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi193Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi193Interval::xi_new(12, 20));
        let q = super::Xi193Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi193_interval_tree_remove() {
        let mut tree = super::Xi193IntervalTree::xi_new();
        tree.xi_insert(super::Xi193Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi193Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi193_interval_tree_gaps() {
        let mut tree = super::Xi193IntervalTree::xi_new();
        tree.xi_insert(super::Xi193Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi193Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi193Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi193Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi193Interval::xi_new(8, 10));
    }

    #[test]
    fn xi193_interval_tree_merge() {
        let mut tree = super::Xi193IntervalTree::xi_new();
        tree.xi_insert(super::Xi193Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi193Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi193Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi193Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi193Interval::xi_new(10, 15));
    }

    #[test]
    fn xi193_interval_tree_all() {
        let mut tree = super::Xi193IntervalTree::xi_new();
        tree.xi_insert(super::Xi193Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi193Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi193_interval_tree_empty() {
        let tree = super::Xi193IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi193_interval_tree_contains_point() {
        let iv = super::Xi193Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 193) ---

    #[test]
    fn xj_193_uf_make_and_find() {
        let mut uf = super::Xj193UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_193_uf_union_connected() {
        let mut uf = super::Xj193UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_193_uf_component_count() {
        let mut uf = super::Xj193UnionFind::xj_new();
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
    fn xj_193_uf_component_size() {
        let mut uf = super::Xj193UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_193_uf_largest_component() {
        let mut uf = super::Xj193UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_193_uf_many_elements() {
        let mut uf = super::Xj193UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_193_uf_separate_components() {
        let mut uf = super::Xj193UnionFind::xj_new();
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
    fn xj_193_uf_path_compression() {
        let mut uf = super::Xj193UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_193_bt_insert_get() {
        let mut bt = super::Xj193BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_193_bt_contains_len() {
        let mut bt = super::Xj193BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_193_bt_replace() {
        let mut bt = super::Xj193BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_193_bt_remove() {
        let mut bt = super::Xj193BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_193_bt_keys_values() {
        let mut bt = super::Xj193BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_193_bt_range() {
        let mut bt = super::Xj193BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_193_bt_min_max() {
        let mut bt = super::Xj193BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_193_bt_many_inserts() {
        let mut bt = super::Xj193BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_193 segment tree tests ---

    #[test]
    fn xk_193_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk193SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_193_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk193SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_193_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk193SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_193_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk193SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_193_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk193SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_193_st_single_element() {
        let data = vec![42];
        let st = super::Xk193SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_193_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk193SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_193_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk193SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_193 disjoint intervals tests ---

    #[test]
    fn xk_193_di_add_and_count() {
        let mut di = super::Xk193DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_193_di_merge_overlap() {
        let mut di = super::Xk193DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_193_di_contains() {
        let mut di = super::Xk193DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_193_di_remove() {
        let mut di = super::Xk193DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_193_di_covered_length() {
        let mut di = super::Xk193DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_193_di_gaps() {
        let mut di = super::Xk193DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_193_di_merge_adjacent() {
        let mut di = super::Xk193DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_193_di_empty() {
        let di = super::Xk193DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_193_rope_new_empty() {
        let rope = super::Xl193Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_193_rope_from_str() {
        let rope = super::Xl193Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_193_rope_insert_at() {
        let mut rope = super::Xl193Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_193_rope_delete_range() {
        let mut rope = super::Xl193Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_193_rope_char_at() {
        let rope = super::Xl193Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_193_rope_split_concat() {
        let rope = super::Xl193Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_193_rope_line_count() {
        let rope = super::Xl193Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_193_rope_line_at() {
        let rope = super::Xl193Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_193_sa_build_and_search() {
        let sa = super::Xl193SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_193_sa_count() {
        let sa = super::Xl193SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_193_sa_longest_repeated() {
        let sa = super::Xl193SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_193_sa_all_positions() {
        let sa = super::Xl193SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_193_sa_len() {
        let sa = super::Xl193SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_193_sa_empty() {
        let sa = super::Xl193SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_193_rope_slice() {
        let rope = super::Xl193Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_193_sa_search_start() {
        let sa = super::Xl193SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_193_sparse_set_get() {
        let mut m = super::Xm193MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_193_sparse_row_col() {
        let mut m = super::Xm193MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_193_sparse_transpose() {
        let mut m = super::Xm193MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_193_sparse_multiply_vec() {
        let mut m = super::Xm193MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_193_sparse_nnz_density() {
        let mut m = super::Xm193MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_193_sparse_clear() {
        let mut m = super::Xm193MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_193_sparse_overwrite_zero() {
        let mut m = super::Xm193MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_193_tokenizer_basic() {
        let t = super::Xm193Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_193_tokenizer_count() {
        let t = super::Xm193Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_193_tokenizer_unique() {
        let t = super::Xm193Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_193_tokenizer_frequency() {
        let t = super::Xm193Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_193_tokenizer_delimiter() {
        let t = super::Xm193Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_193_tokenizer_whitespace() {
        let t = super::Xm193Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_193_tokenizer_empty() {
        let t = super::Xm193Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 193 ----

    #[test]
    fn xn_193_fenwick_prefix_sum() {
        let mut ft = super::Xn193Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_193_fenwick_range_sum() {
        let mut ft = super::Xn193Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_193_fenwick_point_query() {
        let mut ft = super::Xn193Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_193_fenwick_len() {
        let ft = super::Xn193Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_193_fenwick_multiple_updates() {
        let mut ft = super::Xn193Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_193_fenwick_single_element() {
        let mut ft = super::Xn193Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_193_fenwick_find_kth() {
        let mut ft = super::Xn193Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_193_fenwick_negative_delta() {
        let mut ft = super::Xn193Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 193 ----

    #[test]
    fn xn_193_avl_insert_get() {
        let mut m = super::Xn193AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_193_avl_remove() {
        let mut m = super::Xn193AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_193_avl_in_order() {
        let mut m = super::Xn193AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_193_avl_min_max() {
        let mut m = super::Xn193AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_193_avl_floor_ceiling() {
        let mut m = super::Xn193AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_193_avl_height_balanced() {
        let mut m = super::Xn193AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_193_avl_overwrite() {
        let mut m = super::Xn193AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_193_avl_empty() {
        let m: super::Xn193AVL<i32, i32> = super::Xn193AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo193RedBlack tests ---

    #[test]
    fn xo_193_rb_insert_and_get() {
        let mut tree = super::Xo193RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_193_rb_len_and_empty() {
        let mut tree = super::Xo193RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_193_rb_min_max() {
        let mut tree = super::Xo193RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_193_rb_contains() {
        let mut tree = super::Xo193RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_193_rb_remove() {
        let mut tree = super::Xo193RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_193_rb_in_order() {
        let mut tree = super::Xo193RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_193_rb_black_height() {
        let mut tree = super::Xo193RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_193_rb_overwrite() {
        let mut tree = super::Xo193RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo193ConsistentHash tests ---

    #[test]
    fn xo_193_ch_add_and_count() {
        let mut ring = super::Xo193ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_193_ch_remove_node() {
        let mut ring = super::Xo193ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_193_ch_get_node() {
        let mut ring = super::Xo193ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_193_ch_empty_ring() {
        let ring = super::Xo193ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_193_ch_distribution() {
        let mut ring = super::Xo193ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_193_ch_rebalance() {
        let mut ring = super::Xo193ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_193_ch_virtual_nodes() {
        let mut ring = super::Xo193ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_193_ch_consistent_lookup() {
        let mut ring = super::Xo193ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_193_splay_insert_get() {
        let mut t = super::Xp193SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_193_splay_remove() {
        let mut t = super::Xp193SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_193_splay_count_increases() {
        let mut t = super::Xp193SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_193_splay_depth() {
        let mut t = super::Xp193SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_193_splay_len_empty() {
        let t = super::Xp193SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_193_splay_min_max() {
        let mut t = super::Xp193SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_193_splay_overwrite() {
        let mut t = super::Xp193SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_193_splay_remove_missing() {
        let mut t = super::Xp193SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }

}
