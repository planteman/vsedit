//! Extension install/update management.
//!
//! RPC bridge between the extension host and the main thread for extension management.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_mgmt";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MgmtMessage {
    GetInstalled,
    GetExtension {
        extension_id: String,
    },
    Install {
        extension_id: String,
    },
    Uninstall {
        extension_id: String,
    },
    Enable {
        extension_id: String,
    },
    Disable {
        extension_id: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionKind {
    Ui,
    Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionInfo {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub publisher: String,
    pub kind: ExtensionKind,
    pub is_enabled: bool,
    pub extension_path: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<ExtensionDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionDependency {
    pub id: String,
    pub version_range: String,
}

/// Aggregate statistics about installed extensions.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionStats {
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub by_kind: HashMap<String, usize>,
    pub by_publisher: HashMap<String, usize>,
}

// ── Bridge ──

pub struct MgmtBridge {
    extensions: Vec<ExtensionInfo>,
}

impl MgmtBridge {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
        }
    }

    pub fn install(&mut self, ext: ExtensionInfo) {
        if !self.extensions.iter().any(|e| e.id == ext.id) {
            self.extensions.push(ext);
        }
    }

    pub fn uninstall(&mut self, id: &str) -> bool {
        let before = self.extensions.len();
        self.extensions.retain(|e| e.id != id);
        self.extensions.len() < before
    }

    pub fn get_extension(&self, id: &str) -> Option<&ExtensionInfo> {
        self.extensions.iter().find(|e| e.id == id)
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(ext) = self.extensions.iter_mut().find(|e| e.id == id) {
            ext.is_enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn list_installed(&self) -> &[ExtensionInfo] {
        &self.extensions
    }

    pub fn installed_count(&self) -> usize {
        self.extensions.len()
    }

    pub fn get_enabled_extensions(&self) -> Vec<&ExtensionInfo> {
        self.extensions.iter().filter(|e| e.is_enabled).collect()
    }

    pub fn get_disabled_extensions(&self) -> Vec<&ExtensionInfo> {
        self.extensions.iter().filter(|e| !e.is_enabled).collect()
    }

    pub fn get_extensions_by_publisher(&self, publisher: &str) -> Vec<&ExtensionInfo> {
        self.extensions
            .iter()
            .filter(|e| e.publisher == publisher)
            .collect()
    }

    pub fn get_extensions_by_kind(&self, kind: ExtensionKind) -> Vec<&ExtensionInfo> {
        self.extensions
            .iter()
            .filter(|e| e.kind == kind)
            .collect()
    }

    pub fn update_version(&mut self, id: &str, new_version: &str) -> bool {
        if let Some(ext) = self.extensions.iter_mut().find(|e| e.id == id) {
            ext.version = new_version.to_string();
            true
        } else {
            false
        }
    }

    /// Returns `true` if the given extension declares `dep_id` as a dependency.
    pub fn has_dependency(&self, id: &str, dep_id: &str) -> bool {
        self.extensions
            .iter()
            .find(|e| e.id == id)
            .map_or(false, |e| e.dependencies.iter().any(|d| d.id == dep_id))
    }

    /// Returns every installed extension that lists `id` among its dependencies.
    pub fn get_dependents(&self, id: &str) -> Vec<&ExtensionInfo> {
        self.extensions
            .iter()
            .filter(|e| e.dependencies.iter().any(|d| d.id == id))
            .collect()
    }

    pub fn get_stats(&self) -> ExtensionStats {
        let total = self.extensions.len();
        let enabled = self.extensions.iter().filter(|e| e.is_enabled).count();
        let disabled = total - enabled;

        let mut by_kind: HashMap<String, usize> = HashMap::new();
        let mut by_publisher: HashMap<String, usize> = HashMap::new();

        for ext in &self.extensions {
            let kind_key = format!("{:?}", ext.kind);
            *by_kind.entry(kind_key).or_insert(0) += 1;
            *by_publisher.entry(ext.publisher.clone()).or_insert(0) += 1;
        }

        ExtensionStats {
            total,
            enabled,
            disabled,
            by_kind,
            by_publisher,
        }
    }

    /// Case-insensitive search across extension `id` and `display_name`.
    pub fn search_extensions(&self, query: &str) -> Vec<&ExtensionInfo> {
        let q = query.to_lowercase();
        self.extensions
            .iter()
            .filter(|e| {
                e.id.to_lowercase().contains(&q)
                    || e.display_name.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn handle_message(&mut self, msg: &MgmtMessage) -> serde_json::Value {
        match msg {
            MgmtMessage::GetInstalled => {
                let ids: Vec<&str> = self.extensions.iter().map(|e| e.id.as_str()).collect();
                serde_json::json!({"extensions": ids})
            }
            MgmtMessage::GetExtension { extension_id } => {
                let found = self.get_extension(extension_id).is_some();
                serde_json::json!({"found": found, "id": extension_id})
            }
            MgmtMessage::Install { extension_id } => {
                // In real impl would download/install; here we acknowledge
                serde_json::json!({"installing": extension_id})
            }
            MgmtMessage::Uninstall { extension_id } => {
                let ok = self.uninstall(extension_id);
                serde_json::json!({"uninstalled": ok})
            }
            MgmtMessage::Enable { extension_id } => {
                let ok = self.set_enabled(extension_id, true);
                serde_json::json!({"enabled": ok})
            }
            MgmtMessage::Disable { extension_id } => {
                let ok = self.set_enabled(extension_id, false);
                serde_json::json!({"disabled": ok})
            }
        }
    }
}

impl Default for MgmtBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the mgmt extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

// ── Extension Search Result (convenience wrapper) ──

/// Simplified search result for marketplace UI display.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionSearchResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub download_count: u64,
}

impl From<&GalleryExtension> for ExtensionSearchResult {
    fn from(ext: &GalleryExtension) -> Self {
        Self {
            id: ext.id.clone(),
            name: ext.display_name.clone(),
            description: ext.description.clone(),
            version: ext.version.clone(),
            download_count: ext.download_count,
        }
    }
}

/// Search the Open VSX marketplace and return simplified results.
pub async fn search_extensions(query: &str) -> Result<Vec<ExtensionSearchResult>, String> {
    let gq = GalleryQuery::new(query);
    let gallery_results = search_gallery(&gq).await?;
    Ok(gallery_results.iter().map(ExtensionSearchResult::from).collect())
}

/// Install an extension by its marketplace id (e.g. "publisher.name").
/// Downloads from the Open VSX registry and extracts to `ext_dir`.
pub async fn install_extension_by_id(
    id: &str,
    ext_dir: &std::path::Path,
) -> Result<InstalledExtension, String> {
    // Fetch the extension metadata to get the download URL.
    let url = format!("{OPEN_VSX_API}/-/search?query={}&size=1", urlencoded(id));
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("gallery returned status {}", resp.status()));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse JSON: {e}"))?;
    let results = parse_gallery_response(&body)?;
    let ext = results
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| format!("extension '{id}' not found in gallery"))?;
    let download_url = ext
        .download_url
        .ok_or_else(|| format!("no download URL for '{id}'"))?;
    install_extension(&download_url, ext_dir).await
}

// ── Gallery Client (Open VSX) ──

/// Default Open VSX API endpoint.
pub const OPEN_VSX_API: &str = "https://open-vsx.org/api";

/// Query parameters for marketplace search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryQuery {
    pub text: String,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    #[serde(default)]
    pub sort_by: GallerySortBy,
    #[serde(default)]
    pub sort_order: GallerySortOrder,
}

fn default_page() -> u32 { 0 }
fn default_page_size() -> u32 { 20 }

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GallerySortBy {
    #[default]
    Relevance,
    DownloadCount,
    Rating,
    Timestamp,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GallerySortOrder {
    #[default]
    Desc,
    Asc,
}

impl GalleryQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            page: 0,
            page_size: 20,
            sort_by: GallerySortBy::default(),
            sort_order: GallerySortOrder::default(),
        }
    }

    pub fn with_page(mut self, page: u32) -> Self {
        self.page = page;
        self
    }

    pub fn with_page_size(mut self, size: u32) -> Self {
        self.page_size = size;
        self
    }

    pub fn with_sort(mut self, by: GallerySortBy, order: GallerySortOrder) -> Self {
        self.sort_by = by;
        self.sort_order = order;
        self
    }

    /// Build the Open VSX search URL from this query.
    pub fn to_url(&self, base: &str) -> String {
        let sort_key = match self.sort_by {
            GallerySortBy::Relevance => "relevance",
            GallerySortBy::DownloadCount => "downloadCount",
            GallerySortBy::Rating => "rating",
            GallerySortBy::Timestamp => "timestamp",
        };
        let sort_order = match self.sort_order {
            GallerySortOrder::Desc => "desc",
            GallerySortOrder::Asc => "asc",
        };
        format!(
            "{base}/-/search?query={}&offset={}&size={}&sortBy={sort_key}&sortOrder={sort_order}",
            urlencoded(&self.text),
            self.page * self.page_size,
            self.page_size,
        )
    }
}

fn urlencoded(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('+', "%2B")
}

/// A marketplace extension returned by the gallery API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GalleryExtension {
    pub id: String,
    pub display_name: String,
    pub publisher: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub download_count: u64,
    #[serde(default)]
    pub rating: f64,
    #[serde(default)]
    pub install_count: u64,
    #[serde(default)]
    pub download_url: Option<String>,
}

/// Search the Open VSX gallery. Requires an async runtime.
pub async fn search_gallery(query: &GalleryQuery) -> Result<Vec<GalleryExtension>, String> {
    search_gallery_at(OPEN_VSX_API, query).await
}

/// Search the Open VSX gallery at a specific base URL.
pub async fn search_gallery_at(
    base_url: &str,
    query: &GalleryQuery,
) -> Result<Vec<GalleryExtension>, String> {
    let url = query.to_url(base_url);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("gallery returned status {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse JSON: {e}"))?;

    parse_gallery_response(&body)
}

/// Parse the Open VSX search response JSON into gallery extensions.
pub fn parse_gallery_response(
    body: &serde_json::Value,
) -> Result<Vec<GalleryExtension>, String> {
    let extensions = body
        .get("extensions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing 'extensions' array in response".to_string())?;

    let mut results = Vec::new();
    for ext in extensions {
        let namespace = ext
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let name = ext.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let display_name = ext
            .get("displayName")
            .and_then(|v| v.as_str())
            .unwrap_or(name);
        let version = ext.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0");
        let description = ext
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let download_count = ext
            .get("downloadCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let rating = ext
            .get("averageRating")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let id = if namespace.is_empty() {
            name.to_string()
        } else {
            format!("{namespace}.{name}")
        };

        let download_url = ext
            .get("files")
            .and_then(|f| f.get("download"))
            .and_then(|v| v.as_str())
            .map(String::from);

        results.push(GalleryExtension {
            id,
            display_name: display_name.to_string(),
            publisher: namespace.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            download_count,
            rating,
            install_count: download_count,
            download_url,
        });
    }
    Ok(results)
}

/// Get info about a single extension from Open VSX.
pub async fn get_extension_info(
    publisher: &str,
    name: &str,
) -> Result<GalleryExtension, String> {
    get_extension_info_at(OPEN_VSX_API, publisher, name).await
}

/// Get info about a single extension from a specific Open VSX endpoint.
pub async fn get_extension_info_at(
    base_url: &str,
    publisher: &str,
    name: &str,
) -> Result<GalleryExtension, String> {
    let url = format!("{base_url}/{publisher}/{name}");
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("gallery returned status {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse JSON: {e}"))?;

    let namespace = body
        .get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or(publisher);
    let ext_name = body.get("name").and_then(|v| v.as_str()).unwrap_or(name);
    let display_name = body
        .get("displayName")
        .and_then(|v| v.as_str())
        .unwrap_or(ext_name);
    let version = body
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0");
    let description = body
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let download_count = body
        .get("downloadCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let rating = body
        .get("averageRating")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let download_url = body
        .get("files")
        .and_then(|f| f.get("download"))
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(GalleryExtension {
        id: format!("{namespace}.{ext_name}"),
        display_name: display_name.to_string(),
        publisher: namespace.to_string(),
        version: version.to_string(),
        description: description.to_string(),
        download_count,
        rating,
        install_count: download_count,
        download_url,
    })
}

// ── Extension Installation ──

/// Default extensions directory under user config.
pub fn extensions_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("vsedit")
        .join("extensions")
}

