//! Product configuration service.
//!
//! Equivalent to VS Code's `product.json`.
//! Contains metadata about the product (name, version, URLs, etc.).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Errors that can occur when working with product configuration.
#[derive(Debug)]
pub enum ProductError {
    /// JSON parsing or serialization failed.
    Json(serde_json::Error),
    /// A required field is missing or empty.
    ValidationError(String),
}

impl fmt::Display for ProductError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProductError::Json(e) => write!(f, "JSON error: {}", e),
            ProductError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for ProductError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProductError::Json(e) => Some(e),
            ProductError::ValidationError(_) => None,
        }
    }
}

impl From<serde_json::Error> for ProductError {
    fn from(e: serde_json::Error) -> Self {
        ProductError::Json(e)
    }
}

/// Product configuration loaded from product.json or compiled defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductConfiguration {
    pub name_short: String,
    pub name_long: String,
    pub application_name: String,
    pub data_folder_name: String,
    pub version: String,
    pub quality: Option<String>,
    pub commit: Option<String>,
    pub date: Option<String>,

    #[serde(default)]
    pub extensions_gallery: Option<ExtensionsGallery>,

    #[serde(default)]
    pub enable_telemetry: bool,

    #[serde(default)]
    pub report_issue_url: Option<String>,

    #[serde(default)]
    pub documentation_url: Option<String>,

    #[serde(default)]
    pub release_notes_url: Option<String>,

    #[serde(default)]
    pub update_url: Option<String>,

    #[serde(default)]
    pub license_url: Option<String>,

    #[serde(default)]
    pub settings_sync_url: Option<String>,
}

/// Extension gallery configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionsGallery {
    pub service_url: String,
    pub item_url: String,
    #[serde(default)]
    pub control_url: Option<String>,
}

impl ProductConfiguration {
    /// Load product configuration from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Get the default product configuration for vsedit.
    pub fn default_config() -> Self {
        Self {
            name_short: "vsedit".to_string(),
            name_long: "Visual Studio Edit".to_string(),
            application_name: "vsedit".to_string(),
            data_folder_name: ".vsedit".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            quality: Some("stable".to_string()),
            commit: None,
            date: None,
            extensions_gallery: Some(ExtensionsGallery {
                service_url: "https://marketplace.visualstudio.com/_apis/public/gallery"
                    .to_string(),
                item_url: "https://marketplace.visualstudio.com/items".to_string(),
                control_url: None,
            }),
            enable_telemetry: false,
            report_issue_url: None,
            documentation_url: None,
            release_notes_url: None,
            update_url: None,
            license_url: None,
            settings_sync_url: None,
        }
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }

    /// Returns `true` if quality is "stable".
    pub fn is_stable(&self) -> bool {
        self.quality.as_deref() == Some("stable")
    }

    /// Returns `true` if quality is "insider".
    pub fn is_insider(&self) -> bool {
        self.quality.as_deref() == Some("insider")
    }

    /// Returns a display name in the form "name_long version".
    pub fn display_name(&self) -> String {
        format!("{} {}", self.name_long, self.version)
    }

    /// Returns `true` if an extensions gallery is configured.
    pub fn has_gallery(&self) -> bool {
        self.extensions_gallery.is_some()
    }

    /// Merge fields from a partial JSON string into this configuration.
    ///
    /// Only non-null fields in the provided JSON will overwrite existing values.
    pub fn merge_from_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let overlay: serde_json::Value = serde_json::from_str(json)?;
        let mut base: serde_json::Value = serde_json::to_value(&*self)?;

        if let (Some(base_map), Some(overlay_map)) = (base.as_object_mut(), overlay.as_object()) {
            for (key, value) in overlay_map {
                if !value.is_null() {
                    base_map.insert(key.clone(), value.clone());
                }
            }
        }

        *self = serde_json::from_value(base)?;
        Ok(())
    }
}

impl std::fmt::Display for ProductConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.name_short, self.version)
    }
}

impl ExtensionsGallery {
    /// Returns `true` if a control URL is configured.
    pub fn has_control_url(&self) -> bool {
        self.control_url.is_some()
    }

    /// Builds a search URL by appending a query to the service URL.
    pub fn search_url(&self, query: &str) -> String {
        format!("{}/extensionquery?query={}", self.service_url, query)
    }

    /// Builds a URL for a specific extension item by identifier.
    pub fn item_detail_url(&self, identifier: &str) -> String {
        format!("{}?itemName={}", self.item_url, identifier)
    }
}

impl fmt::Display for ExtensionsGallery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Gallery({})", self.service_url)
    }
}

/// Builder for constructing a [`ProductConfiguration`] step by step.
#[derive(Debug, Default)]
pub struct ProductConfigurationBuilder {
    name_short: Option<String>,
    name_long: Option<String>,
    application_name: Option<String>,
    data_folder_name: Option<String>,
    version: Option<String>,
    quality: Option<String>,
    extensions_gallery: Option<ExtensionsGallery>,
    enable_telemetry: bool,
}

impl ProductConfigurationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name_short(mut self, v: impl Into<String>) -> Self {
        self.name_short = Some(v.into());
        self
    }

    pub fn name_long(mut self, v: impl Into<String>) -> Self {
        self.name_long = Some(v.into());
        self
    }

    pub fn application_name(mut self, v: impl Into<String>) -> Self {
        self.application_name = Some(v.into());
        self
    }

    pub fn data_folder_name(mut self, v: impl Into<String>) -> Self {
        self.data_folder_name = Some(v.into());
        self
    }

    pub fn version(mut self, v: impl Into<String>) -> Self {
        self.version = Some(v.into());
        self
    }

    pub fn quality(mut self, v: impl Into<String>) -> Self {
        self.quality = Some(v.into());
        self
    }

    pub fn extensions_gallery(mut self, v: ExtensionsGallery) -> Self {
        self.extensions_gallery = Some(v);
        self
    }

    pub fn enable_telemetry(mut self, v: bool) -> Self {
        self.enable_telemetry = v;
        self
    }

    /// Build the configuration, returning an error if required fields are missing.
    pub fn build(self) -> Result<ProductConfiguration, ProductError> {
        let name_short = self.name_short.ok_or_else(|| {
            ProductError::ValidationError("name_short is required".into())
        })?;
        let name_long = self.name_long.ok_or_else(|| {
            ProductError::ValidationError("name_long is required".into())
        })?;
        let application_name = self.application_name.ok_or_else(|| {
            ProductError::ValidationError("application_name is required".into())
        })?;
        let version = self.version.ok_or_else(|| {
            ProductError::ValidationError("version is required".into())
        })?;

        if name_short.is_empty() {
            return Err(ProductError::ValidationError(
                "name_short must not be empty".into(),
            ));
        }

        let data_folder_name = self
            .data_folder_name
            .unwrap_or_else(|| format!(".{}", application_name));

        Ok(ProductConfiguration {
            name_short,
            name_long,
            application_name,
            data_folder_name,
            version,
            quality: self.quality,
            commit: None,
            date: None,
            extensions_gallery: self.extensions_gallery,
            enable_telemetry: self.enable_telemetry,
            report_issue_url: None,
            documentation_url: None,
            release_notes_url: None,
            update_url: None,
            license_url: None,
            settings_sync_url: None,
        })
    }
}

impl ProductConfiguration {
    /// Create a builder for `ProductConfiguration`.
    pub fn builder() -> ProductConfigurationBuilder {
        ProductConfigurationBuilder::new()
    }

    /// Validate that the configuration has all required non-empty fields.
    pub fn validate(&self) -> Result<(), ProductError> {
        if self.name_short.is_empty() {
            return Err(ProductError::ValidationError(
                "name_short must not be empty".into(),
            ));
        }
        if self.name_long.is_empty() {
            return Err(ProductError::ValidationError(
                "name_long must not be empty".into(),
            ));
        }
        if self.version.is_empty() {
            return Err(ProductError::ValidationError(
                "version must not be empty".into(),
            ));
        }
        Ok(())
    }

    /// Parse a semver-style version string into (major, minor, patch) components.
    /// Returns `None` if the version string is not in the expected format.
    pub fn version_tuple(&self) -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = self.version.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].parse().ok()?;
        Some((major, minor, patch))
    }

    /// Returns `true` if this version is newer than the given version string.
    pub fn is_newer_than(&self, other_version: &str) -> Option<bool> {
        let self_tuple = self.version_tuple()?;
        let parts: Vec<&str> = other_version.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let other_tuple: (u32, u32, u32) = (
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        );
        Some(self_tuple > other_tuple)
    }

    /// Returns a user-agent string suitable for HTTP requests.
    pub fn user_agent(&self) -> String {
        let quality_suffix = self
            .quality
            .as_deref()
            .map(|q| format!("-{}", q))
            .unwrap_or_default();
        format!("{}/{}{}", self.application_name, self.version, quality_suffix)
    }

    /// Count how many optional URL fields are configured.
    pub fn configured_url_count(&self) -> usize {
        [
            &self.report_issue_url,
            &self.documentation_url,
            &self.release_notes_url,
            &self.update_url,
            &self.license_url,
            &self.settings_sync_url,
        ]
        .iter()
        .filter(|u| u.is_some())
        .count()
    }
}

