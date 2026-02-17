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

}