/// An installed extension on disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstalledExtension {
    pub id: String,
    pub version: String,
    pub path: String,
    pub is_enabled: bool,
    pub manifest: ExtensionManifest,
}

/// Parsed package.json manifest from a VSIX extension.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionManifest {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "displayName")]
    pub display_name: String,
    #[serde(default)]
    pub publisher: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub contributes: ExtensionContributions,
    #[serde(default, rename = "extensionDependencies")]
    pub extension_dependencies: Vec<String>,
}

/// Contributions declared in an extension's package.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExtensionContributions {
    #[serde(default)]
    pub commands: Vec<serde_json::Value>,
    #[serde(default)]
    pub languages: Vec<serde_json::Value>,
    #[serde(default)]
    pub themes: Vec<serde_json::Value>,
    #[serde(default)]
    pub snippets: Vec<serde_json::Value>,
    #[serde(default)]
    pub views: serde_json::Value,
    #[serde(default)]
    pub menus: serde_json::Value,
    #[serde(default)]
    pub keybindings: Vec<serde_json::Value>,
    #[serde(default)]
    pub grammars: Vec<serde_json::Value>,
}

/// Install an extension from a VSIX file path or URL.
/// VSIX is a ZIP file; we extract to `<extensions_dir>/<publisher.name-version>/`.
pub async fn install_extension(
    vsix_url_or_path: &str,
    ext_dir: &std::path::Path,
) -> Result<InstalledExtension, String> {
    let vsix_bytes = if vsix_url_or_path.starts_with("http://")
        || vsix_url_or_path.starts_with("https://")
    {
        let client = reqwest::Client::new();
        client
            .get(vsix_url_or_path)
            .send()
            .await
            .map_err(|e| format!("download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("read failed: {e}"))?
            .to_vec()
    } else {
        std::fs::read(vsix_url_or_path).map_err(|e| format!("read file failed: {e}"))?
    };

    install_extension_from_bytes(&vsix_bytes, ext_dir)
}

/// Install from raw VSIX bytes (synchronous extraction).
pub fn install_extension_from_bytes(
    vsix_bytes: &[u8],
    ext_dir: &std::path::Path,
) -> Result<InstalledExtension, String> {
    let cursor = std::io::Cursor::new(vsix_bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("invalid VSIX ZIP: {e}"))?;

    // Find package.json in the archive (may be at root or under extension/)
    let manifest = find_and_parse_manifest(&mut archive)?;

    let dir_name = format!(
        "{}.{}-{}",
        manifest.publisher, manifest.name, manifest.version
    );
    let target_dir = ext_dir.join(&dir_name);

    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir)
            .map_err(|e| format!("failed to remove old dir: {e}"))?;
    }
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("failed to create dir: {e}"))?;

    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("zip entry error: {e}"))?;
        let Some(name) = file.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let out_path = target_dir.join(&name);
        if file.is_dir() {
            std::fs::create_dir_all(&out_path).ok();
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut outfile = std::fs::File::create(&out_path)
                .map_err(|e| format!("create file failed: {e}"))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("extract failed: {e}"))?;
        }
    }

    let id = format!("{}.{}", manifest.publisher, manifest.name);
    Ok(InstalledExtension {
        id,
        version: manifest.version.clone(),
        path: target_dir.to_string_lossy().to_string(),
        is_enabled: true,
        manifest,
    })
}

fn find_and_parse_manifest(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Result<ExtensionManifest, String> {
    // Try common VSIX paths for package.json
    let candidates = [
        "extension/package.json",
        "package.json",
    ];
    for candidate in &candidates {
        if let Ok(mut file) = archive.by_name(candidate) {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut file, &mut contents)
                .map_err(|e| format!("read manifest failed: {e}"))?;
            let manifest: ExtensionManifest =
                serde_json::from_str(&contents).map_err(|e| format!("parse manifest: {e}"))?;
            return Ok(manifest);
        }
    }
    Err("no package.json found in VSIX".to_string())
}

/// Uninstall an extension by removing its directory.
pub fn uninstall_extension(id: &str, ext_dir: &std::path::Path) -> Result<(), String> {
    // Find dir matching publisher.name-*
    for entry in std::fs::read_dir(ext_dir).map_err(|e| format!("read dir: {e}"))? {
        let entry = entry.map_err(|e| format!("entry: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&format!("{id}-")) || name.starts_with(id) {
            std::fs::remove_dir_all(entry.path())
                .map_err(|e| format!("remove failed: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("extension {id} not found on disk"))
}

/// Update an extension by downloading the latest version.
pub async fn update_extension(
    id: &str,
    ext_dir: &std::path::Path,
) -> Result<InstalledExtension, String> {
    let parts: Vec<&str> = id.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(format!("invalid extension id: {id}"));
    }
    let (publisher, name) = (parts[0], parts[1]);

    let info = get_extension_info(publisher, name).await?;
    let download_url = info
        .download_url
        .ok_or_else(|| format!("no download URL for {id}"))?;

    // Remove old version
    let _ = uninstall_extension(id, ext_dir);

    install_extension(&download_url, ext_dir).await
}

// ── Extension Scanning ──

/// Scan the extensions directory and return all installed extensions.
pub fn scan_installed_extensions(
    ext_dir: &std::path::Path,
) -> Vec<InstalledExtension> {
    scan_installed_extensions_with_state(ext_dir, &HashMap::new())
}

/// Scan with a state map tracking enabled/disabled per extension id.
pub fn scan_installed_extensions_with_state(
    ext_dir: &std::path::Path,
    enable_state: &HashMap<String, bool>,
) -> Vec<InstalledExtension> {
    let mut results = Vec::new();
    let entries = match std::fs::read_dir(ext_dir) {
        Ok(e) => e,
        Err(_) => return results,
    };

    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }
        // Look for package.json in the extension dir or extension/ subdir
        let dir_path = entry.path();
        let manifest = try_parse_manifest_from_dir(&dir_path);
        if let Some(manifest) = manifest {
            let id = format!("{}.{}", manifest.publisher, manifest.name);
            let is_enabled = enable_state.get(&id).copied().unwrap_or(true);
            results.push(InstalledExtension {
                id,
                version: manifest.version.clone(),
                path: dir_path.to_string_lossy().to_string(),
                is_enabled,
                manifest,
            });
        }
    }
    results
}

fn try_parse_manifest_from_dir(dir: &std::path::Path) -> Option<ExtensionManifest> {
    let candidates = [
        dir.join("package.json"),
        dir.join("extension").join("package.json"),
    ];
    for path in &candidates {
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(manifest) = serde_json::from_str::<ExtensionManifest>(&contents) {
                if !manifest.name.is_empty() {
                    return Some(manifest);
                }
            }
        }
    }
    None
}

// ── Extension Enable/Disable State ──

/// Global state file for extension enabled/disabled state.
pub fn state_file_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("vsedit")
        .join("extension-state.json")
}

/// Load extension enable state from disk.
pub fn load_enable_state(path: &std::path::Path) -> HashMap<String, bool> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save extension enable state to disk.
pub fn save_enable_state(
    path: &std::path::Path,
    state: &HashMap<String, bool>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(state).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("write: {e}"))
}

/// Enable an extension by updating the state file.
pub fn enable_extension(
    id: &str,
    state_path: &std::path::Path,
) -> Result<(), String> {
    let mut state = load_enable_state(state_path);
    state.insert(id.to_string(), true);
    save_enable_state(state_path, &state)
}

/// Disable an extension by updating the state file.
pub fn disable_extension(
    id: &str,
    state_path: &std::path::Path,
) -> Result<(), String> {
    let mut state = load_enable_state(state_path);
    state.insert(id.to_string(), false);
    save_enable_state(state_path, &state)
}

// ── Semantic Version Parsing & Comparison ──

/// A parsed semantic version (major.minor.patch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SemVer {
    /// Parse a version string like "1.2.3". Returns `None` on invalid input.
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

    /// True when `self` is strictly newer than `other`.
    pub fn is_newer_than(&self, other: &SemVer) -> bool {
        (self.major, self.minor, self.patch) > (other.major, other.minor, other.patch)
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

// ── Version Range Matching ──

/// Check whether `version` satisfies `range`.
///
/// Supported range prefixes:
///   `^1.2.3` – compatible (same major, >= minor.patch)
///   `~1.2.3` – patch-level (same major.minor, >= patch)
///   `>=1.2.3` / `>1.2.3` / `<=1.2.3` / `<1.2.3` / `=1.2.3`
///   `*`       – any version
///   bare `1.2.3` is treated as `=1.2.3`
pub fn version_satisfies(version: &str, range: &str) -> bool {
    let range = range.trim();
    if range == "*" {
        return true;
    }
    if let Some(rest) = range.strip_prefix('^') {
        let Some(ver) = SemVer::parse(version) else { return false };
        let Some(req) = SemVer::parse(rest) else { return false };
        ver.major == req.major && ver >= req
    } else if let Some(rest) = range.strip_prefix('~') {
        let Some(ver) = SemVer::parse(version) else { return false };
        let Some(req) = SemVer::parse(rest) else { return false };
        ver.major == req.major && ver.minor == req.minor && ver.patch >= req.patch
    } else if let Some(rest) = range.strip_prefix(">=") {
        let Some(ver) = SemVer::parse(version) else { return false };
        let Some(req) = SemVer::parse(rest) else { return false };
        ver >= req
    } else if let Some(rest) = range.strip_prefix('>') {
        let Some(ver) = SemVer::parse(version) else { return false };
        let Some(req) = SemVer::parse(rest) else { return false };
        ver > req
    } else if let Some(rest) = range.strip_prefix("<=") {
        let Some(ver) = SemVer::parse(version) else { return false };
        let Some(req) = SemVer::parse(rest) else { return false };
        ver <= req
    } else if let Some(rest) = range.strip_prefix('<') {
        let Some(ver) = SemVer::parse(version) else { return false };
        let Some(req) = SemVer::parse(rest) else { return false };
        ver < req
    } else {
        let bare = range.strip_prefix('=').unwrap_or(range);
        let Some(ver) = SemVer::parse(version) else { return false };
        let Some(req) = SemVer::parse(bare) else { return false };
        ver == req
    }
}

// ── Dependency Resolution ──

/// Errors that can occur during dependency resolution.
#[derive(Debug, Clone, PartialEq)]
pub enum DependencyError {
    /// A required dependency is not installed.
    Missing { extension_id: String, missing_dep: String },
    /// An installed dependency's version doesn't satisfy the required range.
    Incompatible {
        extension_id: String,
        dep_id: String,
        required_range: String,
        installed_version: String,
    },
    /// A dependency cycle was detected.
    Cycle(Vec<String>),
}

impl std::fmt::Display for DependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { extension_id, missing_dep } => {
                write!(f, "{extension_id}: missing dependency {missing_dep}")
            }
            Self::Incompatible { extension_id, dep_id, required_range, installed_version } => {
                write!(
                    f,
                    "{extension_id}: {dep_id} {installed_version} does not satisfy {required_range}"
                )
            }
            Self::Cycle(ids) => write!(f, "dependency cycle: {}", ids.join(" -> ")),
        }
    }
}

