//! Environment and paths service.
//!
//! Equivalent to VS Code's `vs/platform/environment/common/environment.ts`.
//! Provides well-known paths and CLI arguments for vsedit.

use std::path::PathBuf;

/// Well-known directory paths for vsedit data.
pub struct AppPaths {
    /// User data directory (~/.config/vsedit on Linux).
    pub user_data: PathBuf,
    /// Extensions directory.
    pub extensions: PathBuf,
    /// User settings file.
    pub settings_file: PathBuf,
    /// User keybindings file.
    pub keybindings_file: PathBuf,
    /// Snippets directory.
    pub snippets: PathBuf,
    /// Global storage directory.
    pub global_storage: PathBuf,
    /// Workspace storage directory.
    pub workspace_storage: PathBuf,
    /// Log directory.
    pub logs: PathBuf,
    /// Temp directory.
    pub tmp: PathBuf,
}

impl AppPaths {
    /// Create paths using platform-appropriate defaults.
    pub fn defaults() -> Self {
        let data_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("vsedit");

        Self::from_user_data(data_dir)
    }

    /// Create paths rooted at a specific data directory.
    pub fn from_user_data(user_data: PathBuf) -> Self {
        let extensions = user_data.join("extensions");
        let settings_file = user_data.join("User").join("settings.json");
        let keybindings_file = user_data.join("User").join("keybindings.json");
        let snippets = user_data.join("User").join("snippets");
        let global_storage = user_data.join("globalStorage");
        let workspace_storage = user_data.join("workspaceStorage");
        let logs = user_data.join("logs");
        let tmp = user_data.join("tmp");

        Self {
            user_data,
            extensions,
            settings_file,
            keybindings_file,
            snippets,
            global_storage,
            workspace_storage,
            logs,
            tmp,
        }
    }

    /// Ensure all directories exist.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        for dir in [
            &self.user_data,
            &self.extensions,
            &self.snippets,
            &self.global_storage,
            &self.workspace_storage,
            &self.logs,
            &self.tmp,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        // Ensure parent of settings file exists.
        if let Some(parent) = self.settings_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

/// Parsed CLI arguments.
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    /// Files or folders to open.
    pub paths: Vec<PathBuf>,
    /// Go to line:column in first file.
    pub goto: Option<(u32, u32)>,
    /// Start in diff mode.
    pub diff: bool,
    /// Wait for files to be closed.
    pub wait: bool,
    /// Create new window.
    pub new_window: bool,
    /// Reuse existing window.
    pub reuse_window: bool,
    /// Log level.
    pub log_level: Option<String>,
    /// Extensions dir override.
    pub extensions_dir: Option<PathBuf>,
    /// User data dir override.
    pub user_data_dir: Option<PathBuf>,
    /// Disable all extensions.
    pub disable_extensions: bool,
    /// Verbose output.
    pub verbose: bool,
}

/// The environment service providing runtime information.
pub struct EnvironmentService {
    pub paths: AppPaths,
    pub args: CliArgs,
    pub app_name: String,
    pub app_version: String,
}

impl EnvironmentService {
    pub fn new(args: CliArgs) -> Self {
        let user_data = args
            .user_data_dir
            .clone()
            .unwrap_or_else(|| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("vsedit")
            });

        let mut paths = AppPaths::from_user_data(user_data);
        if let Some(ext_dir) = &args.extensions_dir {
            paths.extensions = ext_dir.clone();
        }

        Self {
            paths,
            args,
            app_name: "vsedit".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn is_verbose(&self) -> bool {
        self.args.verbose
    }

    pub fn is_extensions_disabled(&self) -> bool {
        self.args.disable_extensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_structure() {
        let paths = AppPaths::from_user_data(PathBuf::from("/tmp/test-vsedit"));
        assert_eq!(paths.extensions, PathBuf::from("/tmp/test-vsedit/extensions"));
        assert_eq!(
            paths.settings_file,
            PathBuf::from("/tmp/test-vsedit/User/settings.json")
        );
        assert_eq!(
            paths.keybindings_file,
            PathBuf::from("/tmp/test-vsedit/User/keybindings.json")
        );
        assert_eq!(
            paths.global_storage,
            PathBuf::from("/tmp/test-vsedit/globalStorage")
        );
    }

    #[test]
    fn ensure_dirs_creates_directories() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_user_data(dir.path().join("vsedit"));
        paths.ensure_dirs().unwrap();

        assert!(paths.user_data.is_dir());
        assert!(paths.extensions.is_dir());
        assert!(paths.global_storage.is_dir());
        assert!(paths.logs.is_dir());
    }

    #[test]
    fn environment_service_overrides() {
        let args = CliArgs {
            user_data_dir: Some(PathBuf::from("/custom/data")),
            extensions_dir: Some(PathBuf::from("/custom/ext")),
            verbose: true,
            ..Default::default()
        };
        let env_svc = EnvironmentService::new(args);
        assert_eq!(env_svc.paths.user_data, PathBuf::from("/custom/data"));
        assert_eq!(env_svc.paths.extensions, PathBuf::from("/custom/ext"));
        assert!(env_svc.is_verbose());
    }

    #[test]
    fn cli_args_defaults() {
        let args = CliArgs::default();
        assert!(args.paths.is_empty());
        assert!(!args.diff);
        assert!(!args.wait);
        assert!(!args.disable_extensions);
    }
}