/// Accumulated statistics for product operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ProductStats {
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
    pub fn merge(&mut self, other: &ProductStats) {
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

impl Default for ProductStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProductStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProductStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for product.
#[derive(Debug, Clone)]
pub struct ProductValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ProductValidator {
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

impl Default for ProductValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Feature flags for enabling/disabling product features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductFeatureFlags {
    flags: std::collections::HashMap<String, bool>,
}

impl ProductFeatureFlags {
    pub fn new() -> Self {
        Self {
            flags: std::collections::HashMap::new(),
        }
    }

    /// Set a feature flag.
    pub fn set(&mut self, name: impl Into<String>, enabled: bool) {
        self.flags.insert(name.into(), enabled);
    }

    /// Check if a feature is enabled. Returns `false` for unknown flags.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.flags.get(name).copied().unwrap_or(false)
    }

    /// Remove a flag, returning its previous value if it existed.
    pub fn remove(&mut self, name: &str) -> Option<bool> {
        self.flags.remove(name)
    }

    /// Return all enabled feature names.
    pub fn enabled_features(&self) -> Vec<&str> {
        self.flags
            .iter()
            .filter(|&(_, &v)| v)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Return all disabled feature names.
    pub fn disabled_features(&self) -> Vec<&str> {
        self.flags
            .iter()
            .filter(|&(_, &v)| !v)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.flags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }
}

impl Default for ProductFeatureFlags {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProductFeatureFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let enabled = self.enabled_features().len();
        let total = self.flags.len();
        write!(f, "FeatureFlags({}/{} enabled)", enabled, total)
    }
}

/// Update channel that determines which release stream is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Insiders,
    Dev,
}

impl UpdateChannel {
    /// Return the channel identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            UpdateChannel::Stable => "stable",
            UpdateChannel::Insiders => "insiders",
            UpdateChannel::Dev => "dev",
        }
    }

    /// Parse a channel from a string. Returns `Stable` as default.
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "insiders" => UpdateChannel::Insiders,
            "dev" | "development" => UpdateChannel::Dev,
            _ => UpdateChannel::Stable,
        }
    }
}

impl fmt::Display for UpdateChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Determine the update channel from a product configuration.
pub fn product_update_channel(config: &ProductConfiguration) -> UpdateChannel {
    match config.quality.as_deref() {
        Some("insider") | Some("insiders") => UpdateChannel::Insiders,
        Some("dev") | Some("development") => UpdateChannel::Dev,
        _ => UpdateChannel::Stable,
    }
}

/// License type for a product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseType {
    Mit,
    Apache2,
    Gpl3,
    Proprietary,
    Custom,
}

impl LicenseType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LicenseType::Mit => "MIT",
            LicenseType::Apache2 => "Apache-2.0",
            LicenseType::Gpl3 => "GPL-3.0",
            LicenseType::Proprietary => "Proprietary",
            LicenseType::Custom => "Custom",
        }
    }

    /// Returns `true` if this is an open-source license.
    pub fn is_open_source(&self) -> bool {
        !matches!(self, LicenseType::Proprietary | LicenseType::Custom)
    }
}

impl fmt::Display for LicenseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Product license information and validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductLicense {
    pub license_type: LicenseType,
    pub holder: String,
    pub year: u16,
    pub spdx_id: Option<String>,
}

impl ProductLicense {
    pub fn new(license_type: LicenseType, holder: impl Into<String>, year: u16) -> Self {
        let spdx_id = match license_type {
            LicenseType::Mit => Some("MIT".to_string()),
            LicenseType::Apache2 => Some("Apache-2.0".to_string()),
            LicenseType::Gpl3 => Some("GPL-3.0-only".to_string()),
            _ => None,
        };
        Self {
            license_type,
            holder: holder.into(),
            year,
            spdx_id,
        }
    }

    /// Returns a single-line copyright notice.
    pub fn copyright_notice(&self) -> String {
        format!(
            "Copyright (c) {} {} - {}",
            self.year,
            self.holder,
            self.license_type
        )
    }

    /// Validate that the license has sensible field values.
    pub fn validate(&self) -> Result<(), String> {
        if self.holder.is_empty() {
            return Err("license holder must not be empty".to_string());
        }
        if self.year < 1970 || self.year > 2100 {
            return Err(format!("license year {} is out of range [1970..2100]", self.year));
        }
        Ok(())
    }
}

impl fmt::Display for ProductLicense {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.license_type, self.holder)
    }
}

/// A condition that gates whether a feature flag should be active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureCondition {
    /// Always evaluate to the given value.
    Always(bool),
    /// Active only for the specified update channel.
    Channel(UpdateChannel),
    /// Active only when the product version is at least (major, minor, patch).
    MinVersion(u32, u32, u32),
    /// Active when ALL sub-conditions are true.
    AllOf(Vec<FeatureCondition>),
    /// Active when ANY sub-condition is true.
    AnyOf(Vec<FeatureCondition>),
}

/// Evaluates feature flags with conditions against a product configuration.
#[derive(Debug, Clone)]
pub struct FeatureFlagEvaluator {
    rules: Vec<(String, FeatureCondition)>,
}

impl FeatureFlagEvaluator {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Register a feature with a condition.
    pub fn add_rule(&mut self, feature: impl Into<String>, condition: FeatureCondition) {
        self.rules.push((feature.into(), condition));
    }

    /// Evaluate all registered rules against the given configuration and return
    /// a [`ProductFeatureFlags`] containing the results.
    pub fn evaluate(&self, config: &ProductConfiguration) -> ProductFeatureFlags {
        let mut flags = ProductFeatureFlags::new();
        for (name, cond) in &self.rules {
            flags.set(name.clone(), Self::eval_condition(cond, config));
        }
        flags
    }

    fn eval_condition(cond: &FeatureCondition, config: &ProductConfiguration) -> bool {
        match cond {
            FeatureCondition::Always(v) => *v,
            FeatureCondition::Channel(ch) => product_update_channel(config) == *ch,
            FeatureCondition::MinVersion(maj, min, pat) => {
                if let Some((a, b, c)) = config.version_tuple() {
                    (a, b, c) >= (*maj, *min, *pat)
                } else {
                    false
                }
            }
            FeatureCondition::AllOf(subs) => {
                subs.iter().all(|s| Self::eval_condition(s, config))
            }
            FeatureCondition::AnyOf(subs) => {
                subs.iter().any(|s| Self::eval_condition(s, config))
            }
        }
    }
}

impl Default for FeatureFlagEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Compatibility check result between two product configurations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityReport {
    pub compatible: bool,
    pub warnings: Vec<String>,
}

impl CompatibilityReport {
    /// Returns `true` if there are any warnings.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

impl fmt::Display for CompatibilityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.compatible {
            write!(f, "Compatible")?;
        } else {
            write!(f, "Incompatible")?;
        }
        if !self.warnings.is_empty() {
            write!(f, " ({} warning(s))", self.warnings.len())?;
        }
        Ok(())
    }
}

/// Check compatibility between two product configurations.
pub fn check_compatibility(
    source: &ProductConfiguration,
    target: &ProductConfiguration,
) -> CompatibilityReport {
    let mut warnings = Vec::new();
    let mut compatible = true;

    if source.application_name != target.application_name {
        warnings.push(format!(
            "application name mismatch: '{}' vs '{}'",
            source.application_name, target.application_name
        ));
        compatible = false;
    }

    if let (Some(sq), Some(tq)) = (&source.quality, &target.quality) {
        if sq != tq {
            warnings.push(format!("quality mismatch: '{}' vs '{}'", sq, tq));
        }
    }

    if let (Some(sv), Some(tv)) = (source.version_tuple(), target.version_tuple()) {
        if sv.0 != tv.0 {
            warnings.push(format!(
                "major version mismatch: {} vs {}",
                sv.0, tv.0
            ));
            compatible = false;
        }
    }

    if source.extensions_gallery.is_some() != target.extensions_gallery.is_some() {
        warnings.push("gallery configuration differs".to_string());
    }

    CompatibilityReport {
        compatible,
        warnings,
    }
}

/// Telemetry configuration derived from product settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryConfig {
    pub enabled: bool,
    pub level: TelemetryLevel,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TelemetryLevel {
    Off,
    Error,
    Crash,
    All,
}

impl TelemetryLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            TelemetryLevel::Off => "off",
            TelemetryLevel::Error => "error",
            TelemetryLevel::Crash => "crash",
            TelemetryLevel::All => "all",
        }
    }
}

impl fmt::Display for TelemetryLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Build telemetry configuration from a product configuration.
pub fn product_telemetry_config(config: &ProductConfiguration) -> TelemetryConfig {
    TelemetryConfig {
        enabled: config.enable_telemetry,
        level: if config.enable_telemetry {
            TelemetryLevel::All
        } else {
            TelemetryLevel::Off
        },
        endpoint: config.settings_sync_url.clone(),
    }
}

impl ProductConfiguration {
    /// Returns a list of all configured URL field names and their values.
    pub fn configured_urls(&self) -> Vec<(&'static str, &str)> {
        let mut urls = Vec::new();
        if let Some(ref u) = self.report_issue_url { urls.push(("report_issue_url", u.as_str())); }
        if let Some(ref u) = self.documentation_url { urls.push(("documentation_url", u.as_str())); }
        if let Some(ref u) = self.release_notes_url { urls.push(("release_notes_url", u.as_str())); }
        if let Some(ref u) = self.update_url { urls.push(("update_url", u.as_str())); }
        if let Some(ref u) = self.license_url { urls.push(("license_url", u.as_str())); }
        if let Some(ref u) = self.settings_sync_url { urls.push(("settings_sync_url", u.as_str())); }
        urls
    }

    /// Returns a summary string with name, version, and quality.
    pub fn summary_line(&self) -> String {
        match &self.quality {
            Some(q) => format!("{} v{} ({})", self.name_short, self.version, q),
            None => format!("{} v{}", self.name_short, self.version),
        }
    }