impl MgmtBridge {
    /// Validate that every dependency of every installed extension is present
    /// and satisfies its version range.
    pub fn check_dependencies(&self) -> Vec<DependencyError> {
        let mut errors = Vec::new();
        let index: HashMap<&str, &ExtensionInfo> =
            self.extensions.iter().map(|e| (e.id.as_str(), e)).collect();

        for ext in &self.extensions {
            for dep in &ext.dependencies {
                match index.get(dep.id.as_str()) {
                    None => errors.push(DependencyError::Missing {
                        extension_id: ext.id.clone(),
                        missing_dep: dep.id.clone(),
                    }),
                    Some(installed) => {
                        if !version_satisfies(&installed.version, &dep.version_range) {
                            errors.push(DependencyError::Incompatible {
                                extension_id: ext.id.clone(),
                                dep_id: dep.id.clone(),
                                required_range: dep.version_range.clone(),
                                installed_version: installed.version.clone(),
                            });
                        }
                    }
                }
            }
        }
        errors
    }

    /// Return a topological ordering of extensions respecting dependencies.
    /// Extensions with no dependencies come first.
    /// Returns `Err` with a cycle path if a cycle is detected.
    pub fn resolve_load_order(&self) -> Result<Vec<&ExtensionInfo>, DependencyError> {
        let index: HashMap<&str, &ExtensionInfo> =
            self.extensions.iter().map(|e| (e.id.as_str(), e)).collect();

        let mut order: Vec<&str> = Vec::new();
        let mut visited: HashMap<&str, bool> = HashMap::new(); // false = in-progress

        for ext in &self.extensions {
            if !visited.contains_key(ext.id.as_str()) {
                Self::topo_visit(ext.id.as_str(), &index, &mut visited, &mut order)?;
            }
        }

        Ok(order.iter().filter_map(|id| index.get(id).copied()).collect())
    }

    fn topo_visit<'a>(
        id: &'a str,
        index: &HashMap<&'a str, &'a ExtensionInfo>,
        visited: &mut HashMap<&'a str, bool>,
        order: &mut Vec<&'a str>,
    ) -> Result<(), DependencyError> {
        if let Some(&done) = visited.get(id) {
            if !done {
                return Err(DependencyError::Cycle(vec![id.to_string()]));
            }
            return Ok(());
        }
        visited.insert(id, false);
        if let Some(ext) = index.get(id) {
            for dep in &ext.dependencies {
                if let Err(DependencyError::Cycle(mut path)) =
                    Self::topo_visit(dep.id.as_str(), index, visited, order)
                {
                    path.insert(0, id.to_string());
                    return Err(DependencyError::Cycle(path));
                }
            }
        }
        visited.insert(id, true);
        order.push(id);
        Ok(())
    }

    /// Disable an extension and cascade-disable all extensions that depend on it.
    /// Returns the list of extension ids that were disabled.
    pub fn disable_cascade(&mut self, id: &str) -> Vec<String> {
        let mut disabled = Vec::new();
        self.disable_cascade_inner(id, &mut disabled);
        disabled
    }

    fn disable_cascade_inner(&mut self, id: &str, disabled: &mut Vec<String>) {
        if let Some(ext) = self.extensions.iter_mut().find(|e| e.id == id) {
            if !ext.is_enabled {
                return;
            }
            ext.is_enabled = false;
            disabled.push(id.to_string());
        }
        let dependents: Vec<String> = self
            .extensions
            .iter()
            .filter(|e| e.is_enabled && e.dependencies.iter().any(|d| d.id == id))
            .map(|e| e.id.clone())
            .collect();
        for dep_id in dependents {
            self.disable_cascade_inner(&dep_id, disabled);
        }
    }

    /// Check which installed extensions have a newer version available.
    /// `available` maps extension id → latest version string.
    pub fn check_updates(&self, available: &HashMap<String, String>) -> Vec<UpdateInfo> {
        let mut updates = Vec::new();
        for ext in &self.extensions {
            if let Some(latest_str) = available.get(&ext.id) {
                if let (Some(current), Some(latest)) =
                    (SemVer::parse(&ext.version), SemVer::parse(latest_str))
                {
                    if latest.is_newer_than(&current) {
                        updates.push(UpdateInfo {
                            extension_id: ext.id.clone(),
                            current_version: ext.version.clone(),
                            latest_version: latest_str.clone(),
                        });
                    }
                }
            }
        }
        updates
    }
}

/// Describes an available update for an installed extension.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateInfo {
    pub extension_id: String,
    pub current_version: String,
    pub latest_version: String,
}

// ── Search Result Ranking ──

/// Rank marketplace search results by a weighted score.
///
/// Score = `query_relevance * 10  +  ln(1 + downloads) * 2  +  rating`.
/// Results are returned in descending score order.
pub fn rank_search_results(results: &[GalleryExtension], query: &str) -> Vec<RankedResult> {
    let q = query.to_lowercase();
    let mut ranked: Vec<RankedResult> = results
        .iter()
        .map(|ext| {
            let name_lower = ext.display_name.to_lowercase();
            let id_lower = ext.id.to_lowercase();
            let relevance: f64 = if id_lower == q || name_lower == q {
                10.0
            } else if id_lower.starts_with(&q) || name_lower.starts_with(&q) {
                7.0
            } else if id_lower.contains(&q) || name_lower.contains(&q) {
                4.0
            } else if ext.description.to_lowercase().contains(&q) {
                2.0
            } else {
                0.0
            };
            let download_score = (1.0 + ext.download_count as f64).ln() * 2.0;
            let score = relevance * 10.0 + download_score + ext.rating;
            RankedResult {
                id: ext.id.clone(),
                display_name: ext.display_name.clone(),
                score,
            }
        })
        .collect();
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

/// A search result annotated with a computed relevance score.
#[derive(Debug, Clone)]
pub struct RankedResult {
    pub id: String,
    pub display_name: String,
    pub score: f64,
}

// ── Extension Rollback ──

/// A record of a single installed version of an extension.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionRecord {
    pub version: String,
    pub installed_at: u64,
    pub path: Option<String>,
}

/// Tracks version history for extensions to support rollback operations.
#[derive(Debug, Clone)]
pub struct ExtensionRollback {
    pub history: HashMap<String, Vec<VersionRecord>>,
}

impl ExtensionRollback {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
        }
    }

    /// Record a new installation event for the given extension.
    pub fn record_install(&mut self, ext_id: &str, version: &str, timestamp: u64) {
        let records = self.history.entry(ext_id.to_string()).or_default();
        records.push(VersionRecord {
            version: version.to_string(),
            installed_at: timestamp,
            path: None,
        });
    }

    /// Return the second-to-last version record, if one exists.
    pub fn previous_version(&self, ext_id: &str) -> Option<&VersionRecord> {
        self.history.get(ext_id).and_then(|records| {
            if records.len() >= 2 {
                Some(&records[records.len() - 2])
            } else {
                None
            }
        })
    }

    /// Whether there is a previous version available to roll back to.
    pub fn can_rollback(&self, ext_id: &str) -> bool {
        self.previous_version(ext_id).is_some()
    }

    /// Return all version records for the given extension.
    pub fn version_history(&self, ext_id: &str) -> Vec<&VersionRecord> {
        self.history
            .get(ext_id)
            .map(|records| records.iter().collect())
            .unwrap_or_default()
    }

    /// Return the version string of the rollback target (previous version).
    pub fn rollback_target(&self, ext_id: &str) -> Option<&str> {
        self.previous_version(ext_id).map(|r| r.version.as_str())
    }

    /// Total number of version records across all extensions.
    pub fn total_records(&self) -> usize {
        self.history.values().map(|v| v.len()).sum()
    }
}

// ── Extension Compatibility Checker ──

/// The result of a compatibility check.
#[derive(Debug, Clone, PartialEq)]
pub struct CompatResult {
    pub compatible: bool,
    pub engine_ok: bool,
    pub api_ok: bool,
    pub message: String,
}

impl CompatResult {
    pub fn is_compatible(&self) -> bool {
        self.compatible
    }
}

impl fmt::Display for CompatResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.compatible {
            write!(f, "Compatible: {}", self.message)
        } else {
            write!(f, "Incompatible: {}", self.message)
        }
    }
}

/// Parse a semver string into (major, minor, patch), defaulting missing parts to 0.
pub fn parse_semver(version: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = version.split('.').collect();
    let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

/// Check semver compatibility: major must match, minor must be >= required.
pub fn is_semver_compatible(have: &str, need: &str) -> bool {
    let (have_major, have_minor, _) = parse_semver(have);
    let (need_major, need_minor, _) = parse_semver(need);
    have_major == need_major && have_minor >= need_minor
}

/// Checks whether extensions are compatible with the current engine and API versions.
#[derive(Debug, Clone)]
pub struct ExtensionCompatibilityChecker {
    pub engine_version: String,
    pub api_version: String,
}

impl ExtensionCompatibilityChecker {
    pub fn new(engine: &str, api: &str) -> Self {
        Self {
            engine_version: engine.to_string(),
            api_version: api.to_string(),
        }
    }

    /// Check if the given required versions are compatible with this checker's versions.
    pub fn check_compatible(
        &self,
        required_engine: &str,
        required_api: Option<&str>,
    ) -> CompatResult {
        let engine_ok = is_semver_compatible(&self.engine_version, required_engine);
        let api_ok = match required_api {
            Some(api) => is_semver_compatible(&self.api_version, api),
            None => true,
        };
        let compatible = engine_ok && api_ok;
        let message = if compatible {
            "All version requirements satisfied".to_string()
        } else if !engine_ok && !api_ok {
            format!(
                "Engine {} incompatible with required {}; API {} incompatible with required {}",
                self.engine_version,
                required_engine,
                self.api_version,
                required_api.unwrap_or("none"),
            )
        } else if !engine_ok {
            format!(
                "Engine {} incompatible with required {}",
                self.engine_version, required_engine,
            )
        } else {
            format!(
                "API {} incompatible with required {}",
                self.api_version,
                required_api.unwrap_or("none"),
            )
        };
        CompatResult {
            compatible,
            engine_ok,
            api_ok,
            message,
        }
    }
}

// ── Extension Bulk Operations ──

/// The type of bulk operation to perform on an extension.
#[derive(Debug, Clone, PartialEq)]
pub enum BulkOpType {
    Install,
    Update,
    Uninstall,
    Enable,
    Disable,
}

impl fmt::Display for BulkOpType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BulkOpType::Install => write!(f, "Install"),
            BulkOpType::Update => write!(f, "Update"),
            BulkOpType::Uninstall => write!(f, "Uninstall"),
            BulkOpType::Enable => write!(f, "Enable"),
            BulkOpType::Disable => write!(f, "Disable"),
        }
    }
}

/// The current status of a bulk operation.
#[derive(Debug, Clone, PartialEq)]
pub enum BulkOpStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
}

