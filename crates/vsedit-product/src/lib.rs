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
}