    /// Returns true if the product has a commit hash set.
    pub fn has_commit(&self) -> bool {
        self.commit.as_ref().map_or(false, |c| !c.is_empty())
    }

    /// Returns true if the product has a build date set.
    pub fn has_date(&self) -> bool {
        self.date.as_ref().map_or(false, |d| !d.is_empty())
    }

    /// Returns the major version number, or None if unparseable.
    pub fn major_version(&self) -> Option<u32> {
        self.version_tuple().map(|(maj, _, _)| maj)
    }

    /// Returns the minor version number, or None if unparseable.
    pub fn minor_version(&self) -> Option<u32> {
        self.version_tuple().map(|(_, min, _)| min)
    }
}

/// Parsed semantic version with comparison support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    /// Parse a "major.minor.patch" string into a `SemVer`.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }

    /// Returns `true` if this version satisfies a `^major.minor.patch` compatible range,
    /// i.e. same major version and >= the given version.
    pub fn is_compatible_with(&self, other: &SemVer) -> bool {
        if self.major != other.major {
            return false;
        }
        (self.minor, self.patch) >= (other.minor, other.patch)
    }

    /// Bump the patch component, returning a new `SemVer`.
    pub fn bump_patch(&self) -> Self {
        Self { major: self.major, minor: self.minor, patch: self.patch + 1 }
    }

    /// Bump the minor component (resets patch to 0), returning a new `SemVer`.
    pub fn bump_minor(&self) -> Self {
        Self { major: self.major, minor: self.minor + 1, patch: 0 }
    }

    /// Bump the major component (resets minor and patch to 0), returning a new `SemVer`.
    pub fn bump_major(&self) -> Self {
        Self { major: self.major + 1, minor: 0, patch: 0 }
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl ProductConfiguration {
    /// Parse the product version into a [`SemVer`].
    pub fn semver(&self) -> Option<SemVer> {
        SemVer::parse(&self.version)
    }

    /// Returns a compact "name@version" identifier string.
    pub fn name_at_version(&self) -> String {
        format!("{}@{}", self.name_short, self.version)
    }

    /// Returns the gallery service URL if configured, or `None`.
    pub fn gallery_service_url(&self) -> Option<&str> {
        self.extensions_gallery.as_ref().map(|g| g.service_url.as_str())
    }

    /// Produce a diagnostic string describing what is and isn't configured.
    pub fn diagnostics(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("product: {}", self.name_at_version()));
        lines.push(format!("quality: {}", self.quality.as_deref().unwrap_or("(none)")));
        lines.push(format!("gallery: {}", if self.has_gallery() { "yes" } else { "no" }));
        lines.push(format!("telemetry: {}", if self.enable_telemetry { "on" } else { "off" }));
        lines.push(format!("urls configured: {}", self.configured_url_count()));
        lines.join("\n")
    }
}

impl ExtensionsGallery {
    /// Build a URL for downloading a specific extension version.
    pub fn download_url(&self, publisher: &str, name: &str, version: &str) -> String {
        format!(
            "{}/publishers/{}/vsextensions/{}/{}",
            self.service_url, publisher, name, version
        )
    }

    /// Build a statistics URL for an extension.
    pub fn stats_url(&self, identifier: &str) -> String {
        format!("{}/extensionstatistics/{}", self.service_url, identifier)
    }
}

impl ProductStats {
    /// Returns the total elapsed time in milliseconds.
    pub fn total_time_ms(&self) -> f64 {
        self.total_time_ns as f64 / 1_000_000.0
    }

    /// Returns the number of successful operations.
    pub fn successes(&self) -> u64 {
        self.successful_operations
    }

    /// Returns the number of failed operations.
    pub fn failures(&self) -> u64 {
        self.failed_operations
    }

    /// Snapshot the current stats into a serialisable summary.
    pub fn snapshot(&self) -> StatsSummary {
        StatsSummary {
            total: self.total_operations,
            successes: self.successful_operations,
            failures: self.failed_operations,
            avg_ns: self.average_time_ns(),
            min_ns: self.min_time_ns(),
            max_ns: self.max_time_ns(),
        }
    }
}

/// A serialisable point-in-time summary of [`ProductStats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsSummary {
    pub total: u64,
    pub successes: u64,
    pub failures: u64,
    pub avg_ns: u64,
    pub min_ns: Option<u64>,
    pub max_ns: Option<u64>,
}

impl fmt::Display for StatsSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "total={} ok={} err={} avg_ns={}",
            self.total, self.successes, self.failures, self.avg_ns
        )
    }
}

/// Compare two version strings, returning the ordering.
/// Returns None if either version is not parseable.
pub fn compare_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let parse = |s: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 { return None; }
        Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
    };
    let va = parse(a)?;
    let vb = parse(b)?;
    Some(va.cmp(&vb))
}

/// Build a product identifier string: "application_name/version".
pub fn product_identifier(config: &ProductConfiguration) -> String {
    format!("{}/{}", config.application_name, config.version)
}

/// Check if two product configurations share the same gallery service URL.
pub fn same_gallery_service(a: &ProductConfiguration, b: &ProductConfiguration) -> bool {
    match (&a.extensions_gallery, &b.extensions_gallery) {
        (Some(ga), Some(gb)) => ga.service_url == gb.service_url,
        (None, None) => true,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// ProductFeatureFlagOverrides – override-based feature flag management
// ---------------------------------------------------------------------------

/// Manages overrides on top of the existing feature flag system.
pub struct ProductFeatureFlagOverrides {
    overrides: std::collections::HashMap<String, bool>,
}

impl ProductFeatureFlagOverrides {
    /// Create with no overrides.
    pub fn new() -> Self {
        Self { overrides: std::collections::HashMap::new() }
    }

    /// Set an override for a feature flag.
    pub fn set_override(&mut self, flag: impl Into<String>, enabled: bool) {
        self.overrides.insert(flag.into(), enabled);
    }

    /// Remove an override. Returns the previous override value.
    pub fn remove_override(&mut self, flag: &str) -> Option<bool> {
        self.overrides.remove(flag)
    }

    /// Resolve the effective value: override wins, then base, else default.
    pub fn resolve(&self, flag: &str, base: &ProductFeatureFlags) -> bool {
        if let Some(&v) = self.overrides.get(flag) {
            v
        } else {
            base.is_enabled(flag)
        }
    }

    /// Toggle an override. If no override exists, creates one as the opposite of base.
    pub fn toggle(&mut self, flag: &str, base: &ProductFeatureFlags) -> bool {
        let current = self.resolve(flag, base);
        let new_val = !current;
        self.overrides.insert(flag.to_string(), new_val);
        new_val
    }

    /// Number of active overrides.
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    /// Clear all overrides.
    pub fn clear(&mut self) {
        self.overrides.clear();
    }
}

// ---------------------------------------------------------------------------
// ProductUpdateChecker – version comparison
// ---------------------------------------------------------------------------

/// Checks whether a product update is available.
pub struct ProductUpdateChecker {
    current_version: String,
}

impl ProductUpdateChecker {
    /// Create a checker for the given current version.
    pub fn new(current_version: impl Into<String>) -> Self {
        Self { current_version: current_version.into() }
    }

    /// Parse a semver string into (major, minor, patch).
    fn parse(v: &str) -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() != 3 { return None; }
        Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
    }

    /// Check if the available version is newer than current.
    pub fn is_update_available(&self, available: &str) -> bool {
        match (Self::parse(&self.current_version), Self::parse(available)) {
            (Some(cur), Some(avail)) => avail > cur,
            _ => false,
        }
    }

    /// Determine if the update is major, minor, or patch.
    pub fn update_kind(&self, available: &str) -> Option<&'static str> {
        let cur = Self::parse(&self.current_version)?;
        let avail = Self::parse(available)?;
        if avail.0 > cur.0 { Some("major") }
        else if avail.1 > cur.1 { Some("minor") }
        else if avail.2 > cur.2 { Some("patch") }
        else { None }
    }

    /// Get the current version.
    pub fn current_version(&self) -> &str {
        &self.current_version
    }
}

// ---------------------------------------------------------------------------
// ProductLicenseValidator
// ---------------------------------------------------------------------------

/// Validates a product license key.
pub struct ProductLicenseValidator {
    prefix: String,
    expected_length: usize,
}

impl ProductLicenseValidator {
    /// Create a validator expecting keys like `"VSEDIT-XXXX-XXXX-XXXX"`.
    pub fn new(prefix: impl Into<String>, expected_length: usize) -> Self {
        Self { prefix: prefix.into(), expected_length }
    }

    /// Validate a license key format.
    pub fn validate(&self, key: &str) -> Result<(), String> {
        if !key.starts_with(&self.prefix) {
            return Err(format!("key must start with '{}'", self.prefix));
        }
        if key.len() != self.expected_length {
            return Err(format!("key must be {} characters", self.expected_length));
        }
        // Check that segments after prefix are alphanumeric
        let rest = &key[self.prefix.len()..];
        if !rest.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return Err("key contains invalid characters".into());
        }
        Ok(())
    }

    /// Check if a key is valid.
    pub fn is_valid(&self, key: &str) -> bool {
        self.validate(key).is_ok()
    }
}

// ---------------------------------------------------------------------------
// ProductTelemetryKeyManager
// ---------------------------------------------------------------------------

/// Manages telemetry instrumentation keys for the product.
pub struct ProductTelemetryKeyManager {
    keys: std::collections::HashMap<String, String>,
}

impl ProductTelemetryKeyManager {
    /// Create an empty key manager.
    pub fn new() -> Self {
        Self { keys: std::collections::HashMap::new() }
    }