impl fmt::Display for BulkOpStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BulkOpStatus::Pending => write!(f, "Pending"),
            BulkOpStatus::InProgress => write!(f, "InProgress"),
            BulkOpStatus::Completed => write!(f, "Completed"),
            BulkOpStatus::Failed(reason) => write!(f, "Failed: {reason}"),
        }
    }
}

/// A single operation in a bulk batch.
#[derive(Debug, Clone)]
pub struct BulkOp {
    pub ext_id: String,
    pub op_type: BulkOpType,
    pub status: BulkOpStatus,
}

/// Manages a batch of extension operations.
#[derive(Debug, Clone)]
pub struct ExtensionBulkOperation {
    pub operations: Vec<BulkOp>,
}

impl ExtensionBulkOperation {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    pub fn add(&mut self, ext_id: &str, op: BulkOpType) {
        self.operations.push(BulkOp {
            ext_id: ext_id.to_string(),
            op_type: op,
            status: BulkOpStatus::Pending,
        });
    }

    pub fn mark_in_progress(&mut self, ext_id: &str) -> bool {
        for op in &mut self.operations {
            if op.ext_id == ext_id && op.status == BulkOpStatus::Pending {
                op.status = BulkOpStatus::InProgress;
                return true;
            }
        }
        false
    }

    pub fn mark_completed(&mut self, ext_id: &str) -> bool {
        for op in &mut self.operations {
            if op.ext_id == ext_id && op.status == BulkOpStatus::InProgress {
                op.status = BulkOpStatus::Completed;
                return true;
            }
        }
        false
    }

    pub fn mark_failed(&mut self, ext_id: &str, reason: &str) -> bool {
        for op in &mut self.operations {
            if op.ext_id == ext_id && op.status == BulkOpStatus::InProgress {
                op.status = BulkOpStatus::Failed(reason.to_string());
                return true;
            }
        }
        false
    }

    pub fn pending_count(&self) -> usize {
        self.operations
            .iter()
            .filter(|op| op.status == BulkOpStatus::Pending)
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.operations
            .iter()
            .filter(|op| op.status == BulkOpStatus::Completed)
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.operations
            .iter()
            .filter(|op| matches!(op.status, BulkOpStatus::Failed(_)))
            .count()
    }

    pub fn all_completed(&self) -> bool {
        !self.operations.is_empty()
            && self
                .operations
                .iter()
                .all(|op| op.status == BulkOpStatus::Completed)
    }

    pub fn summary(&self) -> String {
        format!(
            "{} total, {} pending, {} completed, {} failed",
            self.operations.len(),
            self.pending_count(),
            self.completed_count(),
            self.failed_count(),
        )
    }
}

// ── Extension Size Calculator ──

/// Size category for an extension based on its byte size.
#[derive(Debug, Clone, PartialEq)]
pub enum SizeCategory {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
}

impl fmt::Display for SizeCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SizeCategory::Tiny => write!(f, "Tiny"),
            SizeCategory::Small => write!(f, "Small"),
            SizeCategory::Medium => write!(f, "Medium"),
            SizeCategory::Large => write!(f, "Large"),
            SizeCategory::Huge => write!(f, "Huge"),
        }
    }
}

/// Utilities for estimating and formatting extension sizes.
#[derive(Debug, Clone)]
pub struct ExtensionSizeCalculator;

impl ExtensionSizeCalculator {
    pub fn new() -> Self {
        Self
    }

    /// Format a byte count into a human-readable string.
    pub fn format_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;
        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Estimate the installed size as approximately 2x the download size.
    pub fn estimate_install_size(download_size: u64) -> u64 {
        download_size.saturating_mul(2)
    }

    /// Sum up estimated install sizes for a slice of extensions.
    /// Uses a simple fixed estimate per extension (1 MB each) since
    /// `ExtensionInfo` does not carry a size field.
    pub fn total_size(extensions: &[ExtensionInfo]) -> u64 {
        const DEFAULT_ESTIMATE: u64 = 1_048_576; // 1 MB
        extensions.len() as u64 * DEFAULT_ESTIMATE
    }

    /// Classify a byte count into a size category.
    pub fn size_category(bytes: u64) -> SizeCategory {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        if bytes < 100 * KB {
            SizeCategory::Tiny
        } else if bytes < MB {
            SizeCategory::Small
        } else if bytes < 10 * MB {
            SizeCategory::Medium
        } else if bytes < 100 * MB {
            SizeCategory::Large
        } else {
            SizeCategory::Huge
        }
    }
}


// ─── ExtMgmt LRU Cache ───────────────────────────────────────

/// A simple LRU cache for extension meta.
#[derive(Debug)]
pub struct ExtMgmtLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> ExtMgmtLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for ExtMgmtLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtMgmtLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── ExtMgmt Formatter ───────────────────────────────────────

/// Formatting options for extension mgmt output.
#[derive(Debug, Clone)]
pub struct ExtMgmtFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for ExtMgmtFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl ExtMgmtFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for extension mgmt data.
pub struct ExtMgmtFmt {
    options: ExtMgmtFmtOpts,
}

impl ExtMgmtFmt {
    pub fn new(options: ExtMgmtFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: ExtMgmtFmtOpts::default() } }

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


