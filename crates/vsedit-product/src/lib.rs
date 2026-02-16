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
}
