//! Extension install/update management.
//!
//! RPC bridge between the extension host and the main thread for extension management.

use std::collections::HashMap;

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
}