/// Extension management configuration manager.
#[derive(Debug, Clone)]
pub struct ExtMgmtConfig {
    entries: Vec<ExtMgmtEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single extension management entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtMgmtEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl ExtMgmtEntry {
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

impl ExtMgmtConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: ExtMgmtEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&ExtMgmtEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ExtMgmtEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&ExtMgmtEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&ExtMgmtEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&ExtMgmtEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<ExtMgmtEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
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
// xa_ extended helpers for ext_mgmt
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtMgmtRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtMgmtRingBuf {
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
pub struct XaExtMgmtCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtMgmtCounter {
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

impl Default for XaExtMgmtCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 63
// ---------------------------------------------------------------------------

/// Generic object pool `Xc63Pool<T>`.
pub struct Xc63Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc63Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc63PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc63Pool<T> {
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
    pub fn stats(&self) -> Xc63PoolStats {
        Xc63PoolStats {
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

impl<T> Default for Xc63Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc63Scheduler`.
pub struct Xc63Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc63Scheduler {
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

impl Default for Xc63Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_63 hash for the given byte slice.
pub fn xc_63_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_63 convention.
pub fn xc_63_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_36 deepening: state machine + event bus ---

/// States for the Xd36 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd36State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd36State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd36Transition {
    pub from: Xd36State,
    pub to: Xd36State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd36StateMachine {
    current: Xd36State,
    history: Vec<Xd36Transition>,
    step_counter: usize,
}

impl Xd36StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd36State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd36State {
        self.current
    }

    pub fn history(&self) -> &[Xd36Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd36State) -> Result<Xd36State, String> {
        let allowed = match (self.current, target) {
            (Xd36State::Idle, Xd36State::Running) => true,
            (Xd36State::Running, Xd36State::Paused) => true,
            (Xd36State::Running, Xd36State::Done) => true,
            (Xd36State::Paused, Xd36State::Running) => true,
            (Xd36State::Paused, Xd36State::Done) => true,
            (Xd36State::Done, Xd36State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_36: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd36Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd36SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd36State> {
        let prefix = "Xd36SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd36State::Idle),
            "Running" => Some(Xd36State::Running),
            "Paused" => Some(Xd36State::Paused),
            "Done" => Some(Xd36State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd36State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd36 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd36Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd36Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd36HandlerFn = Box<dyn Fn(&Xd36Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd36EventBus {
    handlers: Vec<(usize, Option<String>, Xd36HandlerFn)>,
    next_id: usize,
    published: Vec<Xd36Event>,
}

impl Xd36EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd36Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd36Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd36Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd36Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #34
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf34Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf34TrieNode {
    children: std::collections::HashMap<char, Xf34TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf34Trie {
    root: Xf34TrieNode,
    count: usize,
}

impl Xf34Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf34TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf34TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf34TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf34BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf34BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 62).
pub struct Xh62SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh62SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 104 as u64,
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

/// A compact bit set supporting boolean operations (variant 62).
pub struct Xh62BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh62BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 62).
pub struct Xi62Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi62Deque<T> {
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
pub struct Xi62Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi62Interval {
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

/// A simple interval tree (variant 62).
pub struct Xi62IntervalTree {
    xi_intervals: Vec<Xi62Interval>,
}

impl Xi62IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi62Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi62Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi62Interval) -> Vec<&Xi62Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi62Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi62Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi62Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi62Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi62Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi62Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 63) ---

/// Disjoint set / union-find for crate 63.
pub struct Xj63UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj63UnionFind {
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

const XJ63_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 63.
pub struct Xj63BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj63BTreeNode<K, V>>>,
    len: usize,
}

struct Xj63BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj63BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj63BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ63_BTREE_ORDER - 1
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
        let mid = XJ63_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj63BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj63BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj63BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj63BTreeNode::xj_new_leaf();
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


// --- xk_62 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk62SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk62SegmentTree {
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
pub struct Xk62DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk62DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_63).
#[derive(Debug, Clone)]
pub struct Xl63Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl63Rope {
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

/// Suffix array for efficient string searching (xl_63).
#[derive(Debug, Clone)]
pub struct Xl63SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl63SuffixArray {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ext() -> ExtensionInfo {
        ExtensionInfo {
            id: "publisher.extension".into(),
            display_name: "My Extension".into(),
            version: "1.0.0".into(),
            publisher: "publisher".into(),
            kind: ExtensionKind::Workspace,
            is_enabled: true,
            extension_path: Some("/ext/path".into()),
            dependencies: Vec::new(),
        }
    }

    fn test_ext_ui(id: &str, publisher: &str, enabled: bool) -> ExtensionInfo {
        ExtensionInfo {
            id: id.into(),
            display_name: format!("Ext {id}"),
            version: "0.1.0".into(),
            publisher: publisher.into(),
            kind: ExtensionKind::Ui,
            is_enabled: enabled,
            extension_path: None,
            dependencies: Vec::new(),
        }
    }

    fn test_ext_with_deps(id: &str, deps: Vec<(&str, &str)>) -> ExtensionInfo {
        ExtensionInfo {
            id: id.into(),
            display_name: format!("Ext {id}"),
            version: "1.0.0".into(),
            publisher: "acme".into(),
            kind: ExtensionKind::Workspace,
            is_enabled: true,
            extension_path: None,
            dependencies: deps
                .into_iter()
                .map(|(did, vr)| ExtensionDependency {
                    id: did.into(),
                    version_range: vr.into(),
                })
                .collect(),
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = MgmtMessage::Install {
            extension_id: "publisher.ext".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: MgmtMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn extension_info_serialization() {
        let ext = test_ext();
        let json = serde_json::to_string(&ext).unwrap();
        let back: ExtensionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(ext, back);
    }

    #[test]
    fn bridge_install_and_uninstall() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        assert!(bridge.get_extension("publisher.extension").is_some());
        assert!(bridge.uninstall("publisher.extension"));
        assert!(bridge.get_extension("publisher.extension").is_none());
    }

    #[test]
    fn bridge_enable_disable() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        bridge.set_enabled("publisher.extension", false);
        assert!(!bridge.get_extension("publisher.extension").unwrap().is_enabled);
        bridge.set_enabled("publisher.extension", true);
        assert!(bridge.get_extension("publisher.extension").unwrap().is_enabled);
    }

    #[test]
    fn bridge_uninstall_unknown() {
        let mut bridge = MgmtBridge::new();
        assert!(!bridge.uninstall("nope"));
    }

    #[test]
    fn installed_count() {
        let mut bridge = MgmtBridge::new();
        assert_eq!(bridge.installed_count(), 0);
        bridge.install(test_ext());
        assert_eq!(bridge.installed_count(), 1);
        bridge.install(test_ext_ui("a.b", "a", true));
        assert_eq!(bridge.installed_count(), 2);
    }

    #[test]
    fn get_enabled_and_disabled() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext_ui("a.one", "a", true));
        bridge.install(test_ext_ui("a.two", "a", false));
        bridge.install(test_ext_ui("b.one", "b", true));

        let enabled = bridge.get_enabled_extensions();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.iter().all(|e| e.is_enabled));

        let disabled = bridge.get_disabled_extensions();
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].id, "a.two");
    }

    #[test]
    fn get_extensions_by_publisher() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext_ui("a.one", "alpha", true));
        bridge.install(test_ext_ui("a.two", "alpha", false));
        bridge.install(test_ext_ui("b.one", "beta", true));

        let alpha = bridge.get_extensions_by_publisher("alpha");
        assert_eq!(alpha.len(), 2);

        let gamma = bridge.get_extensions_by_publisher("gamma");
        assert!(gamma.is_empty());
    }

    #[test]
    fn get_extensions_by_kind() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        bridge.install(test_ext_ui("ui.ext", "pub", true));

        let ws = bridge.get_extensions_by_kind(ExtensionKind::Workspace);
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].id, "publisher.extension");

        let ui = bridge.get_extensions_by_kind(ExtensionKind::Ui);
        assert_eq!(ui.len(), 1);
        assert_eq!(ui[0].id, "ui.ext");
    }

    #[test]
    fn update_version() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        assert!(bridge.update_version("publisher.extension", "2.0.0"));
        assert_eq!(
            bridge.get_extension("publisher.extension").unwrap().version,
            "2.0.0"
        );
        assert!(!bridge.update_version("nope", "3.0.0"));
    }

    #[test]
    fn dependency_tracking() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext_with_deps("a.child", vec![("a.parent", "^1.0")]));
        bridge.install(test_ext_with_deps("a.parent", vec![]));

        assert!(bridge.has_dependency("a.child", "a.parent"));
        assert!(!bridge.has_dependency("a.parent", "a.child"));

        let dependents = bridge.get_dependents("a.parent");
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].id, "a.child");

        assert!(bridge.get_dependents("a.child").is_empty());
    }

    #[test]
    fn get_stats() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        bridge.install(test_ext_ui("ui.one", "alpha", true));
        bridge.install(test_ext_ui("ui.two", "alpha", false));

        let stats = bridge.get_stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.enabled, 2);
        assert_eq!(stats.disabled, 1);
        assert_eq!(stats.by_kind.get("Workspace"), Some(&1));
        assert_eq!(stats.by_kind.get("Ui"), Some(&2));
        assert_eq!(stats.by_publisher.get("alpha"), Some(&2));
        assert_eq!(stats.by_publisher.get("publisher"), Some(&1));
    }

    #[test]
    fn search_extensions_case_insensitive() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        bridge.install(test_ext_ui("other.tool", "other", true));

        let results = bridge.search_extensions("EXTENSION");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "publisher.extension");

        let results = bridge.search_extensions("ext");
        assert_eq!(results.len(), 2);

        assert!(bridge.search_extensions("zzz").is_empty());
    }

    #[test]
    fn search_extensions_by_display_name() {
        let mut bridge = MgmtBridge::new();
        bridge.install(ExtensionInfo {
            id: "x.y".into(),
            display_name: "Fancy Editor Theme".into(),
            version: "1.0.0".into(),
            publisher: "x".into(),
            kind: ExtensionKind::Ui,
            is_enabled: true,
            extension_path: None,
            dependencies: Vec::new(),
        });

        let results = bridge.search_extensions("fancy");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, "Fancy Editor Theme");
    }

    #[test]
    fn extension_dependency_serialization() {
        let ext = test_ext_with_deps("a.child", vec![("a.parent", ">=1.0.0")]);
        let json = serde_json::to_string(&ext).unwrap();
        let back: ExtensionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(ext, back);
        assert_eq!(back.dependencies.len(), 1);
        assert_eq!(back.dependencies[0].id, "a.parent");
    }

    #[test]
    fn extension_info_without_dependencies_field() {
        let json = r#"{
            "id": "x.y",
            "display_name": "Test",
            "version": "1.0.0",
            "publisher": "x",
            "kind": "workspace",
            "is_enabled": true,
            "extension_path": null
        }"#;
        let ext: ExtensionInfo = serde_json::from_str(json).unwrap();
        assert!(ext.dependencies.is_empty());
    }

    #[test]
    fn stats_empty_bridge() {
        let bridge = MgmtBridge::new();
        let stats = bridge.get_stats();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.enabled, 0);
        assert_eq!(stats.disabled, 0);
        assert!(stats.by_kind.is_empty());
        assert!(stats.by_publisher.is_empty());
    }

    // ── Gallery Client Tests ──

    #[test]
    fn gallery_query_url_construction() {
        let q = GalleryQuery::new("rust-analyzer");
        let url = q.to_url("https://open-vsx.org/api");
        assert!(url.contains("query=rust-analyzer"));
        assert!(url.contains("offset=0"));
        assert!(url.contains("size=20"));
        assert!(url.contains("sortBy=relevance"));
        assert!(url.contains("sortOrder=desc"));
    }

    #[test]
    fn gallery_query_url_with_pagination() {
        let q = GalleryQuery::new("python").with_page(2).with_page_size(10);
        let url = q.to_url("https://example.com/api");
        assert!(url.contains("offset=20"));
        assert!(url.contains("size=10"));
    }

    #[test]
    fn gallery_query_url_with_sort() {
        let q = GalleryQuery::new("theme")
            .with_sort(GallerySortBy::DownloadCount, GallerySortOrder::Asc);
        let url = q.to_url("https://example.com/api");
        assert!(url.contains("sortBy=downloadCount"));
        assert!(url.contains("sortOrder=asc"));
    }

    #[test]
    fn gallery_query_url_encodes_spaces() {
        let q = GalleryQuery::new("rust analyzer");
        let url = q.to_url("https://api.test");
        assert!(url.contains("query=rust%20analyzer"));
    }

    #[test]
    fn gallery_query_serialization() {
        let q = GalleryQuery::new("test");
        let json = serde_json::to_string(&q).unwrap();
        let back: GalleryQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(back.text, "test");
        assert_eq!(back.page_size, 20);
    }

    #[test]
    fn gallery_extension_serialization() {
        let ext = GalleryExtension {
            id: "publisher.ext".into(),
            display_name: "My Ext".into(),
            publisher: "publisher".into(),
            version: "1.0.0".into(),
            description: "A test extension".into(),
            download_count: 1000,
            rating: 4.5,
            install_count: 1000,
            download_url: Some("https://example.com/ext.vsix".into()),
        };
        let json = serde_json::to_string(&ext).unwrap();
        let back: GalleryExtension = serde_json::from_str(&json).unwrap();
        assert_eq!(ext, back);
    }

    #[test]
    fn parse_gallery_response_valid() {
        let json = serde_json::json!({
            "extensions": [
                {
                    "namespace": "matklad",
                    "name": "rust-analyzer",
                    "displayName": "Rust Analyzer",
                    "version": "0.3.0",
                    "description": "Rust language support",
                    "downloadCount": 50000,
                    "averageRating": 4.8,
                    "files": {
                        "download": "https://example.com/rust-analyzer.vsix"
                    }
                },
                {
                    "namespace": "ms-python",
                    "name": "python",
                    "displayName": "Python",
                    "version": "2023.1.0",
                    "description": "Python support"
                }
            ]
        });
        let results = parse_gallery_response(&json).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "matklad.rust-analyzer");
        assert_eq!(results[0].display_name, "Rust Analyzer");
        assert_eq!(results[0].download_count, 50000);
        assert!((results[0].rating - 4.8).abs() < f64::EPSILON);
        assert_eq!(
            results[0].download_url,
            Some("https://example.com/rust-analyzer.vsix".into())
        );
        assert_eq!(results[1].id, "ms-python.python");
        assert!(results[1].download_url.is_none());
    }

    #[test]
    fn parse_gallery_response_empty_array() {
        let json = serde_json::json!({"extensions": []});
        let results = parse_gallery_response(&json).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn parse_gallery_response_missing_key() {
        let json = serde_json::json!({"results": []});
        assert!(parse_gallery_response(&json).is_err());
    }

    // ── Extension Manifest / Contributions Tests ──

    #[test]
    fn extension_manifest_deserialize() {
        let json = r#"{
            "name": "my-ext",
            "displayName": "My Extension",
            "publisher": "acme",
            "version": "1.2.3",
            "description": "A nice extension",
            "contributes": {
                "commands": [{"command": "ext.hello", "title": "Hello"}],
                "languages": [{"id": "rust"}],
                "themes": [{"label": "Dark+"}],
                "snippets": [{"language": "rust", "path": "snippets/rust.json"}],
                "grammars": [{"language": "rust", "scopeName": "source.rust"}],
                "keybindings": [{"command": "ext.hello", "key": "ctrl+shift+h"}]
            },
            "extensionDependencies": ["dep.one", "dep.two"]
        }"#;
        let manifest: ExtensionManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "my-ext");
        assert_eq!(manifest.display_name, "My Extension");
        assert_eq!(manifest.publisher, "acme");
        assert_eq!(manifest.version, "1.2.3");
        assert_eq!(manifest.contributes.commands.len(), 1);
        assert_eq!(manifest.contributes.languages.len(), 1);
        assert_eq!(manifest.contributes.themes.len(), 1);
        assert_eq!(manifest.contributes.snippets.len(), 1);
        assert_eq!(manifest.contributes.grammars.len(), 1);
        assert_eq!(manifest.contributes.keybindings.len(), 1);
        assert_eq!(manifest.extension_dependencies.len(), 2);
    }

    #[test]
    fn extension_manifest_defaults() {
        let json = r#"{"name": "minimal", "publisher": "test"}"#;
        let manifest: ExtensionManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "minimal");
        assert!(manifest.contributes.commands.is_empty());
        assert!(manifest.contributes.themes.is_empty());
        assert!(manifest.extension_dependencies.is_empty());
        assert!(manifest.version.is_empty());
    }

    #[test]
    fn extension_contributions_default() {
        let c = ExtensionContributions::default();
        assert!(c.commands.is_empty());
        assert!(c.languages.is_empty());
        assert!(c.themes.is_empty());
        assert!(c.snippets.is_empty());
        assert!(c.keybindings.is_empty());
        assert!(c.grammars.is_empty());
    }

    // ── VSIX Installation Tests ──

    fn create_test_vsix(manifest_json: &str) -> Vec<u8> {
        use std::io::Write;
        let buf = Vec::new();
        let cursor = std::io::Cursor::new(buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("extension/package.json", options).unwrap();
        zip.write_all(manifest_json.as_bytes()).unwrap();
        zip.start_file("extension/README.md", options).unwrap();
        zip.write_all(b"# Hello").unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn install_extension_from_vsix_bytes() {
        let manifest = r#"{
            "name": "test-ext",
            "publisher": "acme",
            "version": "1.0.0",
            "description": "Test"
        }"#;
        let vsix = create_test_vsix(manifest);
        let tmp = tempfile::tempdir().unwrap();
        let result = install_extension_from_bytes(&vsix, tmp.path()).unwrap();
        assert_eq!(result.id, "acme.test-ext");
        assert_eq!(result.version, "1.0.0");
        assert!(result.is_enabled);
        assert!(std::path::Path::new(&result.path).exists());
        // Check extracted files exist
        let pkg = std::path::Path::new(&result.path)
            .join("extension")
            .join("package.json");
        assert!(pkg.exists());
    }

    #[test]
    fn install_extension_overwrites_existing() {
        let manifest = r#"{
            "name": "test-ext",
            "publisher": "acme",
            "version": "1.0.0",
            "description": "v1"
        }"#;
        let vsix = create_test_vsix(manifest);
        let tmp = tempfile::tempdir().unwrap();
        install_extension_from_bytes(&vsix, tmp.path()).unwrap();

        let manifest2 = r#"{
            "name": "test-ext",
            "publisher": "acme",
            "version": "1.0.0",
            "description": "v2"
        }"#;
        let vsix2 = create_test_vsix(manifest2);
        let result = install_extension_from_bytes(&vsix2, tmp.path()).unwrap();
        assert_eq!(result.manifest.description, "v2");
    }

    #[test]
    fn install_extension_invalid_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let result = install_extension_from_bytes(b"not a zip", tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid VSIX ZIP"));
    }

    #[test]
    fn uninstall_extension_on_disk() {
        let manifest = r#"{"name": "removeme", "publisher": "acme", "version": "2.0.0"}"#;
        let vsix = create_test_vsix(manifest);
        let tmp = tempfile::tempdir().unwrap();
        let installed = install_extension_from_bytes(&vsix, tmp.path()).unwrap();
        assert!(std::path::Path::new(&installed.path).exists());

        uninstall_extension("acme.removeme", tmp.path()).unwrap();
        assert!(!std::path::Path::new(&installed.path).exists());
    }

    #[test]
    fn uninstall_extension_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let result = uninstall_extension("not.exist", tmp.path());
        assert!(result.is_err());
    }

    // ── Extension Scanning Tests ──

    #[test]
    fn scan_installed_extensions_finds_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        // Install two extensions
        let m1 = r#"{"name": "ext-one", "publisher": "pub1", "version": "1.0.0"}"#;
        let m2 = r#"{"name": "ext-two", "publisher": "pub2", "version": "2.0.0"}"#;
        install_extension_from_bytes(&create_test_vsix(m1), tmp.path()).unwrap();
        install_extension_from_bytes(&create_test_vsix(m2), tmp.path()).unwrap();

        let scanned = scan_installed_extensions(tmp.path());
        assert_eq!(scanned.len(), 2);
        let ids: Vec<&str> = scanned.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"pub1.ext-one"));
        assert!(ids.contains(&"pub2.ext-two"));
    }

    #[test]
    fn scan_installed_extensions_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let scanned = scan_installed_extensions(tmp.path());
        assert!(scanned.is_empty());
    }

    #[test]
    fn scan_installed_extensions_nonexistent_dir() {
        let scanned = scan_installed_extensions(std::path::Path::new("/nonexistent/path"));
        assert!(scanned.is_empty());
    }

    #[test]
    fn scan_with_enable_state() {
        let tmp = tempfile::tempdir().unwrap();
        let m = r#"{"name": "ext", "publisher": "pub", "version": "1.0.0"}"#;
        install_extension_from_bytes(&create_test_vsix(m), tmp.path()).unwrap();

        let mut state = HashMap::new();
        state.insert("pub.ext".to_string(), false);
        let scanned = scan_installed_extensions_with_state(tmp.path(), &state);
        assert_eq!(scanned.len(), 1);
        assert!(!scanned[0].is_enabled);
    }

    // ── Enable/Disable State Tests ──

    #[test]
    fn enable_disable_state_persistence() {
        let tmp = tempfile::tempdir().unwrap();
        let state_path = tmp.path().join("state.json");

        enable_extension("pub.ext1", &state_path).unwrap();
        disable_extension("pub.ext2", &state_path).unwrap();

        let loaded = load_enable_state(&state_path);
        assert_eq!(loaded.get("pub.ext1"), Some(&true));
        assert_eq!(loaded.get("pub.ext2"), Some(&false));
    }

    #[test]
    fn load_enable_state_missing_file() {
        let state = load_enable_state(std::path::Path::new("/nonexistent/state.json"));
        assert!(state.is_empty());
    }

    #[test]
    fn enable_disable_toggle() {
        let tmp = tempfile::tempdir().unwrap();
        let state_path = tmp.path().join("state.json");

        disable_extension("pub.ext", &state_path).unwrap();
        assert_eq!(load_enable_state(&state_path).get("pub.ext"), Some(&false));

        enable_extension("pub.ext", &state_path).unwrap();
        assert_eq!(load_enable_state(&state_path).get("pub.ext"), Some(&true));
    }

    #[test]
    fn installed_extension_serialization() {
        let ext = InstalledExtension {
            id: "pub.ext".into(),
            version: "1.0.0".into(),
            path: "/some/path".into(),
            is_enabled: true,
            manifest: ExtensionManifest {
                name: "ext".into(),
                display_name: "Extension".into(),
                publisher: "pub".into(),
                version: "1.0.0".into(),
                description: "desc".into(),
                contributes: ExtensionContributions::default(),
                extension_dependencies: vec!["dep.one".into()],
            },
        };
        let json = serde_json::to_string(&ext).unwrap();
        let back: InstalledExtension = serde_json::from_str(&json).unwrap();
        assert_eq!(ext, back);
        assert_eq!(back.manifest.extension_dependencies.len(), 1);
    }

    #[test]
    fn gallery_sort_by_default_is_relevance() {
        assert_eq!(GallerySortBy::default(), GallerySortBy::Relevance);
    }

    #[test]
    fn gallery_sort_order_default_is_desc() {
        assert_eq!(GallerySortOrder::default(), GallerySortOrder::Desc);
    }

    #[test]
    fn extensions_dir_is_under_config() {
        let dir = extensions_dir();
        let dir_str = dir.to_string_lossy();
        assert!(dir_str.contains("vsedit"));
        assert!(dir_str.contains("extensions"));
    }

    // ── SemVer & Version Range Tests ──

    #[test]
    fn semver_parse_and_compare() {
        let v1 = SemVer::parse("1.2.3").unwrap();
        let v2 = SemVer::parse("1.3.0").unwrap();
        let v3 = SemVer::parse("2.0.0").unwrap();
        assert!(!v1.is_newer_than(&v2));
        assert!(v2.is_newer_than(&v1));
        assert!(v3.is_newer_than(&v2));
        assert_eq!(SemVer::parse("bad"), None);
        assert_eq!(v1.to_string(), "1.2.3");
    }

    #[test]
    fn version_range_matching() {
        // caret: same major, >= specified
        assert!(version_satisfies("1.5.0", "^1.2.0"));
        assert!(version_satisfies("1.2.0", "^1.2.0"));
        assert!(!version_satisfies("2.0.0", "^1.2.0"));
        assert!(!version_satisfies("1.1.9", "^1.2.0"));

        // tilde: same major.minor, >= patch
        assert!(version_satisfies("1.2.5", "~1.2.3"));
        assert!(!version_satisfies("1.3.0", "~1.2.3"));

        // comparison operators
        assert!(version_satisfies("2.0.0", ">=1.0.0"));
        assert!(version_satisfies("1.0.0", ">=1.0.0"));
        assert!(!version_satisfies("0.9.0", ">=1.0.0"));
        assert!(version_satisfies("2.0.0", ">1.0.0"));
        assert!(!version_satisfies("1.0.0", ">1.0.0"));
        assert!(version_satisfies("0.9.0", "<1.0.0"));
        assert!(version_satisfies("1.0.0", "<=1.0.0"));

        // exact & wildcard
        assert!(version_satisfies("1.0.0", "=1.0.0"));
        assert!(version_satisfies("1.0.0", "1.0.0"));
        assert!(!version_satisfies("1.0.1", "1.0.0"));
        assert!(version_satisfies("9.9.9", "*"));
    }

    // ── Dependency Resolution Tests ──

    #[test]
    fn check_dependencies_reports_missing_and_incompatible() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext_with_deps("app", vec![("lib-a", "^1.0.0"), ("lib-b", "^2.0.0")]));
        // lib-a installed at correct version
        bridge.install(ExtensionInfo {
            id: "lib-a".into(),
            display_name: "Lib A".into(),
            version: "1.5.0".into(),
            publisher: "acme".into(),
            kind: ExtensionKind::Workspace,
            is_enabled: true,
            extension_path: None,
            dependencies: vec![],
        });
        // lib-b installed but too old
        bridge.install(ExtensionInfo {
            id: "lib-b".into(),
            display_name: "Lib B".into(),
            version: "1.9.0".into(),
            publisher: "acme".into(),
            kind: ExtensionKind::Workspace,
            is_enabled: true,
            extension_path: None,
            dependencies: vec![],
        });

        let errors = bridge.check_dependencies();
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            DependencyError::Incompatible { dep_id, .. } => assert_eq!(dep_id, "lib-b"),
            other => panic!("expected Incompatible, got {:?}", other),
        }
    }

    #[test]
    fn resolve_load_order_topological() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext_with_deps("app", vec![("lib", "^1.0.0")]));
        bridge.install(test_ext_with_deps("lib", vec![("core", "^1.0.0")]));
        bridge.install(test_ext_with_deps("core", vec![]));

        let order = bridge.resolve_load_order().unwrap();
        let ids: Vec<&str> = order.iter().map(|e| e.id.as_str()).collect();
        // core before lib before app
        let core_pos = ids.iter().position(|&id| id == "core").unwrap();
        let lib_pos = ids.iter().position(|&id| id == "lib").unwrap();
        let app_pos = ids.iter().position(|&id| id == "app").unwrap();
        assert!(core_pos < lib_pos);
        assert!(lib_pos < app_pos);
    }

    // ── Cascade Disable Test ──

    #[test]
    fn disable_cascade_disables_dependents() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext_with_deps("core", vec![]));
        bridge.install(test_ext_with_deps("mid", vec![("core", "^1.0.0")]));
        bridge.install(test_ext_with_deps("leaf", vec![("mid", "^1.0.0")]));

        let disabled = bridge.disable_cascade("core");
        assert_eq!(disabled.len(), 3);
        assert!(disabled.contains(&"core".to_string()));
        assert!(disabled.contains(&"mid".to_string()));
        assert!(disabled.contains(&"leaf".to_string()));
        // All should now be disabled
        assert!(bridge.get_enabled_extensions().is_empty());
    }

    // ── Update Checking Test ──

    #[test]
    fn check_updates_finds_newer_versions() {
        let mut bridge = MgmtBridge::new();
        bridge.install(ExtensionInfo {
            id: "a.ext".into(),
            display_name: "A".into(),
            version: "1.0.0".into(),
            publisher: "a".into(),
            kind: ExtensionKind::Workspace,
            is_enabled: true,
            extension_path: None,
            dependencies: vec![],
        });
        bridge.install(ExtensionInfo {
            id: "b.ext".into(),
            display_name: "B".into(),
            version: "2.0.0".into(),
            publisher: "b".into(),
            kind: ExtensionKind::Workspace,
            is_enabled: true,
            extension_path: None,
            dependencies: vec![],
        });

        let mut available = HashMap::new();
        available.insert("a.ext".to_string(), "1.2.0".to_string());
        available.insert("b.ext".to_string(), "2.0.0".to_string()); // same, no update

        let updates = bridge.check_updates(&available);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].extension_id, "a.ext");
        assert_eq!(updates[0].latest_version, "1.2.0");
    }

    // ── Search Ranking Test ──

    #[test]
    fn rank_search_results_orders_by_score() {
        let results = vec![
            GalleryExtension {
                id: "other.tool".into(),
                display_name: "Other Tool".into(),
                publisher: "other".into(),
                version: "1.0.0".into(),
                description: "Has rust in description".into(),
                download_count: 100,
                rating: 3.0,
                install_count: 100,
                download_url: None,
            },
            GalleryExtension {
                id: "rust-lang.rust".into(),
                display_name: "Rust".into(),
                publisher: "rust-lang".into(),
                version: "2.0.0".into(),
                description: "Official Rust support".into(),
                download_count: 500_000,
                rating: 4.9,
                install_count: 500_000,
                download_url: None,
            },
        ];
        let ranked = rank_search_results(&results, "rust");
        assert_eq!(ranked.len(), 2);
        // Exact name match + huge downloads should rank first
        assert_eq!(ranked[0].id, "rust-lang.rust");
        assert!(ranked[0].score > ranked[1].score);
    }

    // ── Rollback Tests ──

    #[test]
    fn rollback_record_and_previous() {
        let mut rb = ExtensionRollback::new();
        rb.record_install("ext.a", "1.0.0", 100);
        assert!(!rb.can_rollback("ext.a"));
        assert_eq!(rb.rollback_target("ext.a"), None);

        rb.record_install("ext.a", "1.1.0", 200);
        assert!(rb.can_rollback("ext.a"));
        let prev = rb.previous_version("ext.a").unwrap();
        assert_eq!(prev.version, "1.0.0");
        assert_eq!(prev.installed_at, 100);
        assert_eq!(rb.rollback_target("ext.a"), Some("1.0.0"));
    }

    #[test]
    fn rollback_version_history_and_total() {
        let mut rb = ExtensionRollback::new();
        rb.record_install("ext.a", "1.0.0", 10);
        rb.record_install("ext.a", "2.0.0", 20);
        rb.record_install("ext.b", "0.1.0", 30);
        assert_eq!(rb.version_history("ext.a").len(), 2);
        assert_eq!(rb.version_history("ext.b").len(), 1);
        assert_eq!(rb.version_history("ext.missing").len(), 0);
        assert_eq!(rb.total_records(), 3);
    }

    // ── Compatibility Tests ──

    #[test]
    fn parse_semver_basic() {
        assert_eq!(parse_semver("1.2.3"), (1, 2, 3));
        assert_eq!(parse_semver("2.0"), (2, 0, 0));
        assert_eq!(parse_semver("3"), (3, 0, 0));
        assert_eq!(parse_semver(""), (0, 0, 0));
    }

    #[test]
    fn semver_compatible_checks() {
        assert!(is_semver_compatible("1.5.0", "1.3.0"));
        assert!(is_semver_compatible("1.3.0", "1.3.0"));
        assert!(!is_semver_compatible("1.2.0", "1.3.0"));
        assert!(!is_semver_compatible("2.5.0", "1.3.0"));
    }

    #[test]
    fn compat_checker_all_ok() {
        let checker = ExtensionCompatibilityChecker::new("1.80.0", "1.5.0");
        let result = checker.check_compatible("1.70.0", Some("1.4.0"));
        assert!(result.is_compatible());
        assert!(result.engine_ok);
        assert!(result.api_ok);
    }

    #[test]
    fn compat_checker_engine_fail() {
        let checker = ExtensionCompatibilityChecker::new("1.60.0", "1.5.0");
        let result = checker.check_compatible("1.70.0", None);
        assert!(!result.is_compatible());
        assert!(!result.engine_ok);
        assert!(result.api_ok);
        assert!(result.to_string().contains("Incompatible"));
    }

    #[test]
    fn compat_result_display() {
        let ok = CompatResult {
            compatible: true,
            engine_ok: true,
            api_ok: true,
            message: "OK".into(),
        };
        assert!(ok.to_string().starts_with("Compatible"));
    }

    // ── Bulk Operation Tests ──

    #[test]
    fn bulk_op_lifecycle() {
        let mut bulk = ExtensionBulkOperation::new();
        bulk.add("ext.a", BulkOpType::Install);
        bulk.add("ext.b", BulkOpType::Update);
        assert_eq!(bulk.pending_count(), 2);
        assert!(!bulk.all_completed());

        assert!(bulk.mark_in_progress("ext.a"));
        assert_eq!(bulk.pending_count(), 1);

        assert!(bulk.mark_completed("ext.a"));
        assert_eq!(bulk.completed_count(), 1);

        assert!(bulk.mark_in_progress("ext.b"));
        assert!(bulk.mark_failed("ext.b", "network error"));
        assert_eq!(bulk.failed_count(), 1);
        assert!(!bulk.all_completed());
    }

    #[test]
    fn bulk_op_summary_format() {
        let mut bulk = ExtensionBulkOperation::new();
        bulk.add("ext.a", BulkOpType::Enable);
        bulk.add("ext.b", BulkOpType::Disable);
        let s = bulk.summary();
        assert!(s.contains("2 total"));
        assert!(s.contains("2 pending"));
    }

    #[test]
    fn bulk_op_display_types() {
        assert_eq!(format!("{}", BulkOpType::Install), "Install");
        assert_eq!(format!("{}", BulkOpType::Uninstall), "Uninstall");
        assert_eq!(format!("{}", BulkOpStatus::Pending), "Pending");
        assert_eq!(
            format!("{}", BulkOpStatus::Failed("oops".into())),
            "Failed: oops"
        );
    }

    // ── Size Calculator Tests ──

    #[test]
    fn format_size_human_readable() {
        assert_eq!(ExtensionSizeCalculator::format_size(42), "42 B");
        assert_eq!(ExtensionSizeCalculator::format_size(1024), "1.0 KB");
        assert_eq!(
            ExtensionSizeCalculator::format_size(1_572_864),
            "1.5 MB"
        );
        assert_eq!(
            ExtensionSizeCalculator::format_size(1_073_741_824),
            "1.0 GB"
        );
    }

    #[test]
    fn estimate_install_size_doubles() {
        assert_eq!(ExtensionSizeCalculator::estimate_install_size(500), 1000);
        assert_eq!(ExtensionSizeCalculator::estimate_install_size(0), 0);
    }

    #[test]
    fn size_category_boundaries() {
        assert_eq!(ExtensionSizeCalculator::size_category(50), SizeCategory::Tiny);
        assert_eq!(
            ExtensionSizeCalculator::size_category(500_000),
            SizeCategory::Small
        );
        assert_eq!(
            ExtensionSizeCalculator::size_category(5_000_000),
            SizeCategory::Medium
        );
        assert_eq!(
            ExtensionSizeCalculator::size_category(50_000_000),
            SizeCategory::Large
        );
        assert_eq!(
            ExtensionSizeCalculator::size_category(500_000_000),
            SizeCategory::Huge
        );
        assert_eq!(format!("{}", SizeCategory::Tiny), "Tiny");
    }

    #[test]
    fn extmgmt_lru_insert_get() {
        let mut c = ExtMgmtLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn extmgmt_lru_eviction() {
        let mut c = ExtMgmtLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn extmgmt_lru_hit_ratio() {
        let mut c = ExtMgmtLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn extmgmt_lru_clear() {
        let mut c = ExtMgmtLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn extmgmt_lru_remove() {
        let mut c = ExtMgmtLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn extmgmt_lru_peek() {
        let mut c = ExtMgmtLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn extmgmt_fmt_list() {
        let f = ExtMgmtFmt::new(ExtMgmtFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn extmgmt_fmt_kv() {
        let f = ExtMgmtFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn extmgmt_fmt_section() {
        let f = ExtMgmtFmt::new(ExtMgmtFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn extmgmt_fmt_truncate() {
        let f = ExtMgmtFmt::new(ExtMgmtFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn extmgmt_fmt_opts_defaults() {
        let o = ExtMgmtFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn ext_mgmt_entry_creation() {
        let e = ExtMgmtEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn ext_mgmt_entry_with_priority() {
        let e = ExtMgmtEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn ext_mgmt_entry_metadata() {
        let e = ExtMgmtEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn ext_mgmt_entry_remove_meta() {
        let mut e = ExtMgmtEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn ext_mgmt_entry_activate_deactivate() {
        let mut e = ExtMgmtEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn ext_mgmt_config_add_sorted() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("lo", "Lo").with_priority(1));
        c.add(ExtMgmtEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn ext_mgmt_config_capacity() {
        let mut c = ExtMgmtConfig::new(1);
        assert!(c.add(ExtMgmtEntry::new("a", "A")));
        assert!(!c.add(ExtMgmtEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn ext_mgmt_config_remove() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn ext_mgmt_config_get() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn ext_mgmt_config_active_entries() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("a", "A"));
        c.add(ExtMgmtEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn ext_mgmt_config_enable_disable() {
        let mut c = ExtMgmtConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn ext_mgmt_config_clear() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn ext_mgmt_config_find_by_label() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn ext_mgmt_config_top_n() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("a", "A").with_priority(1));
        c.add(ExtMgmtEntry::new("b", "B").with_priority(2));
        c.add(ExtMgmtEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn ext_mgmt_config_deactivate_activate_all() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("a", "A"));
        c.add(ExtMgmtEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn ext_mgmt_config_highest_priority() {
        let mut c = ExtMgmtConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(ExtMgmtEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn ext_mgmt_config_contains() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn ext_mgmt_config_labels() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("a", "Alpha"));
        c.add(ExtMgmtEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn ext_mgmt_config_drain_inactive() {
        let mut c = ExtMgmtConfig::new(10);
        c.add(ExtMgmtEntry::new("a", "A"));
        c.add(ExtMgmtEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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


    // xa_ extended tests for ext_mgmt
    #[test]
    fn xa_ext_mgmt_ring_new() {
        let rb = super::XaExtMgmtRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_mgmt_ring_push_len() {
        let mut rb = super::XaExtMgmtRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_mgmt_ring_wrap() {
        let mut rb = super::XaExtMgmtRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_mgmt_ring_mean_empty() {
        let rb = super::XaExtMgmtRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_mgmt_ring_mean_values() {
        let mut rb = super::XaExtMgmtRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_mgmt_ring_min_max() {
        let mut rb = super::XaExtMgmtRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_mgmt_ring_iter() {
        let mut rb = super::XaExtMgmtRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_mgmt_counter_new() {
        let c = super::XaExtMgmtCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_mgmt_counter_inc() {
        let mut c = super::XaExtMgmtCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_mgmt_counter_inc_by() {
        let mut c = super::XaExtMgmtCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_mgmt_counter_reset() {
        let mut c = super::XaExtMgmtCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_mgmt_counter_clear() {
        let mut c = super::XaExtMgmtCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_mgmt_counter_default() {
        let c = super::XaExtMgmtCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 63 ----

    #[test]
    fn xc_63_pool_new_empty() {
        let pool: super::Xc63Pool<i32> = super::Xc63Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_63_pool_release_acquire() {
        let mut pool = super::Xc63Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_63_pool_acquire_empty() {
        let mut pool: super::Xc63Pool<i32> = super::Xc63Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_63_pool_full() {
        let mut pool = super::Xc63Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_63_pool_drain() {
        let mut pool = super::Xc63Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_63_pool_stats() {
        let mut pool = super::Xc63Pool::new(8);
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
    fn xc_63_pool_clear() {
        let mut pool = super::Xc63Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_63_pool_shrink() {
        let mut pool = super::Xc63Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_63_pool_default() {
        let pool: super::Xc63Pool<String> = super::Xc63Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_63_pool_extend() {
        let mut pool = super::Xc63Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_63_pool_retain() {
        let mut pool = super::Xc63Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_63_scheduler_round_robin() {
        let mut sched = super::Xc63Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_63_scheduler_empty() {
        let mut sched = super::Xc63Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_63_scheduler_reset() {
        let mut sched = super::Xc63Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_63_scheduler_add_remove() {
        let mut sched = super::Xc63Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_63_scheduler_targets() {
        let sched = super::Xc63Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_63_hash_empty() {
        assert_eq!(super::xc_63_hash(b""), 5381);
    }

    #[test]
    fn xc_63_hash_data() {
        let h = super::xc_63_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_63_hash(b"hello"), h);
    }

    #[test]
    fn xc_63_reverse_str() {
        assert_eq!(super::xc_63_reverse("abc"), "cba");
        assert_eq!(super::xc_63_reverse(""), "");
    }


    // --- xd_36 deepening tests ---

    #[test]
    fn xd_36_sm_initial_state() {
        let sm = Xd36StateMachine::new();
        assert_eq!(sm.current_state(), Xd36State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_36_sm_valid_idle_to_running() {
        let mut sm = Xd36StateMachine::new();
        assert!(sm.transition(Xd36State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd36State::Running);
    }

    #[test]
    fn xd_36_sm_valid_running_to_paused() {
        let mut sm = Xd36StateMachine::new();
        sm.transition(Xd36State::Running).unwrap();
        assert!(sm.transition(Xd36State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd36State::Paused);
    }

    #[test]
    fn xd_36_sm_valid_running_to_done() {
        let mut sm = Xd36StateMachine::new();
        sm.transition(Xd36State::Running).unwrap();
        assert!(sm.transition(Xd36State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd36State::Done);
    }

    #[test]
    fn xd_36_sm_valid_paused_to_running() {
        let mut sm = Xd36StateMachine::new();
        sm.transition(Xd36State::Running).unwrap();
        sm.transition(Xd36State::Paused).unwrap();
        assert!(sm.transition(Xd36State::Running).is_ok());
    }

    #[test]
    fn xd_36_sm_valid_done_to_idle() {
        let mut sm = Xd36StateMachine::new();
        sm.transition(Xd36State::Running).unwrap();
        sm.transition(Xd36State::Done).unwrap();
        assert!(sm.transition(Xd36State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd36State::Idle);
    }

    #[test]
    fn xd_36_sm_invalid_idle_to_done() {
        let mut sm = Xd36StateMachine::new();
        assert!(sm.transition(Xd36State::Done).is_err());
    }

    #[test]
    fn xd_36_sm_invalid_idle_to_paused() {
        let mut sm = Xd36StateMachine::new();
        assert!(sm.transition(Xd36State::Paused).is_err());
    }

    #[test]
    fn xd_36_sm_history_tracking() {
        let mut sm = Xd36StateMachine::new();
        sm.transition(Xd36State::Running).unwrap();
        sm.transition(Xd36State::Paused).unwrap();
        sm.transition(Xd36State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd36State::Idle);
        assert_eq!(sm.history()[0].to, Xd36State::Running);
        assert_eq!(sm.history()[1].from, Xd36State::Running);
        assert_eq!(sm.history()[2].to, Xd36State::Done);
    }

    #[test]
    fn xd_36_sm_serialize_deserialize() {
        let mut sm = Xd36StateMachine::new();
        sm.transition(Xd36State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd36StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd36State::Running));
    }

    #[test]
    fn xd_36_sm_deserialize_invalid() {
        assert_eq!(Xd36StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_36_sm_reset() {
        let mut sm = Xd36StateMachine::new();
        sm.transition(Xd36State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd36State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_36_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd36EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd36Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_36_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd36EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd36Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd36Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_36_bus_unsubscribe() {
        let mut bus = Xd36EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_36_event_kind_and_payload() {
        let e = Xd36Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd36Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_36_bus_clear_history() {
        let mut bus = Xd36EventBus::new();
        bus.publish(Xd36Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_36_sm_step_counter_increments() {
        let mut sm = Xd36StateMachine::new();
        sm.transition(Xd36State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd36State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #34 --

    #[test]
    fn xf34_trie_insert_search() {
        let mut t = Xf34Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf34_trie_starts_with() {
        let mut t = Xf34Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf34_trie_remove() {
        let mut t = Xf34Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf34_trie_word_count() {
        let mut t = Xf34Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf34_trie_longest_prefix() {
        let mut t = Xf34Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf34_trie_all_words() {
        let mut t = Xf34Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf34_trie_autocomplete() {
        let mut t = Xf34Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf34_trie_empty_search() {
        let t = Xf34Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf34_bloom_add_contains() {
        let mut bf = Xf34BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf34_bloom_probably_absent() {
        let bf = Xf34BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf34_bloom_false_positive_rate() {
        let mut bf = Xf34BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf34_bloom_clear() {
        let mut bf = Xf34BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf34_bloom_union() {
        let mut a = Xf34BloomFilter::xf_new(512, 2);
        let mut b = Xf34BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf34_bloom_intersection_estimate() {
        let mut a = Xf34BloomFilter::xf_new(512, 2);
        let mut b = Xf34BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf34_bloom_union_size_mismatch() {
        let a = Xf34BloomFilter::xf_new(256, 2);
        let b = Xf34BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh62_skip_insert_contains() {
        let mut sl = super::Xh62SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh62_skip_remove() {
        let mut sl = super::Xh62SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh62_skip_len() {
        let mut sl = super::Xh62SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh62_skip_range_query() {
        let mut sl = super::Xh62SkipList::xh_new(4);
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
    fn xh62_skip_floor_ceiling() {
        let mut sl = super::Xh62SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh62_skip_rank() {
        let mut sl = super::Xh62SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh62_skip_empty() {
        let sl = super::Xh62SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh62_skip_duplicates() {
        let mut sl = super::Xh62SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh62_bitset_set_test() {
        let mut bs = super::Xh62BitSet::xh_new(256);
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
    fn xh62_bitset_clear_count() {
        let mut bs = super::Xh62BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh62_bitset_and_or_xor() {
        let mut a = super::Xh62BitSet::xh_new(128);
        let mut b = super::Xh62BitSet::xh_new(128);
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
    fn xh62_bitset_iter_ones() {
        let mut bs = super::Xh62BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh62_bitset_first_last() {
        let mut bs = super::Xh62BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh62_bitset_empty() {
        let bs = super::Xh62BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi62_deque_push_pop_back() {
        let mut dq = super::Xi62Deque::xi_new(4);
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
    fn xi62_deque_push_pop_front() {
        let mut dq = super::Xi62Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi62_deque_mixed_ops() {
        let mut dq = super::Xi62Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi62_deque_get_and_split() {
        let mut dq = super::Xi62Deque::xi_new(8);
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
    fn xi62_deque_rotate_left() {
        let mut dq = super::Xi62Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi62_deque_rotate_right() {
        let mut dq = super::Xi62Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi62_deque_grow() {
        let mut dq = super::Xi62Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi62_deque_empty() {
        let dq = super::Xi62Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi62_interval_tree_insert_query() {
        let mut tree = super::Xi62IntervalTree::xi_new();
        tree.xi_insert(super::Xi62Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi62Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi62Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi62_interval_tree_overlap() {
        let mut tree = super::Xi62IntervalTree::xi_new();
        tree.xi_insert(super::Xi62Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi62Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi62Interval::xi_new(12, 20));
        let q = super::Xi62Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi62_interval_tree_remove() {
        let mut tree = super::Xi62IntervalTree::xi_new();
        tree.xi_insert(super::Xi62Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi62Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi62_interval_tree_gaps() {
        let mut tree = super::Xi62IntervalTree::xi_new();
        tree.xi_insert(super::Xi62Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi62Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi62Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi62Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi62Interval::xi_new(8, 10));
    }

    #[test]
    fn xi62_interval_tree_merge() {
        let mut tree = super::Xi62IntervalTree::xi_new();
        tree.xi_insert(super::Xi62Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi62Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi62Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi62Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi62Interval::xi_new(10, 15));
    }

    #[test]
    fn xi62_interval_tree_all() {
        let mut tree = super::Xi62IntervalTree::xi_new();
        tree.xi_insert(super::Xi62Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi62Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi62_interval_tree_empty() {
        let tree = super::Xi62IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi62_interval_tree_contains_point() {
        let iv = super::Xi62Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 63) ---

    #[test]
    fn xj_63_uf_make_and_find() {
        let mut uf = super::Xj63UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_63_uf_union_connected() {
        let mut uf = super::Xj63UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_63_uf_component_count() {
        let mut uf = super::Xj63UnionFind::xj_new();
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
    fn xj_63_uf_component_size() {
        let mut uf = super::Xj63UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_63_uf_largest_component() {
        let mut uf = super::Xj63UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_63_uf_many_elements() {
        let mut uf = super::Xj63UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_63_uf_separate_components() {
        let mut uf = super::Xj63UnionFind::xj_new();
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
    fn xj_63_uf_path_compression() {
        let mut uf = super::Xj63UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_63_bt_insert_get() {
        let mut bt = super::Xj63BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_63_bt_contains_len() {
        let mut bt = super::Xj63BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_63_bt_replace() {
        let mut bt = super::Xj63BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_63_bt_remove() {
        let mut bt = super::Xj63BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_63_bt_keys_values() {
        let mut bt = super::Xj63BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_63_bt_range() {
        let mut bt = super::Xj63BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_63_bt_min_max() {
        let mut bt = super::Xj63BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_63_bt_many_inserts() {
        let mut bt = super::Xj63BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_62 segment tree tests ---

    #[test]
    fn xk_62_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk62SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_62_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk62SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_62_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk62SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_62_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk62SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_62_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk62SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_62_st_single_element() {
        let data = vec![42];
        let st = super::Xk62SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_62_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk62SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_62_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk62SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_62 disjoint intervals tests ---

    #[test]
    fn xk_62_di_add_and_count() {
        let mut di = super::Xk62DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_62_di_merge_overlap() {
        let mut di = super::Xk62DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_62_di_contains() {
        let mut di = super::Xk62DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_62_di_remove() {
        let mut di = super::Xk62DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_62_di_covered_length() {
        let mut di = super::Xk62DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_62_di_gaps() {
        let mut di = super::Xk62DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_62_di_merge_adjacent() {
        let mut di = super::Xk62DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_62_di_empty() {
        let di = super::Xk62DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_63_rope_new_empty() {
        let rope = super::Xl63Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_63_rope_from_str() {
        let rope = super::Xl63Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_63_rope_insert_at() {
        let mut rope = super::Xl63Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_63_rope_delete_range() {
        let mut rope = super::Xl63Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_63_rope_char_at() {
        let rope = super::Xl63Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_63_rope_split_concat() {
        let rope = super::Xl63Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_63_rope_line_count() {
        let rope = super::Xl63Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_63_rope_line_at() {
        let rope = super::Xl63Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_63_sa_build_and_search() {
        let sa = super::Xl63SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_63_sa_count() {
        let sa = super::Xl63SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_63_sa_longest_repeated() {
        let sa = super::Xl63SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_63_sa_all_positions() {
        let sa = super::Xl63SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_63_sa_len() {
        let sa = super::Xl63SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_63_sa_empty() {
        let sa = super::Xl63SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_63_rope_slice() {
        let rope = super::Xl63Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_63_sa_search_start() {
        let sa = super::Xl63SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}