    /// Register a telemetry key for a component.
    pub fn register_key(&mut self, component: impl Into<String>, key: impl Into<String>) {
        self.keys.insert(component.into(), key.into());
    }

    /// Get the key for a component.
    pub fn get_key(&self, component: &str) -> Option<&str> {
        self.keys.get(component).map(|s| s.as_str())
    }

    /// Remove a key.
    pub fn remove_key(&mut self, component: &str) -> bool {
        self.keys.remove(component).is_some()
    }

    /// Number of registered keys.
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// List all component names.
    pub fn components(&self) -> Vec<&str> {
        self.keys.keys().map(|s| s.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// ProductQuality
// ---------------------------------------------------------------------------

/// Represents the quality level of a product build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProductQuality {
    Stable,
    Insiders,
    Exploration,
    Development,
}

impl ProductQuality {
    /// Parse a quality level from a string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "stable" => Some(Self::Stable),
            "insiders" | "insider" => Some(Self::Insiders),
            "exploration" => Some(Self::Exploration),
            "development" | "dev" => Some(Self::Development),
            _ => None,
        }
    }

    /// Human-readable label for this quality level.
    pub fn label(&self) -> &str {
        match self {
            Self::Stable => "Stable",
            Self::Insiders => "Insiders",
            Self::Exploration => "Exploration",
            Self::Development => "Development",
        }
    }

    /// Returns `true` if this is the stable release channel.
    pub fn is_stable(&self) -> bool {
        matches!(self, Self::Stable)
    }

    /// Returns `true` for any non-stable quality level.
    pub fn is_prerelease(&self) -> bool {
        !self.is_stable()
    }

    /// Returns the update channel name used for this quality level.
    pub fn update_channel(&self) -> &str {
        match self {
            Self::Stable => "stable",
            Self::Insiders => "insiders",
            Self::Exploration => "exploration",
            Self::Development => "dev",
        }
    }
}

impl Default for ProductQuality {
    fn default() -> Self {
        Self::Stable
    }
}

impl fmt::Display for ProductQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------
// ProductBuildInfo
// ---------------------------------------------------------------------------

/// Metadata about a specific product build.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductBuildInfo {
    pub commit_sha: String,
    pub build_date: String,
    pub version: String,
    pub platform: String,
}

impl ProductBuildInfo {
    /// Create a new build info record.
    pub fn new(commit: &str, date: &str, version: &str, platform: &str) -> Self {
        Self {
            commit_sha: commit.to_string(),
            build_date: date.to_string(),
            version: version.to_string(),
            platform: platform.to_string(),
        }
    }

    /// Returns the first 8 characters of the commit SHA, or the full
    /// string if it is shorter than 8 characters.
    pub fn short_commit(&self) -> &str {
        if self.commit_sha.len() >= 8 {
            &self.commit_sha[..8]
        } else {
            &self.commit_sha
        }
    }

    /// A build is considered a development build when the commit SHA is
    /// empty or set to the placeholder value `"unknown"`.
    pub fn is_development_build(&self) -> bool {
        self.commit_sha.is_empty() || self.commit_sha == "unknown"
    }

    /// One-line human-readable summary of this build.
    pub fn summary(&self) -> String {
        format!(
            "{} ({}) on {} [{}]",
            self.version,
            self.short_commit(),
            self.platform,
            self.build_date
        )
    }
}

impl fmt::Display for ProductBuildInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// RemoteKind & ProductRemoteIndicator
// ---------------------------------------------------------------------------

/// The kind of remote session the product is running in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteKind {
    SSH,
    WSL,
    Container,
    Tunnel,
    Web,
}

impl fmt::Display for RemoteKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::SSH => "SSH",
            Self::WSL => "WSL",
            Self::Container => "Container",
            Self::Tunnel => "Tunnel",
            Self::Web => "Web",
        };
        f.write_str(label)
    }
}

/// Tracks whether the product is running in a remote session.
#[derive(Debug, Clone)]
pub struct ProductRemoteIndicator {
    kind: Option<RemoteKind>,
}

impl ProductRemoteIndicator {
    /// Create a new indicator with no remote session.
    pub fn new() -> Self {
        Self { kind: None }
    }

    /// Mark the product as running in the given remote session type.
    pub fn set_remote(&mut self, kind: RemoteKind) {
        self.kind = Some(kind);
    }

    /// Returns `true` if a remote session is active.
    pub fn is_remote(&self) -> bool {
        self.kind.is_some()
    }

    /// Returns the kind of remote session, if any.
    pub fn remote_kind(&self) -> Option<&RemoteKind> {
        self.kind.as_ref()
    }

    /// Human-readable label describing the current state.
    pub fn label(&self) -> &str {
        match &self.kind {
            Some(RemoteKind::SSH) => "Remote (SSH)",
            Some(RemoteKind::WSL) => "Remote (WSL)",
            Some(RemoteKind::Container) => "Remote (Container)",
            Some(RemoteKind::Tunnel) => "Remote (Tunnel)",
            Some(RemoteKind::Web) => "Remote (Web)",
            None => "Local",
        }
    }
}

impl Default for ProductRemoteIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProductRemoteIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------
// ProductCapabilityMatrix
// ---------------------------------------------------------------------------

/// Tracks a set of named capabilities and whether each is enabled.
#[derive(Debug, Clone)]
pub struct ProductCapabilityMatrix {
    capabilities: std::collections::HashMap<String, bool>,
}

impl ProductCapabilityMatrix {
    /// Create an empty capability matrix.
    pub fn new() -> Self {
        Self {
            capabilities: std::collections::HashMap::new(),
        }
    }

    /// Register a capability with an initial enabled/disabled state.
    pub fn register(&mut self, capability: &str, enabled: bool) {
        self.capabilities.insert(capability.to_string(), enabled);
    }

    /// Returns `true` if the capability exists and is enabled.
    pub fn is_enabled(&self, capability: &str) -> bool {
        self.capabilities.get(capability).copied().unwrap_or(false)
    }

    /// Enable a previously registered capability (no-op if not registered).
    pub fn enable(&mut self, capability: &str) {
        if let Some(v) = self.capabilities.get_mut(capability) {
            *v = true;
        }
    }

    /// Disable a previously registered capability (no-op if not registered).
    pub fn disable(&mut self, capability: &str) {
        if let Some(v) = self.capabilities.get_mut(capability) {
            *v = false;
        }
    }

    /// Returns the names of all enabled capabilities.
    pub fn enabled_capabilities(&self) -> Vec<&str> {
        let mut result: Vec<&str> = self
            .capabilities
            .iter()
            .filter(|(_, v)| **v)
            .map(|(k, _)| k.as_str())
            .collect();
        result.sort();
        result
    }

    /// Returns all capabilities with their enabled/disabled state.
    pub fn all_capabilities(&self) -> Vec<(&str, bool)> {
        let mut result: Vec<(&str, bool)> = self
            .capabilities
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        result.sort_by_key(|(k, _)| *k);
        result
    }

    /// Total number of registered capabilities.
    pub fn count(&self) -> usize {
        self.capabilities.len()
    }
}

impl Default for ProductCapabilityMatrix {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProductCapabilityMatrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let enabled = self.enabled_capabilities().len();
        write!(f, "{}/{} capabilities enabled", enabled, self.count())
    }
}


// ─── ProdB Builder & Validator ─────────────────────────────

/// Builder for constructing product configurations.
#[derive(Debug, Clone)]
pub struct ProdBBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl ProdBBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<ProdBCfg, ProdBBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(ProdBBuildErr { errors }); }
        Ok(ProdBCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated product configuration.
#[derive(Debug, Clone)]
pub struct ProdBCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl ProdBCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &ProdBCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for ProdBCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProdBCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct ProdBBuildErr { pub errors: Vec<String> }

impl fmt::Display for ProdBBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ProdBBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for ProdBBuildErr {}

// ─── ProdF Formatter ───────────────────────────────────────

/// Formatting options for product output.
#[derive(Debug, Clone)]
pub struct ProdFFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for ProdFFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl ProdFFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for product data.
pub struct ProdFFmt {
    options: ProdFFmtOpts,
}

impl ProdFFmt {
    pub fn new(options: ProdFFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: ProdFFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


/// Product info configuration manager.
#[derive(Debug, Clone)]
pub struct ProductConfig {
    entries: Vec<ProductEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single product info entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl ProductEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl ProductConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: ProductEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&ProductEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ProductEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&ProductEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&ProductEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&ProductEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<ProductEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Product metadata and branding — extended utilities (qg)
// ---------------------------------------------------------------------------

/// Metric accumulator for product operations.
#[derive(Debug, Clone)]
pub struct QgMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QgMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for product.
#[derive(Debug, Clone)]
pub struct QgRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QgRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for product lookups.
#[derive(Debug, Clone)]
pub struct QgLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QgLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for product
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaProductRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaProductRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaProductCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaProductCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaProductCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 142
// ---------------------------------------------------------------------------

/// Generic object pool `Xc142Pool<T>`.
pub struct Xc142Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc142Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc142PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc142Pool<T> {
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
    pub fn stats(&self) -> Xc142PoolStats {
        Xc142PoolStats {
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

impl<T> Default for Xc142Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc142Scheduler`.
pub struct Xc142Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc142Scheduler {
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

impl Default for Xc142Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_142 hash for the given byte slice.
pub fn xc_142_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_142 convention.
pub fn xc_142_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe5 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe5Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe5PipelineError {
    pub stage: Xe5Stage,
    pub message: String,
}

impl std::fmt::Display for Xe5PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe5Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe5Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError>>>,
    stage_names: Vec<Xe5Stage>,
}

impl Xe5Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe5Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe5Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe5Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe5Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> {
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

    pub fn compose(mut self, other: Xe5Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe5CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe5CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe5Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe5CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe5CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe5Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe5CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_5_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe5CacheEntry {
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

    fn xe_5_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe5CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_5_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> {
    Ok(data)
}

pub fn xe_5_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_5_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_5_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_5_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe5PipelineError> {
    Err(Xe5PipelineError {
        stage: Xe5Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #68
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf68Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf68TrieNode {
    children: std::collections::HashMap<char, Xf68TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf68Trie {
    root: Xf68TrieNode,
    count: usize,
}

impl Xf68Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf68TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf68TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf68TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf68BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf68BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 141).
pub struct Xh141SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh141SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 183 as u64,
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

/// A compact bit set supporting boolean operations (variant 141).
pub struct Xh141BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh141BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 141).
pub struct Xi141Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi141Deque<T> {
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
pub struct Xi141Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi141Interval {
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

/// A simple interval tree (variant 141).
pub struct Xi141IntervalTree {
    xi_intervals: Vec<Xi141Interval>,
}

impl Xi141IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi141Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi141Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi141Interval) -> Vec<&Xi141Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi141Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi141Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi141Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi141Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi141Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi141Interval> = Vec::new();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = ProductConfiguration::default_config();
        assert_eq!(cfg.name_short, "vsedit");
        assert_eq!(cfg.application_name, "vsedit");
        assert!(!cfg.enable_telemetry);
    }

    #[test]
    fn roundtrip_json() {
        let cfg = ProductConfiguration::default_config();
        let json = cfg.to_json();
        let cfg2 = ProductConfiguration::from_json(&json).unwrap();
        assert_eq!(cfg.name_short, cfg2.name_short);
        assert_eq!(cfg.version, cfg2.version);
    }

    #[test]
    fn parse_custom_json() {
        let json = r#"{
            "nameShort": "custom",
            "nameLong": "Custom Editor",
            "applicationName": "custom-editor",
            "dataFolderName": ".custom",
            "version": "2.0.0",
            "enableTelemetry": true
        }"#;
        let cfg = ProductConfiguration::from_json(json).unwrap();
        assert_eq!(cfg.name_short, "custom");
        assert!(cfg.enable_telemetry);
        assert!(cfg.extensions_gallery.is_none());
    }

    #[test]
    fn gallery_config() {
        let cfg = ProductConfiguration::default_config();
        let gallery = cfg.extensions_gallery.unwrap();
        assert!(gallery.service_url.contains("marketplace"));
    }

    #[test]
    fn is_stable_default() {
        let cfg = ProductConfiguration::default_config();
        assert!(cfg.is_stable());
        assert!(!cfg.is_insider());
    }

    #[test]
    fn is_insider() {
        let mut cfg = ProductConfiguration::default_config();
        cfg.quality = Some("insider".to_string());
        assert!(cfg.is_insider());
        assert!(!cfg.is_stable());
    }

    #[test]
    fn display_name_format() {
        let cfg = ProductConfiguration::default_config();
        let expected = format!("Visual Studio Edit {}", cfg.version);
        assert_eq!(cfg.display_name(), expected);
    }

    #[test]
    fn has_gallery() {
        let cfg = ProductConfiguration::default_config();
        assert!(cfg.has_gallery());

        let mut cfg2 = cfg;
        cfg2.extensions_gallery = None;
        assert!(!cfg2.has_gallery());
    }

    #[test]
    fn display_trait() {
        let cfg = ProductConfiguration::default_config();
        let displayed = format!("{}", cfg);
        assert!(displayed.starts_with("vsedit "));
    }

    #[test]
    fn gallery_has_control_url() {
        let gallery = ExtensionsGallery {
            service_url: "https://example.com".to_string(),
            item_url: "https://example.com/items".to_string(),
            control_url: Some("https://example.com/control".to_string()),
        };
        assert!(gallery.has_control_url());

        let gallery_no_ctrl = ExtensionsGallery {
            service_url: "https://example.com".to_string(),
            item_url: "https://example.com/items".to_string(),
            control_url: None,
        };
        assert!(!gallery_no_ctrl.has_control_url());
    }

    #[test]
    fn gallery_search_url() {
        let gallery = ExtensionsGallery {
            service_url: "https://marketplace.visualstudio.com/_apis/public/gallery".to_string(),
            item_url: "https://marketplace.visualstudio.com/items".to_string(),
            control_url: None,
        };
        let url = gallery.search_url("rust");
        assert!(url.contains("extensionquery?query=rust"));
        assert!(url.starts_with("https://marketplace.visualstudio.com"));
    }

    #[test]
    fn merge_from_json_partial() {
        let mut cfg = ProductConfiguration::default_config();
        assert_eq!(cfg.name_short, "vsedit");

        cfg.merge_from_json(r#"{"nameShort": "merged"}"#).unwrap();
        assert_eq!(cfg.name_short, "merged");
        // Other fields should remain unchanged.
        assert_eq!(cfg.name_long, "Visual Studio Edit");
    }

    #[test]
    fn new_optional_fields_default() {
        let json = r#"{
            "nameShort": "test",
            "nameLong": "Test Editor",
            "applicationName": "test-editor",
            "dataFolderName": ".test",
            "version": "1.0.0"
        }"#;
        let cfg = ProductConfiguration::from_json(json).unwrap();
        assert!(cfg.update_url.is_none());
        assert!(cfg.license_url.is_none());
        assert!(cfg.settings_sync_url.is_none());
    }

    #[test]
    fn builder_success() {
        let cfg = ProductConfiguration::builder()
            .name_short("myapp")
            .name_long("My Application")
            .application_name("myapp")
            .version("1.2.3")
            .quality("stable")
            .build()
            .unwrap();
        assert_eq!(cfg.name_short, "myapp");
        assert_eq!(cfg.data_folder_name, ".myapp");
        assert!(cfg.is_stable());
    }

    #[test]
    fn builder_missing_required_field() {
        let result = ProductConfiguration::builder()
            .name_short("myapp")
            .build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("name_long is required"));
    }

    #[test]
    fn builder_empty_name_short() {
        let result = ProductConfiguration::builder()
            .name_short("")
            .name_long("Test")
            .application_name("test")
            .version("1.0.0")
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must not be empty"));
    }

    #[test]
    fn validate_default_config() {
        let cfg = ProductConfiguration::default_config();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_empty_name() {
        let mut cfg = ProductConfiguration::default_config();
        cfg.name_short = String::new();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn version_tuple_parsing() {
        let cfg = ProductConfiguration::builder()
            .name_short("t")
            .name_long("T")
            .application_name("t")
            .version("3.14.159")
            .build()
            .unwrap();
        assert_eq!(cfg.version_tuple(), Some((3, 14, 159)));
    }

    #[test]
    fn version_tuple_invalid() {
        let mut cfg = ProductConfiguration::default_config();
        cfg.version = "not-a-version".to_string();
        assert_eq!(cfg.version_tuple(), None);
    }

    #[test]
    fn is_newer_than_comparison() {
        let mut cfg = ProductConfiguration::default_config();
        cfg.version = "2.0.0".to_string();
        assert_eq!(cfg.is_newer_than("1.9.99"), Some(true));
        assert_eq!(cfg.is_newer_than("2.0.0"), Some(false));
        assert_eq!(cfg.is_newer_than("3.0.0"), Some(false));
        assert_eq!(cfg.is_newer_than("bad"), None);
    }

    #[test]
    fn user_agent_format() {
        let cfg = ProductConfiguration::default_config();
        let ua = cfg.user_agent();
        assert!(ua.starts_with("vsedit/"));
        assert!(ua.ends_with("-stable"));
    }

    #[test]
    fn user_agent_no_quality() {
        let mut cfg = ProductConfiguration::default_config();
        cfg.quality = None;
        let ua = cfg.user_agent();
        assert!(!ua.contains('-'));
    }

    #[test]
    fn configured_url_count_default() {
        let cfg = ProductConfiguration::default_config();
        assert_eq!(cfg.configured_url_count(), 0);
    }

    #[test]
    fn configured_url_count_with_urls() {
        let mut cfg = ProductConfiguration::default_config();
        cfg.report_issue_url = Some("https://example.com/issues".into());
        cfg.license_url = Some("https://example.com/license".into());
        assert_eq!(cfg.configured_url_count(), 2);
    }

    #[test]
    fn product_error_display() {
        let err = ProductError::ValidationError("test error".into());
        assert_eq!(err.to_string(), "Validation error: test error");
    }

    #[test]
    fn partial_eq_configs() {
        let a = ProductConfiguration::default_config();
        let b = ProductConfiguration::default_config();
        assert_eq!(a, b);
    }

    #[test]
    fn gallery_display_trait() {
        let gallery = ExtensionsGallery {
            service_url: "https://example.com/gallery".to_string(),
            item_url: "https://example.com/items".to_string(),
            control_url: None,
        };
        let display = format!("{}", gallery);
        assert!(display.contains("example.com/gallery"));
    }

    #[test]
    fn gallery_item_detail_url() {
        let gallery = ExtensionsGallery {
            service_url: "https://example.com".to_string(),
            item_url: "https://example.com/items".to_string(),
            control_url: None,
        };
        let url = gallery.item_detail_url("publisher.extension");
        assert_eq!(url, "https://example.com/items?itemName=publisher.extension");
    }

    #[test]
    fn product_stats_new_defaults() {
        let stats = ProductStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn product_stats_record_success() {
        let mut stats = ProductStats::new();
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
    fn product_stats_record_failure() {
        let mut stats = ProductStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn product_stats_reset() {
        let mut stats = ProductStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn product_stats_merge() {
        let mut a = ProductStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ProductStats::new();
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
    fn product_stats_display() {
        let mut stats = ProductStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn product_stats_default() {
        let stats = ProductStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn product_validator_accepts_valid_name() {
        let v = ProductValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn product_validator_rejects_empty() {
        let v = ProductValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn product_validator_rejects_too_long() {
        let v = ProductValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn product_validator_forbidden_prefix() {
        let v = ProductValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn product_validator_allowed_chars() {
        let v = ProductValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn product_validator_range() {
        let v = ProductValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn product_sanitize_removes_control() {
        let result = ProductValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn product_truncate_short_string() {
        assert_eq!(ProductValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn product_truncate_long_string() {
        let result = ProductValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn product_is_ascii_printable() {
        assert!(ProductValidator::is_ascii_printable("Hello World 123"));
        assert!(!ProductValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn feature_flags_set_and_check() {
        let mut flags = ProductFeatureFlags::new();
        flags.set("copilot", true);
        flags.set("ai-chat", false);
        assert!(flags.is_enabled("copilot"));
        assert!(!flags.is_enabled("ai-chat"));
        assert!(!flags.is_enabled("unknown"));
    }

    #[test]
    fn feature_flags_enabled_list() {
        let mut flags = ProductFeatureFlags::new();
        flags.set("a", true);
        flags.set("b", false);
        flags.set("c", true);
        let enabled = flags.enabled_features();
        assert_eq!(enabled.len(), 2);
    }

    #[test]
    fn feature_flags_remove() {
        let mut flags = ProductFeatureFlags::new();
        flags.set("x", true);
        assert_eq!(flags.remove("x"), Some(true));
        assert_eq!(flags.len(), 0);
    }

    #[test]
    fn feature_flags_display() {
        let flags = ProductFeatureFlags::new();
        assert_eq!(flags.to_string(), "FeatureFlags(0/0 enabled)");
    }

    #[test]
    fn update_channel_from_quality() {
        let mut config = ProductConfiguration::default_config();
        config.quality = Some("insider".into());
        assert_eq!(product_update_channel(&config), UpdateChannel::Insiders);
        config.quality = Some("dev".into());
        assert_eq!(product_update_channel(&config), UpdateChannel::Dev);
        config.quality = None;
        assert_eq!(product_update_channel(&config), UpdateChannel::Stable);
    }

    #[test]
    fn update_channel_display() {
        assert_eq!(UpdateChannel::Stable.to_string(), "stable");
        assert_eq!(UpdateChannel::Insiders.to_string(), "insiders");
        assert_eq!(UpdateChannel::Dev.to_string(), "dev");
    }

    #[test]
    fn telemetry_config_enabled() {
        let mut config = ProductConfiguration::default_config();
        config.enable_telemetry = true;
        let tc = product_telemetry_config(&config);
        assert!(tc.enabled);
        assert_eq!(tc.level, TelemetryLevel::All);
    }

    #[test]
    fn telemetry_config_disabled() {
        let mut config = ProductConfiguration::default_config();
        config.enable_telemetry = false;
        let tc = product_telemetry_config(&config);
        assert!(!tc.enabled);
        assert_eq!(tc.level, TelemetryLevel::Off);
    }

    #[test]
    fn license_type_open_source() {
        assert!(LicenseType::Mit.is_open_source());
        assert!(LicenseType::Apache2.is_open_source());
        assert!(LicenseType::Gpl3.is_open_source());
        assert!(!LicenseType::Proprietary.is_open_source());
        assert!(!LicenseType::Custom.is_open_source());
    }

    #[test]
    fn product_license_copyright_notice() {
        let lic = ProductLicense::new(LicenseType::Mit, "Acme Corp", 2024);
        let notice = lic.copyright_notice();
        assert!(notice.contains("2024"));
        assert!(notice.contains("Acme Corp"));
        assert!(notice.contains("MIT"));
        assert_eq!(lic.spdx_id, Some("MIT".to_string()));
    }

    #[test]
    fn product_license_validate() {
        let good = ProductLicense::new(LicenseType::Apache2, "Dev", 2024);
        assert!(good.validate().is_ok());

        let bad_holder = ProductLicense::new(LicenseType::Mit, "", 2024);
        assert!(bad_holder.validate().is_err());

        let bad_year = ProductLicense::new(LicenseType::Mit, "Dev", 1900);
        assert!(bad_year.validate().is_err());
    }

    #[test]
    fn feature_flag_evaluator_always() {
        let config = ProductConfiguration::default_config();
        let mut eval = FeatureFlagEvaluator::new();
        eval.add_rule("on", FeatureCondition::Always(true));
        eval.add_rule("off", FeatureCondition::Always(false));
        let flags = eval.evaluate(&config);
        assert!(flags.is_enabled("on"));
        assert!(!flags.is_enabled("off"));
    }

    #[test]
    fn feature_flag_evaluator_channel_and_version() {
        let mut config = ProductConfiguration::default_config();
        config.quality = Some("insider".to_string());
        config.version = "2.5.0".to_string();

        let mut eval = FeatureFlagEvaluator::new();
        eval.add_rule("insider-only", FeatureCondition::Channel(UpdateChannel::Insiders));
        eval.add_rule("stable-only", FeatureCondition::Channel(UpdateChannel::Stable));
        eval.add_rule("v2-plus", FeatureCondition::MinVersion(2, 0, 0));
        eval.add_rule("v3-plus", FeatureCondition::MinVersion(3, 0, 0));

        let flags = eval.evaluate(&config);
        assert!(flags.is_enabled("insider-only"));
        assert!(!flags.is_enabled("stable-only"));
        assert!(flags.is_enabled("v2-plus"));
        assert!(!flags.is_enabled("v3-plus"));
    }

    #[test]
    fn feature_flag_evaluator_composite() {
        let mut config = ProductConfiguration::default_config();
        config.quality = Some("stable".to_string());
        config.version = "1.5.0".to_string();

        let mut eval = FeatureFlagEvaluator::new();
        eval.add_rule(
            "stable-v1",
            FeatureCondition::AllOf(vec![
                FeatureCondition::Channel(UpdateChannel::Stable),
                FeatureCondition::MinVersion(1, 0, 0),
            ]),
        );
        eval.add_rule(
            "insider-or-v2",
            FeatureCondition::AnyOf(vec![
                FeatureCondition::Channel(UpdateChannel::Insiders),
                FeatureCondition::MinVersion(2, 0, 0),
            ]),
        );

        let flags = eval.evaluate(&config);
        assert!(flags.is_enabled("stable-v1"));
        assert!(!flags.is_enabled("insider-or-v2"));
    }

    #[test]
    fn compatibility_same_product() {
        let a = ProductConfiguration::default_config();
        let b = ProductConfiguration::default_config();
        let report = check_compatibility(&a, &b);
        assert!(report.compatible);
        assert!(!report.has_warnings());
        assert!(report.to_string().contains("Compatible"));
    }

    #[test]
    fn compatibility_different_app_name() {
        let a = ProductConfiguration::default_config();
        let mut b = ProductConfiguration::default_config();
        b.application_name = "other-app".to_string();
        let report = check_compatibility(&a, &b);
        assert!(!report.compatible);
        assert!(report.has_warnings());
        assert!(report.warnings.iter().any(|w| w.contains("application name")));
    }

    #[test]
    fn configured_urls_returns_set_urls() {
        let mut cfg = ProductConfiguration::default_config();
        cfg.report_issue_url = Some("https://example.com/issue".into());
        let urls = cfg.configured_urls();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].0, "report_issue_url");
    }

    #[test]
    fn summary_line_with_quality() {
        let cfg = ProductConfiguration::default_config();
        let summary = cfg.summary_line();
        assert!(summary.contains("vsedit"));
        assert!(summary.contains(&cfg.version));
    }

    #[test]
    fn summary_line_without_quality() {
        let mut cfg = ProductConfiguration::default_config();
        cfg.quality = None;
        let summary = cfg.summary_line();
        assert!(!summary.contains('('));
    }

    #[test]
    fn has_commit_and_date() {
        let mut cfg = ProductConfiguration::default_config();
        assert!(!cfg.has_commit());
        assert!(!cfg.has_date());
        cfg.commit = Some("abc123".into());
        cfg.date = Some("2024-01-01".into());
        assert!(cfg.has_commit());
        assert!(cfg.has_date());
    }

    #[test]
    fn major_minor_version() {
        let cfg = ProductConfiguration::default_config();
        assert!(cfg.major_version().is_some());
        assert!(cfg.minor_version().is_some());
    }

    #[test]
    fn compare_versions_ordering() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Some(std::cmp::Ordering::Equal));
        assert_eq!(compare_versions("1.0.0", "2.0.0"), Some(std::cmp::Ordering::Less));
        assert_eq!(compare_versions("2.1.0", "1.9.9"), Some(std::cmp::Ordering::Greater));
        assert_eq!(compare_versions("bad", "1.0.0"), None);
    }

    #[test]
    fn product_identifier_format() {
        let cfg = ProductConfiguration::default_config();
        let id = product_identifier(&cfg);
        assert!(id.contains('/'));
        assert!(id.starts_with(&cfg.application_name));
    }

    #[test]
    fn same_gallery_service_check() {
        let a = ProductConfiguration::default_config();
        let b = ProductConfiguration::default_config();
        assert!(same_gallery_service(&a, &b));

        let mut c = ProductConfiguration::default_config();
        c.extensions_gallery = None;
        assert!(!same_gallery_service(&a, &c));
    }

    // -- ProductFeatureFlagOverrides tests --

    #[test]
    fn feature_flag_overrides_basic() {
        let mut base = ProductFeatureFlags::new();
        base.set("dark_mode", true);
        base.set("beta", false);

        let mut ov = ProductFeatureFlagOverrides::new();
        assert!(ov.resolve("dark_mode", &base)); // base value
        ov.set_override("dark_mode", false);
        assert!(!ov.resolve("dark_mode", &base)); // overridden
        assert_eq!(ov.override_count(), 1);
    }

    #[test]
    fn feature_flag_overrides_toggle() {
        let mut base = ProductFeatureFlags::new();
        base.set("feature", true);
        let mut ov = ProductFeatureFlagOverrides::new();
        let new_val = ov.toggle("feature", &base);
        assert!(!new_val);
        assert!(!ov.resolve("feature", &base));
    }

    #[test]
    fn feature_flag_overrides_clear() {
        let base = ProductFeatureFlags::new();
        let mut ov = ProductFeatureFlagOverrides::new();
        ov.set_override("a", true);
        ov.set_override("b", false);
        ov.clear();
        assert_eq!(ov.override_count(), 0);
        assert!(!ov.resolve("a", &base)); // back to base default (false)
    }

    // -- ProductUpdateChecker tests --

    #[test]
    fn update_checker_newer() {
        let checker = ProductUpdateChecker::new("1.0.0");
        assert!(checker.is_update_available("1.1.0"));
        assert!(checker.is_update_available("2.0.0"));
        assert!(!checker.is_update_available("1.0.0"));
        assert!(!checker.is_update_available("0.9.0"));
    }

    #[test]
    fn update_checker_kind() {
        let checker = ProductUpdateChecker::new("1.5.3");
        assert_eq!(checker.update_kind("2.0.0"), Some("major"));
        assert_eq!(checker.update_kind("1.6.0"), Some("minor"));
        assert_eq!(checker.update_kind("1.5.4"), Some("patch"));
        assert_eq!(checker.update_kind("1.5.3"), None);
    }

    // -- ProductLicenseValidator tests --

    #[test]
    fn license_valid() {
        let v = ProductLicenseValidator::new("VSEDIT-", 21);
        assert!(v.is_valid("VSEDIT-ABCD-1234-EFGH"));
    }

    #[test]
    fn license_invalid_prefix() {
        let v = ProductLicenseValidator::new("VSEDIT-", 21);
        let result = v.validate("WRONG--ABCD-1234-EFGH");
        assert!(result.is_err());
    }

    #[test]
    fn license_invalid_length() {
        let v = ProductLicenseValidator::new("VSEDIT-", 21);
        assert!(!v.is_valid("VSEDIT-SHORT"));
    }

    // -- ProductQuality tests --

    #[test]
    fn quality_default_is_stable() {
        let q = ProductQuality::default();
        assert!(q.is_stable());
        assert!(!q.is_prerelease());
        assert_eq!(q.label(), "Stable");
    }

    #[test]
    fn quality_from_str_variants() {
        assert_eq!(ProductQuality::from_str("stable"), Some(ProductQuality::Stable));
        assert_eq!(ProductQuality::from_str("Insiders"), Some(ProductQuality::Insiders));
        assert_eq!(ProductQuality::from_str("insider"), Some(ProductQuality::Insiders));
        assert_eq!(ProductQuality::from_str("dev"), Some(ProductQuality::Development));
        assert_eq!(ProductQuality::from_str("exploration"), Some(ProductQuality::Exploration));
        assert_eq!(ProductQuality::from_str("unknown"), None);
    }

    #[test]
    fn quality_update_channels() {
        assert_eq!(ProductQuality::Stable.update_channel(), "stable");
        assert_eq!(ProductQuality::Insiders.update_channel(), "insiders");
        assert_eq!(ProductQuality::Development.update_channel(), "dev");
    }

    #[test]
    fn quality_display() {
        assert_eq!(format!("{}", ProductQuality::Exploration), "Exploration");
    }

    // -- ProductBuildInfo tests --

    #[test]
    fn build_info_short_commit() {
        let info = ProductBuildInfo::new("abc12345def", "2024-01-15", "1.0.0", "linux-x64");
        assert_eq!(info.short_commit(), "abc12345");
        assert!(!info.is_development_build());
    }

    #[test]
    fn build_info_dev_build() {
        let info = ProductBuildInfo::new("unknown", "2024-01-15", "0.0.0-dev", "linux-x64");
        assert!(info.is_development_build());

        let empty = ProductBuildInfo::new("", "2024-01-15", "0.0.0-dev", "linux-x64");
        assert!(empty.is_development_build());
    }

    #[test]
    fn build_info_summary() {
        let info = ProductBuildInfo::new("abc12345def", "2024-01-15", "1.0.0", "linux-x64");
        let s = info.summary();
        assert!(s.contains("1.0.0"));
        assert!(s.contains("abc12345"));
        assert!(s.contains("linux-x64"));
    }

    // -- ProductRemoteIndicator tests --

    #[test]
    fn remote_indicator_local_by_default() {
        let ri = ProductRemoteIndicator::new();
        assert!(!ri.is_remote());
        assert_eq!(ri.remote_kind(), None);
        assert_eq!(ri.label(), "Local");
    }

    #[test]
    fn remote_indicator_set_ssh() {
        let mut ri = ProductRemoteIndicator::new();
        ri.set_remote(RemoteKind::SSH);
        assert!(ri.is_remote());
        assert_eq!(ri.remote_kind(), Some(&RemoteKind::SSH));
        assert_eq!(ri.label(), "Remote (SSH)");
    }

    #[test]
    fn remote_kind_display() {
        assert_eq!(format!("{}", RemoteKind::Container), "Container");
        assert_eq!(format!("{}", RemoteKind::Web), "Web");
    }

    // -- ProductCapabilityMatrix tests --

    #[test]
    fn capability_matrix_register_and_query() {
        let mut m = ProductCapabilityMatrix::new();
        m.register("terminal", true);
        m.register("debug", false);
        assert!(m.is_enabled("terminal"));
        assert!(!m.is_enabled("debug"));
        assert!(!m.is_enabled("nonexistent"));
        assert_eq!(m.count(), 2);
    }

    #[test]
    fn capability_matrix_enable_disable() {
        let mut m = ProductCapabilityMatrix::new();
        m.register("git", false);
        assert!(!m.is_enabled("git"));
        m.enable("git");
        assert!(m.is_enabled("git"));
        m.disable("git");
        assert!(!m.is_enabled("git"));
    }

    #[test]
    fn capability_matrix_lists() {
        let mut m = ProductCapabilityMatrix::new();
        m.register("alpha", true);
        m.register("beta", false);
        m.register("gamma", true);
        let enabled = m.enabled_capabilities();
        assert_eq!(enabled, vec!["alpha", "gamma"]);
        let all = m.all_capabilities();
        assert_eq!(all.len(), 3);
    }

    // -- ProductTelemetryKeyManager tests --

    #[test]
    fn telemetry_key_manager() {
        let mut m = ProductTelemetryKeyManager::new();
        m.register_key("editor", "key-123");
        m.register_key("terminal", "key-456");
        assert_eq!(m.get_key("editor"), Some("key-123"));
        assert_eq!(m.key_count(), 2);
        assert!(m.remove_key("editor"));
        assert_eq!(m.key_count(), 1);
    }

    #[test]
    fn telemetry_key_missing() {
        let m = ProductTelemetryKeyManager::new();
        assert_eq!(m.get_key("nope"), None);
    }

    #[test]
    fn prodb_builder_valid() {
        let cfg = ProdBBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn prodb_builder_empty_name() {
        let r = ProdBBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn prodb_builder_bad_priority() {
        assert!(ProdBBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn prodb_builder_zero_max() {
        assert!(ProdBBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn prodb_cfg_merge() {
        let mut a = ProdBBuilder::new("a").property("x", "1").build().unwrap();
        let b = ProdBBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn prodb_cfg_display() {
        let cfg = ProdBBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

    #[test]
    fn prodf_fmt_list() {
        let f = ProdFFmt::new(ProdFFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn prodf_fmt_kv() {
        let f = ProdFFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn prodf_fmt_section() {
        let f = ProdFFmt::new(ProdFFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn prodf_fmt_truncate() {
        let f = ProdFFmt::new(ProdFFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn prodf_fmt_opts_defaults() {
        let o = ProdFFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn product_entry_creation() {
        let e = ProductEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn product_entry_with_priority() {
        let e = ProductEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn product_entry_metadata() {
        let e = ProductEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn product_entry_remove_meta() {
        let mut e = ProductEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn product_entry_activate_deactivate() {
        let mut e = ProductEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn product_config_add_sorted() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("lo", "Lo").with_priority(1));
        c.add(ProductEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn product_config_capacity() {
        let mut c = ProductConfig::new(1);
        assert!(c.add(ProductEntry::new("a", "A")));
        assert!(!c.add(ProductEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn product_config_remove() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn product_config_get() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn product_config_active_entries() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("a", "A"));
        c.add(ProductEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn product_config_enable_disable() {
        let mut c = ProductConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn product_config_clear() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn product_config_find_by_label() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn product_config_top_n() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("a", "A").with_priority(1));
        c.add(ProductEntry::new("b", "B").with_priority(2));
        c.add(ProductEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn product_config_deactivate_activate_all() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("a", "A"));
        c.add(ProductEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn product_config_highest_priority() {
        let mut c = ProductConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(ProductEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn product_config_contains() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn product_config_labels() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("a", "Alpha"));
        c.add(ProductEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn product_config_drain_inactive() {
        let mut c = ProductConfig::new(10);
        c.add(ProductEntry::new("a", "A"));
        c.add(ProductEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qg_metrics_empty() {
        let m = QgMetrics::new("product");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qg_metrics_record_and_mean() {
        let mut m = QgMetrics::new("product");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qg_metrics_min_max() {
        let mut m = QgMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qg_metrics_variance_and_std() {
        let mut m = QgMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qg_metrics_percentile() {
        let mut m = QgMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qg_metrics_merge() {
        let mut a = QgMetrics::new("a");
        a.record(1.0);
        let mut b = QgMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qg_metrics_reset() {
        let mut m = QgMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qg_rate_window_empty() {
        let rw = QgRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qg_rate_window_tick_and_rate() {
        let mut rw = QgRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qg_lru_cache_basic() {
        let mut c = QgLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qg_lru_cache_contains_and_keys() {
        let mut c = QgLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qg_lru_cache_remove() {
        let mut c = QgLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qg_metrics_sum() {
        let mut m = QgMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qg_metrics_label() {
        let m = QgMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qg_lru_cache_clear() {
        let mut c = QgLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for product
    #[test]
    fn xa_product_ring_new() {
        let rb = super::XaProductRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_product_ring_push_len() {
        let mut rb = super::XaProductRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_product_ring_wrap() {
        let mut rb = super::XaProductRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_product_ring_mean_empty() {
        let rb = super::XaProductRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_product_ring_mean_values() {
        let mut rb = super::XaProductRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_product_ring_min_max() {
        let mut rb = super::XaProductRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_product_ring_iter() {
        let mut rb = super::XaProductRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_product_counter_new() {
        let c = super::XaProductCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_product_counter_inc() {
        let mut c = super::XaProductCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_product_counter_inc_by() {
        let mut c = super::XaProductCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_product_counter_reset() {
        let mut c = super::XaProductCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_product_counter_clear() {
        let mut c = super::XaProductCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_product_counter_default() {
        let c = super::XaProductCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 142 ----

    #[test]
    fn xc_142_pool_new_empty() {
        let pool: super::Xc142Pool<i32> = super::Xc142Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_142_pool_release_acquire() {
        let mut pool = super::Xc142Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_142_pool_acquire_empty() {
        let mut pool: super::Xc142Pool<i32> = super::Xc142Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_142_pool_full() {
        let mut pool = super::Xc142Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_142_pool_drain() {
        let mut pool = super::Xc142Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_142_pool_stats() {
        let mut pool = super::Xc142Pool::new(8);
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
    fn xc_142_pool_clear() {
        let mut pool = super::Xc142Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_142_pool_shrink() {
        let mut pool = super::Xc142Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_142_pool_default() {
        let pool: super::Xc142Pool<String> = super::Xc142Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_142_pool_extend() {
        let mut pool = super::Xc142Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_142_pool_retain() {
        let mut pool = super::Xc142Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_142_scheduler_round_robin() {
        let mut sched = super::Xc142Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_142_scheduler_empty() {
        let mut sched = super::Xc142Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_142_scheduler_reset() {
        let mut sched = super::Xc142Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_142_scheduler_add_remove() {
        let mut sched = super::Xc142Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_142_scheduler_targets() {
        let sched = super::Xc142Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_142_hash_empty() {
        assert_eq!(super::xc_142_hash(b""), 5381);
    }

    #[test]
    fn xc_142_hash_data() {
        let h = super::xc_142_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_142_hash(b"hello"), h);
    }

    #[test]
    fn xc_142_reverse_str() {
        assert_eq!(super::xc_142_reverse("abc"), "cba");
        assert_eq!(super::xc_142_reverse(""), "");
    }


    #[test]
    fn xe_5_pipeline_empty() {
        let p = super::Xe5Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_5_pipeline_parse_stage() {
        let p = super::Xe5Pipeline::new()
            .add_parse(super::xe_5_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_5_pipeline_transform_double() {
        let p = super::Xe5Pipeline::new()
            .add_transform(super::xe_5_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_5_pipeline_validate_reverse() {
        let p = super::Xe5Pipeline::new()
            .add_validate(super::xe_5_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_5_pipeline_emit_filter() {
        let p = super::Xe5Pipeline::new()
            .add_emit(super::xe_5_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_5_pipeline_multi_stage() {
        let p = super::Xe5Pipeline::new()
            .add_parse(super::xe_5_pipeline_identity)
            .add_transform(super::xe_5_pipeline_double)
            .add_validate(super::xe_5_pipeline_reverse)
            .add_emit(super::xe_5_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_5_pipeline_error_propagation() {
        let p = super::Xe5Pipeline::new()
            .add_parse(super::xe_5_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe5Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_5_pipeline_compose() {
        let p1 = super::Xe5Pipeline::new()
            .add_parse(super::xe_5_pipeline_identity);
        let p2 = super::Xe5Pipeline::new()
            .add_transform(super::xe_5_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_5_pipeline_error_display() {
        let e = super::Xe5PipelineError {
            stage: super::Xe5Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_5_cache_put_get() {
        let mut c = super::Xe5Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_5_cache_miss() {
        let mut c: super::Xe5Cache<&str, i32> = super::Xe5Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_5_cache_ttl_expiry() {
        let mut c = super::Xe5Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_5_cache_evict() {
        let mut c = super::Xe5Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_5_cache_capacity() {
        let mut c = super::Xe5Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_5_cache_stats() {
        let mut c = super::Xe5Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_5_cache_clear() {
        let mut c = super::Xe5Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #68 --

    #[test]
    fn xf68_trie_insert_search() {
        let mut t = Xf68Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf68_trie_starts_with() {
        let mut t = Xf68Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf68_trie_remove() {
        let mut t = Xf68Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf68_trie_word_count() {
        let mut t = Xf68Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf68_trie_longest_prefix() {
        let mut t = Xf68Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf68_trie_all_words() {
        let mut t = Xf68Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf68_trie_autocomplete() {
        let mut t = Xf68Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf68_trie_empty_search() {
        let t = Xf68Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf68_bloom_add_contains() {
        let mut bf = Xf68BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf68_bloom_probably_absent() {
        let bf = Xf68BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf68_bloom_false_positive_rate() {
        let mut bf = Xf68BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf68_bloom_clear() {
        let mut bf = Xf68BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf68_bloom_union() {
        let mut a = Xf68BloomFilter::xf_new(512, 2);
        let mut b = Xf68BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf68_bloom_intersection_estimate() {
        let mut a = Xf68BloomFilter::xf_new(512, 2);
        let mut b = Xf68BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf68_bloom_union_size_mismatch() {
        let a = Xf68BloomFilter::xf_new(256, 2);
        let b = Xf68BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh141_skip_insert_contains() {
        let mut sl = super::Xh141SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh141_skip_remove() {
        let mut sl = super::Xh141SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh141_skip_len() {
        let mut sl = super::Xh141SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh141_skip_range_query() {
        let mut sl = super::Xh141SkipList::xh_new(4);
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
    fn xh141_skip_floor_ceiling() {
        let mut sl = super::Xh141SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh141_skip_rank() {
        let mut sl = super::Xh141SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh141_skip_empty() {
        let sl = super::Xh141SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh141_skip_duplicates() {
        let mut sl = super::Xh141SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh141_bitset_set_test() {
        let mut bs = super::Xh141BitSet::xh_new(256);
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
    fn xh141_bitset_clear_count() {
        let mut bs = super::Xh141BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh141_bitset_and_or_xor() {
        let mut a = super::Xh141BitSet::xh_new(128);
        let mut b = super::Xh141BitSet::xh_new(128);
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
    fn xh141_bitset_iter_ones() {
        let mut bs = super::Xh141BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh141_bitset_first_last() {
        let mut bs = super::Xh141BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh141_bitset_empty() {
        let bs = super::Xh141BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi141_deque_push_pop_back() {
        let mut dq = super::Xi141Deque::xi_new(4);
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
    fn xi141_deque_push_pop_front() {
        let mut dq = super::Xi141Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi141_deque_mixed_ops() {
        let mut dq = super::Xi141Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi141_deque_get_and_split() {
        let mut dq = super::Xi141Deque::xi_new(8);
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
    fn xi141_deque_rotate_left() {
        let mut dq = super::Xi141Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi141_deque_rotate_right() {
        let mut dq = super::Xi141Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi141_deque_grow() {
        let mut dq = super::Xi141Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi141_deque_empty() {
        let dq = super::Xi141Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi141_interval_tree_insert_query() {
        let mut tree = super::Xi141IntervalTree::xi_new();
        tree.xi_insert(super::Xi141Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi141Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi141Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi141_interval_tree_overlap() {
        let mut tree = super::Xi141IntervalTree::xi_new();
        tree.xi_insert(super::Xi141Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi141Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi141Interval::xi_new(12, 20));
        let q = super::Xi141Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi141_interval_tree_remove() {
        let mut tree = super::Xi141IntervalTree::xi_new();
        tree.xi_insert(super::Xi141Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi141Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi141_interval_tree_gaps() {
        let mut tree = super::Xi141IntervalTree::xi_new();
        tree.xi_insert(super::Xi141Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi141Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi141Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi141Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi141Interval::xi_new(8, 10));
    }

    #[test]
    fn xi141_interval_tree_merge() {
        let mut tree = super::Xi141IntervalTree::xi_new();
        tree.xi_insert(super::Xi141Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi141Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi141Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi141Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi141Interval::xi_new(10, 15));
    }

    #[test]
    fn xi141_interval_tree_all() {
        let mut tree = super::Xi141IntervalTree::xi_new();
        tree.xi_insert(super::Xi141Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi141Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi141_interval_tree_empty() {
        let tree = super::Xi141IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi141_interval_tree_contains_point() {
        let iv = super::Xi141Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}