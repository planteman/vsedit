//! vsedit main binary — terminal port of Visual Studio Code.
//!
//! Entry point that ties together the TUI framework, workbench, input handling,
//! editor controller, and all subsystems into a working terminal editor.
//!
//! ## Startup sequence
//!
//! 1. Parse CLI args (clap)
//! 2. Initialize logging (tracing)
//! 3. Load user settings and keybindings
//! 4. Initialize configuration, theme, workspace, extensions
//! 5. Register commands and default keybindings
//! 6. Initialize terminal backend
//! 7. Open files specified on the command line
//! 8. Enter main event loop

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{
    Event as CtEvent, EventStream, KeyCode, KeyModifiers, MouseButton, MouseEventKind,
};
use futures::StreamExt;

use vsedit_backup::BackupService;
use vsedit_commands::CommandRegistry;
use vsedit_configuration::{
    Configuration, ConfigurationModel, ConfigurationRegistry, ConfigurationService,
    ConfigurationTarget, load_json_file, load_user_settings, register_default_settings,
};
use vsedit_contextkey::{ContextKeyService, ContextKeyValue};
use vsedit_editor_controller::{EditorAction, EditorController};
use vsedit_editor_types::ITextModel;
use vsedit_editor_widget::EditorWidget;
use vsedit_environment::EnvironmentService;
use vsedit_ext_host::ExtensionHostManager;
use vsedit_ext_mgmt::scan_installed_extensions;
use vsedit_ext_testing::TestBridge;
use vsedit_ext_timeline::TimelineBridge;
use vsedit_input::{InputEvent, from_crossterm_key, key_input_to_chord};
use vsedit_keybinding_svc::{
    KeybindingMatch, KeybindingResolver, load_keybindings_json, register_default_keybindings,
};
use vsedit_lifecycle_svc::{LifecyclePhase, LifecycleService, ShutdownReason};
use vsedit_notification_svc::NotificationService;
use vsedit_platform::Platform;
use vsedit_accessibility::ScreenReaderSupport;
use vsedit_remote::RemoteService;
use vsedit_state::{StateScope, StateService};
use vsedit_wb_clipboard::ClipboardService;
use vsedit_theme::dark_plus;
use vsedit_tui::{restore_terminal, setup_terminal};
use vsedit_userdatasync::{SyncResource, SyncService};
use vsedit_workbench::{ActivePanelView, FocusedPart, Workbench, WorkbenchAction};
use vsedit_workspace::Workspace;
use vsedit_lsp::{DiagnosticCollection, Severity as LspSeverity};
use vsedit_debug::BreakpointStore;
use vsedit_terminal::PtySession;
use vsedit_files::watcher::FileWatcher;

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

/// vsedit — A terminal-based code editor inspired by Visual Studio Code.
#[derive(Parser, Debug)]
#[command(name = "vsedit", version, about, long_about = None)]
struct Cli {
    /// Files or folders to open.
    #[arg(value_name = "FILE_OR_FOLDER")]
    paths: Vec<PathBuf>,

    /// Open a diff view between two files.
    #[arg(long, num_args = 2, value_names = ["FILE1", "FILE2"])]
    diff: Vec<PathBuf>,

    /// Open a 3-way merge editor (mine, base, theirs).
    #[arg(long, num_args = 3, value_names = ["MINE", "BASE", "THEIRS"])]
    merge: Vec<PathBuf>,

    /// Open a file at line:col (e.g. --goto src/main.rs:10:5).
    #[arg(long, value_name = "FILE:LINE:COL")]
    goto: Option<String>,

    /// Force open in a new window.
    #[arg(long)]
    new_window: bool,

    /// Wait for the file(s) to be closed before returning.
    #[arg(short, long)]
    wait: bool,

    /// Install an extension by id (publisher.name).
    #[arg(long, value_name = "EXT_ID")]
    install_extension: Option<String>,

    /// List installed extensions.
    #[arg(long)]
    list_extensions: bool,

    /// Reuse an existing window if possible.
    #[arg(long)]
    reuse_window: bool,

    /// Disable all installed extensions.
    #[arg(long)]
    disable_extensions: bool,

    /// Set the log level (trace, debug, info, warn, error).
    #[arg(long, value_name = "LEVEL")]
    log_level: Option<String>,

    /// Override the user data directory.
    #[arg(long, value_name = "DIR")]
    user_data_dir: Option<PathBuf>,

    /// Override the extensions directory.
    #[arg(long, value_name = "DIR")]
    extensions_dir: Option<PathBuf>,

    /// Set the display language locale (e.g. en-US, de, ja).
    #[arg(long, value_name = "LOCALE")]
    locale: Option<String>,

    /// Enable verbose output.
    #[arg(long)]
    verbose: bool,
}

// ---------------------------------------------------------------------------
// Application state — bundles all subsystems
// ---------------------------------------------------------------------------

/// Per-file diagnostic summary from the LSP, used for status bar display.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct LspDiagnostic {
    errors: usize,
    warnings: usize,
    infos: usize,
    hints: usize,
}

#[allow(dead_code)]
struct AppState {
    workbench: Workbench,
    controller: EditorController,
    editor_widget: EditorWidget,
    context_keys: ContextKeyService,
    keybinding_resolver: KeybindingResolver,
    command_registry: CommandRegistry,
    config_service: ConfigurationService,
    lifecycle: LifecycleService,
    state_service: StateService,
    notification_service: NotificationService,
    backup_service: BackupService,
    ext_host: ExtensionHostManager,
    env_service: EnvironmentService,
    timeline_bridge: TimelineBridge,
    test_bridge: TestBridge,
    sync_service: SyncService,
    screen_reader: ScreenReaderSupport,
    remote_service: RemoteService,
    clipboard_service: ClipboardService,
    _workspace: Workspace,
    file_path: Option<PathBuf>,
    /// LSP diagnostic collection across all open files.
    lsp_diagnostics: DiagnosticCollection,
    /// Cached per-file diagnostic summary for the status bar.
    lsp_diagnostic_summary: LspDiagnostic,
    /// DAP breakpoints: file path → sorted list of 1-based line numbers.
    breakpoints: HashMap<PathBuf, Vec<u32>>,
    /// DAP breakpoint store from the debug subsystem.
    breakpoint_store: BreakpointStore,
    /// Active PTY sessions for the integrated terminal panel.
    pty_sessions: Vec<PtySession>,
    /// Whether a debug session is currently active.
    pub debug_active: bool,
    /// File watcher for detecting external modifications.
    pub file_watcher: Option<FileWatcher>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    // Install a panic hook that attempts crash recovery before aborting.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Best-effort: restore terminal so the user's shell is usable.
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        eprintln!("\nvsedit crashed — attempting recovery");
        // Delegate to the default hook for the backtrace / message.
        default_hook(info);
    }));

    if let Err(e) = run().await {
        eprintln!("vsedit: {e}");
        std::process::exit(1);
    }
}

async fn run() -> io::Result<()> {
    let cli = Cli::parse();

    // ── 1. Logging ─────────────────────────────────────────────────────
    let log_level = match cli.log_level.as_deref().unwrap_or("info") {
        "trace" => vsedit_log::LogLevel::Trace,
        "debug" => vsedit_log::LogLevel::Debug,
        "warn" | "warning" => vsedit_log::LogLevel::Warning,
        "error" => vsedit_log::LogLevel::Error,
        _ => vsedit_log::LogLevel::Info,
    };
    vsedit_log::init_tracing(log_level);
    tracing::info!("vsedit starting");

    // ── 2. Product configuration ───────────────────────────────────────
    let product = vsedit_product::ProductConfiguration::default_config();
    tracing::info!("{} v{}", product.name_long, product.version);

    // ── 3. Environment service ─────────────────────────────────────────
    let env_args = cli_to_env_args(&cli);
    if let Err(e) = env_args.validate() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, e.to_string()));
    }
    let env_svc = EnvironmentService::new(env_args);
    if let Err(e) = env_svc.paths.ensure_dirs() {
        tracing::warn!("Could not create data directories: {e}");
    }
    tracing::info!("{}", env_svc.startup_summary());

    // ── 4. Configuration service ───────────────────────────────────────
    let mut config_registry = ConfigurationRegistry::new();
    register_default_settings(&mut config_registry);
    let defaults_model = config_registry.get_defaults();
    let mut configuration = Configuration::with_defaults(defaults_model);

    // Load user settings from disk (~/.config/vsedit/settings.json).
    match load_user_settings() {
        Ok(user_val) => {
            let mut user_model = ConfigurationModel::new();
            if let Some(obj) = user_val.as_object() {
                for (k, v) in obj {
                    user_model.set_value(k, v.clone());
                }
            }
            configuration.set_layer(ConfigurationTarget::User, user_model);
            tracing::info!("Loaded user settings");
        }
        Err(e) => tracing::warn!("Could not load user settings: {e}"),
    }
    let config_service = ConfigurationService::new(configuration);

    // ── 5. Keybindings ─────────────────────────────────────────────────
    let mut keybinding_resolver = KeybindingResolver::new();
    register_default_keybindings(&mut keybinding_resolver);

    // Load user keybindings overlay.
    let kb_path = &env_svc.paths.keybindings_file;
    if kb_path.exists() {
        match std::fs::read_to_string(kb_path) {
            Ok(json_str) => {
                let platform = Platform::current();
                match load_keybindings_json(&json_str, platform) {
                    Ok(rules) => {
                        let count = rules.len();
                        for rule in rules {
                            keybinding_resolver.add_rule(rule);
                        }
                        tracing::info!("Loaded {count} user keybindings");
                    }
                    Err(e) => tracing::warn!("Could not parse keybindings.json: {e}"),
                }
            }
            Err(e) => tracing::warn!("Could not read keybindings file: {e}"),
        }
    }

    // ── 6. Theme ───────────────────────────────────────────────────────
    let theme_name: Option<String> = config_service.get_effective_value("workbench.colorTheme")
        .and_then(|v| serde_json::from_value(v).ok());
    let _theme = match theme_name.as_deref() {
        Some("Monokai") => vsedit_theme::monokai(),
        Some("Solarized Dark") => vsedit_theme::solarized_dark(),
        Some("High Contrast") => vsedit_theme::high_contrast(),
        Some("Light+") | Some("Default Light+") => vsedit_theme::light_plus(),
        _ => dark_plus(),
    };
    tracing::info!("Theme: {} ({})", _theme.label, _theme.id);

    // ── 7. Context keys ────────────────────────────────────────────────
    let context_keys = ContextKeyService::new();
    context_keys.set_context("editorFocus", ContextKeyValue::Bool(true));
    context_keys.set_context("editorTextFocus", ContextKeyValue::Bool(true));
    context_keys.set_context("inputFocus", ContextKeyValue::Bool(false));
    context_keys.set_context("sideBarVisible", ContextKeyValue::Bool(true));
    context_keys.set_context("panelVisible", ContextKeyValue::Bool(false));
    context_keys.set_context("inDebugMode", ContextKeyValue::Bool(false));
    context_keys.set_context(
        "platform",
        ContextKeyValue::String(format!("{:?}", Platform::current())),
    );

    // ── 8. Commands ────────────────────────────────────────────────────
    let command_registry = CommandRegistry::new();
    register_builtin_commands(&command_registry);

    // ── 9. Workspace ───────────────────────────────────────────────────
    let workspace = resolve_workspace(&cli);
    if let Some(root) = workspace.get_workspace_root() {
        tracing::info!("Workspace root: {}", root.display());
    }

    // ── 10. Extensions ─────────────────────────────────────────────────
    let mut ext_host = ExtensionHostManager::new();
    if !env_svc.args.disable_extensions {
        let installed = scan_installed_extensions(&env_svc.paths.extensions);
        for ext in &installed {
            tracing::info!("Extension: {} v{}", ext.id, ext.version);
        }

        // Handle --install-extension (early exit).
        if let Some(ref ext_id) = cli.install_extension {
            return handle_install_extension(ext_id, &env_svc.paths.extensions).await;
        }

        // Handle --list-extensions (early exit).
        if cli.list_extensions {
            for ext in &installed {
                println!("{} ({})", ext.id, ext.version);
            }
            return Ok(());
        }

        // Register scanned extensions with host manager.
        for ext in &installed {
            if let Ok(json_str) = serde_json::to_string(&ext.manifest) {
                let location = vsedit_uri::VsUri::file(&ext.path);
                if let Ok(desc) = vsedit_ext_host::ExtensionDescription::from_package_json(
                    &json_str, location,
                ) {
                    ext_host.register_extension(desc);
                }
            }
        }

        // Start the extension host process (best-effort).
        if let Err(e) = ext_host.start_host() {
            tracing::warn!("Extension host did not start: {e}");
        }
    } else {
        tracing::info!("Extensions disabled via --disable-extensions");
        // Still handle early-exit flags.
        if cli.list_extensions {
            println!("(extensions disabled)");
            return Ok(());
        }
    }

    // ── 11. Lifecycle ──────────────────────────────────────────────────
    let lifecycle = LifecycleService::new();
    lifecycle.set_phase(LifecyclePhase::Starting);

    // ── 12. State persistence ──────────────────────────────────────────
    let mut state_service = StateService::new();
    let state_path = env_svc.paths.user_data.join("state.json");
    load_persisted_state(&state_path, &mut state_service);

    // ── 13. Notification & backup ──────────────────────────────────────
    let notification_service = NotificationService::new();
    let backup_dir = env_svc.paths.user_data.join("backups");
    let backup_service = BackupService::new(backup_dir.to_string_lossy().to_string());

    // ── 13a. Timeline service ──────────────────────────────────────────
    let timeline_bridge = TimelineBridge::new();
    vsedit_ext_timeline::register();
    tracing::info!("Timeline service initialized");

    // ── 13b. Testing service ───────────────────────────────────────────
    let test_bridge = TestBridge::new();
    tracing::info!("Testing service initialized");

    // ── 13c. Settings sync service ─────────────────────────────────────
    let mut sync_service = SyncService::new();
    sync_service.add_resource(SyncResource::Settings);
    sync_service.add_resource(SyncResource::Keybindings);
    sync_service.add_resource(SyncResource::Snippets);
    sync_service.add_resource(SyncResource::Extensions);
    sync_service.add_resource(SyncResource::GlobalState);
    tracing::info!(
        "Settings sync service initialized ({} resources)",
        sync_service.resource_count()
    );

    // ── 13d. Accessibility support ─────────────────────────────────────
    let mut screen_reader = ScreenReaderSupport::new();
    let sr_detected = ScreenReaderSupport::detect_from_env();
    screen_reader.set_active(sr_detected);
    if sr_detected {
        tracing::info!("Screen reader detected — accessibility mode enabled");
    } else {
        tracing::info!("Accessibility support ready (no screen reader detected)");
    }

    // ── 13e. Remote development readiness ──────────────────────────────
    let remote_service = RemoteService::new();
    if remote_service.is_remote() {
        tracing::info!(
            "Remote session active ({} connections)",
            remote_service.connection_count()
        );
    } else {
        tracing::info!("Local session — remote service on standby");
    }

    // ── 13f. LSP bridge ────────────────────────────────────────────────
    // LSP is initialized lazily when a file is opened:
    // 1. Detect language from file extension
    // 2. Look up configured language server for that language
    // 3. Spawn the language server process
    // 4. Send initialize/initialized handshake
    // 5. Send textDocument/didOpen with file content
    // 6. Receive diagnostics via textDocument/publishDiagnostics
    let lsp_diagnostics = DiagnosticCollection::new();
    let lsp_diagnostic_summary = LspDiagnostic::default();
    tracing::info!("LSP bridge ready (lazy initialization on file open)");

    // ── 13g. DAP breakpoint store ──────────────────────────────────────
    let breakpoints: HashMap<PathBuf, Vec<u32>> = HashMap::new();
    let breakpoint_store = BreakpointStore::new();
    tracing::info!("DAP breakpoint store initialized");

    // ── 14. Workbench & editor ─────────────────────────────────────────
    let mut workbench = Workbench::new();
    workbench.start();

    // Determine the file to open (from --goto, positional args, etc.).
    let (file_path, goto_pos) = resolve_open_target(&cli);

    let content = match &file_path {
        Some(path) if path.is_file() => match std::fs::read_to_string(path) {
            Ok(text) => {
                tracing::info!("Opened: {}", path.display());
                text
            }
            Err(e) => {
                tracing::warn!("Could not read {}: {e}", path.display());
                String::new()
            }
        },
        _ => String::new(),
    };

    let mut controller = EditorController::new(&content);
    let mut editor_widget = EditorWidget::new();
    editor_widget.open_text(&content);

    // Open the file as a tab in the workbench.
    if let Some(ref path) = file_path {
        workbench.open_file(path, &content);
    } else {
        workbench.set_editor_content(&controller.model.get_value(), None);
    }

    // Apply --goto position.
    if let Some((line, col)) = goto_pos {
        controller.execute_action(EditorAction::GoToLine(line));
        // Move cursor to column (1-based → 0-based internally).
        for _ in 1..col {
            controller.execute_action(EditorAction::MoveCursorRight);
        }
    }

    let pos = controller.cursors.get_primary().position();
    workbench.set_cursor_info(pos.line, pos.column);

    // Restore previous sidebar / panel state from persisted state.
    restore_ui_state(&state_service, &mut workbench);

    // Scan workspace for Quick Open file list and detect git branch.
    if let Some(root) = workspace.get_workspace_root() {
        workbench.scan_workspace_files(&root);
        tracing::info!("Quick Open: {} workspace files indexed", workbench.workspace_files.len());

        if vsedit_ext_scm::git::GitCli::is_git_repo(&root) {
            let git = vsedit_ext_scm::git::GitCli::new(root.clone());
            if let Ok(branch) = git.current_branch() {
                workbench.statusbar.update_item("statusbar.branch", &format!("⎇ {}", branch));
                tracing::info!("Git branch: {branch}");
            }
        }
    }

    lifecycle.set_phase(LifecyclePhase::Ready);
    tracing::info!("Startup complete — entering event loop");

    // ── 15. Terminal & event loop ──────────────────────────────────────
    let mut terminal = setup_terminal()?;

    let mut app = AppState {
        workbench,
        controller,
        editor_widget,
        context_keys,
        keybinding_resolver,
        command_registry,
        config_service,
        lifecycle,
        state_service,
        notification_service,
        backup_service,
        ext_host,
        env_service: env_svc,
        timeline_bridge,
        test_bridge,
        sync_service,
        screen_reader,
        remote_service,
        clipboard_service: ClipboardService::new(100),
        _workspace: workspace,
        file_path,
        lsp_diagnostics,
        lsp_diagnostic_summary,
        breakpoints,
        breakpoint_store,
        pty_sessions: Vec::new(),
        debug_active: false,
        file_watcher: None,
    };

    app.lifecycle.set_phase(LifecyclePhase::Restored);

    // Restore persisted UI state (sidebar/panel visibility, cursor).
    restore_persisted_ui_state(&mut app);

    // Start watching the open file for external changes.
    if let Some(ref file_path) = app.file_path {
        if let Ok(mut watcher) = FileWatcher::new() {
            if watcher.watch(file_path).is_ok() {
                tracing::info!("Watching file for changes: {}", file_path.display());
                app.file_watcher = Some(watcher);
            }
        }
    }

    // Notify extension host about the initially opened document.
    if let Some(ref file_path) = app.file_path.clone() {
        let content = app.controller.model.get_value();
        notify_ext_did_open(&mut app, file_path, &content);
    }

    let result = run_event_loop(&mut terminal, &mut app).await;

    // ── 16. Shutdown ───────────────────────────────────────────────────
    app.lifecycle.request_shutdown(ShutdownReason::Quit);

    // Auto-save dirty files on exit.
    save_dirty_files(&mut app);

    // Persist UI state.
    persist_ui_state(&app);
    let state_path = app.env_service.paths.user_data.join("state.json");
    save_persisted_state(&state_path, &app.state_service);

    // Stop extension host.
    app.ext_host.stop_host();

    // Kill any remaining PTY sessions.
    for pty in &mut app.pty_sessions {
        let _ = pty.kill();
    }
    app.pty_sessions.clear();

    restore_terminal(&mut terminal)?;
    tracing::info!("vsedit exiting");
    result
}

// ---------------------------------------------------------------------------
// CLI → EnvironmentService bridge
// ---------------------------------------------------------------------------

fn cli_to_env_args(cli: &Cli) -> vsedit_environment::CliArgs {
    let mut paths = cli.paths.clone();
    if !cli.diff.is_empty() {
        paths.extend(cli.diff.iter().cloned());
    }
    if !cli.merge.is_empty() {
        paths.extend(cli.merge.iter().cloned());
    }

    let goto = cli.goto.as_ref().and_then(|g| parse_goto_arg(g));

    vsedit_environment::CliArgs {
        paths,
        goto,
        diff: !cli.diff.is_empty(),
        wait: cli.wait,
        new_window: cli.new_window,
        reuse_window: cli.reuse_window,
        log_level: cli.log_level.clone(),
        extensions_dir: cli.extensions_dir.clone(),
        user_data_dir: cli.user_data_dir.clone(),
        disable_extensions: cli.disable_extensions,
        verbose: cli.verbose,
        merge: !cli.merge.is_empty(),
        locale: cli.locale.clone(),
    }
}

/// Parse `file:line:col` into the file path and a `(line, col)` pair.
fn parse_goto_arg(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.rsplitn(3, ':').collect();
    if parts.len() >= 2 {
        let col: u32 = parts[0].parse().ok()?;
        let line: u32 = parts[1].parse().ok()?;
        if line >= 1 && col >= 1 {
            return Some((line, col));
        }
    }
    None
}

/// Extract the file path from `--goto file:line:col`.
fn parse_goto_file(s: &str) -> Option<PathBuf> {
    // Strip trailing :line:col
    let parts: Vec<&str> = s.rsplitn(3, ':').collect();
    if parts.len() == 3 {
        Some(PathBuf::from(parts[2]))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Workspace resolution
// ---------------------------------------------------------------------------

fn resolve_workspace(cli: &Cli) -> Workspace {
    for p in &cli.paths {
        if p.is_dir() {
            return Workspace::open_folder(p);
        }
        // .code-workspace file — parse and open as multi-root workspace.
        if p.extension().and_then(|e| e.to_str()) == Some("code-workspace") {
            if let Ok(ws_file) = vsedit_workspace::parse_workspace_file(p) {
                // Use the first folder from the workspace file.
                if let Some(first) = ws_file.folders.first() {
                    return Workspace::open_folder(Path::new(&first.path));
                }
            }
        }
        // If a file, use its parent directory as workspace root.
        if p.is_file() {
            if let Some(parent) = p.parent() {
                return Workspace::open_folder(parent);
            }
        }
    }
    Workspace::empty()
}

// ---------------------------------------------------------------------------
// File open target resolution
// ---------------------------------------------------------------------------

fn resolve_open_target(cli: &Cli) -> (Option<PathBuf>, Option<(u32, u32)>) {
    // --goto file:line:col takes priority.
    if let Some(ref goto_str) = cli.goto {
        let path = parse_goto_file(goto_str);
        let pos = parse_goto_arg(goto_str);
        if path.is_some() {
            return (path, pos);
        }
    }
    // First positional arg that is a file.
    for p in &cli.paths {
        if p.is_file() || !p.exists() {
            return (Some(p.clone()), None);
        }
    }
    (None, None)
}

// ---------------------------------------------------------------------------
// Extension install (early-exit path)
// ---------------------------------------------------------------------------

async fn handle_install_extension(ext_id: &str, ext_dir: &Path) -> io::Result<()> {
    println!("Installing extension: {ext_id}...");
    match vsedit_ext_mgmt::install_extension(ext_id, ext_dir).await {
        Ok(installed) => {
            println!("Installed {} v{}", installed.id, installed.version);
            Ok(())
        }
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
    }
}

// ---------------------------------------------------------------------------
// Builtin command registration
// ---------------------------------------------------------------------------

fn register_builtin_commands(registry: &CommandRegistry) {
    let noop = || -> vsedit_commands::CommandHandler {
        Box::new(|_args| Ok(None))
    };

    let cmds: Vec<(&str, vsedit_commands::CommandHandler)> = vec![
        ("workbench.action.quit", noop()),
        ("workbench.action.files.save", noop()),
        ("workbench.action.files.saveAll", noop()),
        ("workbench.action.quickOpen", noop()),
        ("workbench.action.gotoLine", noop()),
        ("workbench.action.showCommands", noop()),
        ("workbench.action.toggleSidebarVisibility", noop()),
        ("workbench.action.togglePanel", noop()),
        ("workbench.action.terminal.toggleTerminal", noop()),
        ("workbench.action.splitEditor", noop()),
        ("workbench.action.focusFirstEditorGroup", noop()),
        ("workbench.action.focusSecondEditorGroup", noop()),
        ("workbench.action.focusThirdEditorGroup", noop()),
        ("workbench.action.tasks.build", noop()),
        ("workbench.action.debug.start", noop()),
        ("editor.action.formatDocument", noop()),
        ("editor.action.commentLine", noop()),
        ("editor.action.addSelectionToNextFindMatch", noop()),
        ("editor.action.selectAllMatches", noop()),
        ("editor.action.triggerSuggest", noop()),
        ("editor.action.goToDeclaration", noop()),
        ("editor.action.peekDefinition", noop()),
        ("editor.action.rename", noop()),
        ("editor.debug.toggleBreakpoint", noop()),
    ];
    vsedit_commands::register_builtin_commands(registry, cmds);
}

// ---------------------------------------------------------------------------
// State persistence
// ---------------------------------------------------------------------------

fn load_persisted_state(path: &Path, state: &mut StateService) {
    if !path.exists() {
        return;
    }
    match load_json_file(path) {
        Ok(val) => {
            if let Some(obj) = val.as_object() {
                for (k, v) in obj {
                    if let Some(s) = v.as_str() {
                        state.set(k.as_str(), s, StateScope::Global);
                    }
                }
            }
            tracing::info!("Restored {} state entries", state.key_count());
        }
        Err(e) => tracing::warn!("Could not load state: {e}"),
    }
}

fn save_persisted_state(path: &Path, state: &StateService) {
    let mut map = serde_json::Map::new();
    for (k, v) in state.get_by_scope(StateScope::Global) {
        map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
    }
    let json = serde_json::Value::Object(map);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(path, serde_json::to_string_pretty(&json).unwrap_or_default()) {
        tracing::warn!("Could not save state: {e}");
    }
}

fn persist_ui_state(app: &AppState) {
    let pos = app.controller.cursors.get_primary().position();
    let state = serde_json::json!({
        "cursor": {
            "line": pos.line,
            "column": pos.column,
        },
        "file": app.file_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        "sidebar_visible": app.workbench.layout.is_part_visible(vsedit_wb_layout::Part::Sidebar),
        "panel_visible": app.workbench.layout.is_part_visible(vsedit_wb_layout::Part::Panel),
        "active_sidebar": format!("{:?}", app.workbench.active_sidebar),
    });

    let state_path = app.env_service.paths.user_data.join("ui-state.json");
    if let Some(parent) = state_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(
        &state_path,
        serde_json::to_string_pretty(&state).unwrap_or_default(),
    ) {
        tracing::warn!("Could not save UI state: {e}");
    }
}

fn restore_ui_state(state: &StateService, workbench: &mut Workbench) {
    if let Some(folder) = state.get("workspace.folder") {
        workbench.workspace_folder = Some(folder.to_string());
    }
}

/// Restore persisted UI state (sidebar/panel visibility, cursor position)
/// from the `ui-state.json` file written at shutdown.
fn restore_persisted_ui_state(app: &mut AppState) {
    let state_path = app.env_service.paths.user_data.join("ui-state.json");
    let content = match std::fs::read_to_string(&state_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let state: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(sidebar) = state.get("sidebar_visible").and_then(|v| v.as_bool()) {
        let currently_visible = app
            .workbench
            .layout
            .is_part_visible(vsedit_wb_layout::Part::Sidebar);
        if sidebar != currently_visible {
            app.workbench.layout.toggle_sidebar();
        }
    }
    if let Some(panel) = state.get("panel_visible").and_then(|v| v.as_bool()) {
        let currently_visible = app
            .workbench
            .layout
            .is_part_visible(vsedit_wb_layout::Part::Panel);
        if panel != currently_visible {
            app.workbench.layout.toggle_panel();
        }
    }
    // Restore cursor position if same file.
    if let Some(saved_file) = state.get("file").and_then(|v| v.as_str()) {
        if let Some(ref current_file) = app.file_path {
            if current_file.to_string_lossy() == saved_file {
                if let Some(line) = state
                    .get("cursor")
                    .and_then(|c| c.get("line"))
                    .and_then(|l| l.as_u64())
                {
                    app.controller
                        .execute_action(EditorAction::GoToLine(line as u32));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Dirty-file auto-save on exit
// ---------------------------------------------------------------------------

fn save_dirty_files(app: &mut AppState) {
    let value = app.controller.model.get_value();
    if !app.workbench.is_modified {
        return;
    }
    // Try to save to the original path.
    let save_path = app
        .workbench
        .tab_service
        .get_active_tab()
        .and_then(|t| t.file_path.clone())
        .or_else(|| app.file_path.clone());

    if let Some(path) = &save_path {
        // Create a backup before saving.
        app.backup_service
            .create_backup(&path.display().to_string(), &value);
    }

    if let Some(path) = save_path {
        match std::fs::write(&path, &value) {
            Ok(()) => tracing::info!("Auto-saved on exit: {}", path.display()),
            Err(e) => tracing::error!("Auto-save failed: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

async fn run_event_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
) -> io::Result<()> {
    let mut event_stream = EventStream::new();
    // 60 fps render target — 16ms frame budget.
    let mut tick_interval = tokio::time::interval(Duration::from_millis(16));
    let mut should_quit = false;

    loop {
        terminal.draw(|frame| app.workbench.render(frame))?;

        if should_quit {
            break;
        }

        tokio::select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        should_quit = handle_event(event, app);
                    }
                    Some(Err(_)) | None => {
                        should_quit = true;
                    }
                }
            }
            _ = tick_interval.tick() => {
                // Periodic housekeeping: dismiss expired notifications, etc.
                // Poll PTY sessions for output and feed into terminal view.
                for pty in &mut app.pty_sessions {
                    if let Ok(data) = pty.read_output() {
                        if !data.is_empty() {
                            app.workbench.terminal_view.process_active_output(&data);
                        }
                    }
                }
                // Poll file watcher for external changes.
                if let Some(ref mut watcher) = app.file_watcher {
                    while let Some(event) = watcher.try_recv() {
                        match event.kind {
                            vsedit_files::watcher::FileChangeKind::Modified => {
                                tracing::info!("File modified externally: {:?}", event.path);
                                if !app.workbench.is_modified {
                                    if let Ok(content) = std::fs::read_to_string(&event.path) {
                                        app.controller = EditorController::new(&content);
                                        app.workbench.set_editor_content(
                                            &content,
                                            event.path.to_str().map(|s| s.to_string()),
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Event dispatch
// ---------------------------------------------------------------------------

/// Returns `true` if the application should quit.
fn handle_event(event: CtEvent, app: &mut AppState) -> bool {
    match event {
        CtEvent::Key(key_event) => handle_key_event(key_event, app),
        CtEvent::Resize(_cols, _rows) => false,
        CtEvent::Mouse(mouse_event) => handle_mouse_event(mouse_event, app),
        CtEvent::Paste(_) | CtEvent::FocusGained | CtEvent::FocusLost => false,
    }
}

fn handle_key_event(key_event: crossterm::event::KeyEvent, app: &mut AppState) -> bool {
    let has_ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
    let has_shift = key_event.modifiers.contains(KeyModifiers::SHIFT);
    let has_alt = key_event.modifiers.contains(KeyModifiers::ALT);

    // Update context keys based on current focus.
    update_context_keys(app);

    // ── Command palette open → route everything through workbench ──────
    if app.workbench.focused == FocusedPart::CommandPalette {
        let input = from_crossterm_key(key_event);
        let action = app.workbench.handle_input(InputEvent::Key(input));
        return handle_workbench_action(&action, app);
    }

    // ── Quick Input (Quick Open / Go To Line) → route through workbench ──
    if app.workbench.focused == FocusedPart::QuickInput {
        let input = from_crossterm_key(key_event);
        let action = app.workbench.handle_input(InputEvent::Key(input));
        return handle_workbench_action(&action, app);
    }

    // ── Terminal panel focused → route keystrokes to PTY ─────────────
    if app.workbench.focused == FocusedPart::Panel
        && app.workbench.active_panel == ActivePanelView::Terminal
        && !app.pty_sessions.is_empty()
    {
        // Allow Ctrl+J and Ctrl+` to toggle panel even from terminal.
        if has_ctrl {
            match key_event.code {
                KeyCode::Char('j') => {
                    return dispatch_command("workbench.action.togglePanel", app);
                }
                KeyCode::Char('`') => {
                    return dispatch_command("workbench.action.terminal.toggleTerminal", app);
                }
                _ => {}
            }
        }
        // Send keystroke to the PTY.
        if let Some(pty) = app.pty_sessions.first_mut() {
            let data = crossterm_key_to_bytes(key_event);
            if !data.is_empty() {
                let _ = pty.write_input(&data);
            }
        }
        return false;
    }

    // ── Find overlay keys ──────────────────────────────────────────────
    if app.editor_widget.show_find && !has_ctrl {
        match key_event.code {
            KeyCode::Esc => {
                app.editor_widget.close_find();
                return false;
            }
            KeyCode::Enter | KeyCode::F(3) => {
                if has_shift {
                    app.editor_widget.find_previous();
                } else {
                    app.editor_widget.find_next();
                }
                return false;
            }
            _ => {}
        }
    }

    // F3/Shift+F3 re-open find if there are matches.
    if !has_ctrl && key_event.code == KeyCode::F(3) && !app.editor_widget.show_find {
        if !app.editor_widget.find_state.matches.is_empty() {
            app.editor_widget.show_find = true;
            if has_shift {
                app.editor_widget.find_previous();
            } else {
                app.editor_widget.find_next();
            }
            return false;
        }
    }

    // ── Try keybinding resolver first ──────────────────────────────────
    let input = from_crossterm_key(key_event);
    let chord = key_input_to_chord(input);
    let now_ms = Instant::now().elapsed().as_millis() as u64;
    let kb_match = app
        .keybinding_resolver
        .resolve_key(&app.context_keys, chord, now_ms);

    match kb_match {
        KeybindingMatch::ExactMatch { ref command, .. } => {
            let cmd = command.clone();
            return dispatch_command(&cmd, app);
        }
        KeybindingMatch::PartialMatch => {
            // Waiting for second chord of a two-part keybinding.
            return false;
        }
        KeybindingMatch::NoMatch => {
            // Fall through to hardcoded handling below.
        }
    }

    // ── Ctrl+key combos ────────────────────────────────────────────────
    if has_ctrl {
        match key_event.code {
            // -- Ctrl+Shift combos --
            KeyCode::Char('k') | KeyCode::Char('K') if has_shift => {
                exec_editor_mutating(app, EditorAction::DeleteLine);
                return false;
            }
            KeyCode::Char('l') | KeyCode::Char('L') if has_shift => {
                app.controller.execute_action(EditorAction::SelectAllOccurrences);
                sync_state(app);
                return false;
            }
            KeyCode::Up if has_shift => {
                app.controller.execute_action(EditorAction::AddCursorAbove);
                sync_state(app);
                return false;
            }
            KeyCode::Down if has_shift => {
                app.controller.execute_action(EditorAction::AddCursorBelow);
                sync_state(app);
                return false;
            }
            KeyCode::Enter if has_shift => {
                exec_editor_mutating(app, EditorAction::InsertLineAbove);
                return false;
            }
            KeyCode::Char('\\') if has_shift => {
                app.controller.execute_action(EditorAction::JumpToMatchingBracket);
                sync_state(app);
                return false;
            }
            // Ctrl+Shift+B → Run build task
            KeyCode::Char('b') | KeyCode::Char('B') if has_shift => {
                return dispatch_command("workbench.action.tasks.build", app);
            }

            // -- Ctrl-only combos --
            KeyCode::Char('p') => {
                // Ctrl+P → Quick Open (file picker)
                return dispatch_command("workbench.action.quickOpen", app);
            }
            KeyCode::Char('g') => {
                // Ctrl+G → Go to Line
                return dispatch_command("workbench.action.gotoLine", app);
            }
            KeyCode::Char('\\') => {
                // Ctrl+\ → Split editor
                return dispatch_command("workbench.action.splitEditor", app);
            }
            KeyCode::Char('j') => {
                // Ctrl+J → Toggle bottom panel
                return dispatch_command("workbench.action.togglePanel", app);
            }
            KeyCode::Char('`') => {
                // Ctrl+` → Toggle terminal
                return dispatch_command("workbench.action.terminal.toggleTerminal", app);
            }
            KeyCode::Char('1') => {
                return dispatch_command("workbench.action.focusFirstEditorGroup", app);
            }
            KeyCode::Char('2') => {
                return dispatch_command("workbench.action.focusSecondEditorGroup", app);
            }
            KeyCode::Char('3') => {
                return dispatch_command("workbench.action.focusThirdEditorGroup", app);
            }
            KeyCode::Char('d') => {
                app.controller.execute_action(EditorAction::AddSelectionToNextFindMatch);
                sync_state(app);
                return false;
            }
            KeyCode::Char('l') => {
                app.controller.execute_action(EditorAction::SelectLine);
                sync_state(app);
                return false;
            }
            KeyCode::Char('/') => {
                exec_editor_mutating(app, EditorAction::ToggleLineComment);
                return false;
            }
            KeyCode::Char(']') => {
                exec_editor_mutating(app, EditorAction::IndentLine);
                return false;
            }
            KeyCode::Char('[') => {
                exec_editor_mutating(app, EditorAction::OutdentLine);
                return false;
            }
            KeyCode::Enter => {
                exec_editor_mutating(app, EditorAction::InsertLineBelow);
                return false;
            }
            KeyCode::Char('f') => {
                app.editor_widget.open_find();
                return false;
            }
            KeyCode::Char('h') => {
                app.editor_widget.open_find();
                if !app.editor_widget.show_replace {
                    app.editor_widget.toggle_replace();
                }
                return false;
            }
            KeyCode::Char('s') => {
                save_active_file(app);
                sync_state(app);
                return false;
            }
            KeyCode::Char('w') => {
                // Ctrl+W → close current tab
                dispatch_command("workbench.action.closeActiveEditor", app);
                load_active_tab_into_controller(app);
                return false;
            }
            KeyCode::Tab if has_shift => {
                // Ctrl+Shift+Tab → previous tab
                switch_to_prev_tab(app);
                return false;
            }
            KeyCode::Tab => {
                // Ctrl+Tab → next tab
                switch_to_next_tab(app);
                return false;
            }
            KeyCode::PageUp => {
                // Ctrl+PageUp → previous tab
                switch_to_prev_tab(app);
                return false;
            }
            KeyCode::PageDown => {
                // Ctrl+PageDown → next tab
                switch_to_next_tab(app);
                return false;
            }
            KeyCode::Char('z') => {
                app.controller.execute_action(EditorAction::Undo);
                sync_state(app);
                return false;
            }
            KeyCode::Char('y') => {
                app.controller.execute_action(EditorAction::Redo);
                sync_state(app);
                return false;
            }
            KeyCode::Char('c') => {
                app.controller.execute_action(EditorAction::Copy);
                let text = app.controller.clipboard.clone();
                app.clipboard_service.write_text(text, 0);
                sync_state(app);
                return false;
            }
            KeyCode::Char('x') => {
                app.controller.execute_action(EditorAction::Cut);
                let text = app.controller.clipboard.clone();
                app.clipboard_service.write_text(text, 0);
                sync_state(app);
                return false;
            }
            KeyCode::Char('v') => {
                if let Some(text) = app.clipboard_service.read_text() {
                    exec_editor_mutating(app, EditorAction::Paste(text.to_string()));
                }
                return false;
            }
            KeyCode::Char('a') => {
                app.controller.execute_action(EditorAction::SelectAll);
                sync_state(app);
                return false;
            }
            KeyCode::Home => {
                app.controller.execute_action(EditorAction::MoveCursorDocumentStart);
                sync_state(app);
                return false;
            }
            KeyCode::End => {
                app.controller.execute_action(EditorAction::MoveCursorDocumentEnd);
                sync_state(app);
                return false;
            }
            _ => {
                // Route through workbench keybinding resolver as fallback.
                let input = from_crossterm_key(key_event);
                let action = app.workbench.handle_input(InputEvent::Key(input));
                return handle_workbench_action(&action, app);
            }
        }
    }

    // ── F9 → Toggle breakpoint ───────────────────────────────────────
    if key_event.code == KeyCode::F(9) && !has_ctrl && !has_alt {
        toggle_breakpoint(app);
        return false;
    }

    // ── F5 → Start debugging / Shift+F5 → Stop debugging ────────────
    if key_event.code == KeyCode::F(5) && !has_ctrl && !has_alt {
        if has_shift && app.debug_active {
            // Stop debugging
            app.debug_active = false;
            app.workbench.statusbar.update_item("statusbar.debug", "");
            app.context_keys
                .set_context("inDebugMode", ContextKeyValue::Bool(false));
            tracing::info!("Debug session stopped");
            return false;
        }
        return dispatch_command("workbench.action.debug.start", app);
    }

    // ── F10 → Step over (debug) ────────────────────────────────────────
    if key_event.code == KeyCode::F(10) && app.debug_active {
        tracing::info!("Step over");
        return false;
    }

    // ── F11 → Step into (debug) ────────────────────────────────────────
    if key_event.code == KeyCode::F(11) && app.debug_active {
        tracing::info!("Step into");
        return false;
    }

    // ── Non-ctrl key events → editor actions ───────────────────────────
    let editor_action = match key_event.code {
        KeyCode::Char(c) if !has_alt => Some(EditorAction::InsertText(c.to_string())),
        KeyCode::Backspace => Some(EditorAction::DeleteLeft),
        KeyCode::Delete => Some(EditorAction::DeleteRight),
        KeyCode::Enter => Some(EditorAction::NewLine),
        KeyCode::Tab => Some(EditorAction::IndentLine),
        KeyCode::Left if has_shift => Some(EditorAction::SelectLeft),
        KeyCode::Left => Some(EditorAction::MoveCursorLeft),
        KeyCode::Right if has_shift => Some(EditorAction::SelectRight),
        KeyCode::Right => Some(EditorAction::MoveCursorRight),
        KeyCode::Up if has_alt => Some(EditorAction::MoveLineUp),
        KeyCode::Up if has_shift => Some(EditorAction::SelectUp),
        KeyCode::Up => Some(EditorAction::MoveCursorUp),
        KeyCode::Down if has_alt => Some(EditorAction::MoveLineDown),
        KeyCode::Down if has_shift => Some(EditorAction::SelectDown),
        KeyCode::Down => Some(EditorAction::MoveCursorDown),
        KeyCode::Home => Some(EditorAction::MoveCursorLineStart),
        KeyCode::End => Some(EditorAction::MoveCursorLineEnd),
        KeyCode::PageUp => Some(EditorAction::PageUp(20)),
        KeyCode::PageDown => Some(EditorAction::PageDown(20)),
        _ => None,
    };

    if let Some(action) = editor_action {
        app.controller.execute_action(action);
        mark_modified(app);
    }
    sync_state(app);
    false
}

// ---------------------------------------------------------------------------
// Command dispatch
// ---------------------------------------------------------------------------

/// Dispatch a named command. Returns `true` if the app should quit.
fn dispatch_command(cmd: &str, app: &mut AppState) -> bool {
    tracing::debug!("dispatch: {cmd}");

    match cmd {
        "workbench.action.quit" => return true,
        "workbench.action.files.save" => {
            save_active_file(app);
            sync_state(app);
        }
        "workbench.action.files.saveAll" => {
            save_all_dirty_tabs(app);
        }
        "workbench.action.quickOpen" => {
            // Route to workbench quick-open (command palette in file mode).
            app.workbench.execute_command(cmd);
        }
        "workbench.action.showCommands" => {
            app.workbench.execute_command(cmd);
        }
        "workbench.action.nextEditor" => {
            switch_to_next_tab(app);
        }
        "workbench.action.previousEditor" => {
            switch_to_prev_tab(app);
        }
        "workbench.action.closeActiveEditor" => {
            // Notify ext host about document close before removing tab.
            let close_path = app.workbench.tab_service.get_active_tab()
                .and_then(|tab| tab.file_path.clone());
            if let Some(fp) = close_path {
                notify_ext_did_close(app, &fp);
            }
            app.workbench.execute_command(cmd);
            load_active_tab_into_controller(app);
        }
        "workbench.action.gotoLine" => {
            app.workbench.open_goto_line();
        }
        "workbench.action.splitEditor" => {
            app.workbench.editor_groups.split_editor(vsedit_workbench::SplitDirection::Right);
        }
        "workbench.action.focusFirstEditorGroup" => {
            app.workbench.editor_groups.focus_group(0);
        }
        "workbench.action.focusSecondEditorGroup" => {
            if app.workbench.editor_groups.group_count() > 1 {
                app.workbench.editor_groups.focus_group(1);
            }
        }
        "workbench.action.focusThirdEditorGroup" => {
            if app.workbench.editor_groups.group_count() > 2 {
                app.workbench.editor_groups.focus_group(2);
            }
        }
        "workbench.action.togglePanel" | "workbench.action.terminal.toggleTerminal" => {
            app.workbench.execute_command(cmd);
            let visible = app.workbench.focused == FocusedPart::Panel;
            app.context_keys
                .set_context("panelVisible", ContextKeyValue::Bool(visible));
            // Spawn a PTY session on first terminal toggle if none exists.
            if visible && app.pty_sessions.is_empty() {
                let shell = vsedit_terminal::detect_default_shell();
                match PtySession::spawn(shell.to_string_lossy().as_ref(), 80, 24) {
                    Ok(pty) => {
                        app.pty_sessions.push(pty);
                        // Add a terminal tab to the view if none exists.
                        if app.workbench.terminal_view.is_terminal_tabs_empty() {
                            app.workbench.terminal_view.add_tab("bash");
                        }
                    }
                    Err(e) => tracing::warn!("Failed to spawn terminal: {e}"),
                }
            }
        }
        "workbench.action.toggleSidebarVisibility" => {
            app.workbench.execute_command(cmd);
            let visible = app.workbench.focused == FocusedPart::Sidebar;
            app.context_keys
                .set_context("sideBarVisible", ContextKeyValue::Bool(visible));
        }
        "workbench.action.debug.start" => {
            // Look for .vscode/launch.json in workspace
            if let Some(root) = app._workspace.get_workspace_root() {
                let launch_path = root.join(".vscode").join("launch.json");
                if launch_path.exists() {
                    match std::fs::read_to_string(&launch_path) {
                        Ok(content) => {
                            match vsedit_json::parse_jsonc(&content) {
                                Ok(config) => {
                                    if let Some(configs) = config.get("configurations").and_then(|c| c.as_array()) {
                                        if let Some(first) = configs.first() {
                                            let program = first.get("program")
                                                .and_then(|p| p.as_str())
                                                .unwrap_or("");
                                            let debug_type = first.get("type")
                                                .and_then(|t| t.as_str())
                                                .unwrap_or("unknown");
                                            tracing::info!("Starting debug session: type={}, program={}", debug_type, program);
                                            app.workbench.statusbar.update_item("statusbar.debug", "⚡ Debugging");
                                            app.debug_active = true;
                                            // Fire onDebug activation event.
                                            tracing::info!("Firing onDebug activation event");
                                            let exts: Vec<String> = app
                                                .ext_host
                                                .should_activate("onDebug")
                                                .iter()
                                                .map(|ext| ext.id.clone())
                                                .collect();
                                            for ext_id in &exts {
                                                app.ext_host.mark_activated(ext_id);
                                            }
                                        }
                                    }
                                }
                                Err(e) => tracing::warn!("Failed to parse launch.json: {e}"),
                            }
                        }
                        Err(e) => tracing::warn!("Failed to read launch.json: {e}"),
                    }
                } else {
                    tracing::info!("No launch.json found — create .vscode/launch.json to configure debugging");
                }
            }
            app.context_keys
                .set_context("inDebugMode", ContextKeyValue::Bool(true));
            app.workbench.execute_command(cmd);
            app.workbench.layout.set_part_visible(vsedit_wb_layout::Part::Panel, true);
        }
        "workbench.action.tasks.build" => {
            app.workbench.execute_command(cmd);
        }
        "editor.action.commentLine" => {
            exec_editor_mutating(app, EditorAction::ToggleLineComment);
        }
        "editor.action.addSelectionToNextFindMatch" => {
            app.controller.execute_action(EditorAction::AddSelectionToNextFindMatch);
            sync_state(app);
        }
        "editor.action.selectAllMatches" => {
            app.controller.execute_action(EditorAction::SelectAllOccurrences);
            sync_state(app);
        }
        "editor.debug.toggleBreakpoint" => {
            toggle_breakpoint(app);
        }
        _ if cmd.starts_with("__gotoLine:") => {
            if let Some(line_str) = cmd.strip_prefix("__gotoLine:") {
                if let Ok(line) = line_str.parse::<u32>() {
                    app.controller.execute_action(EditorAction::GoToLine(line));
                    sync_state(app);
                }
            }
        }
        _ => {
            // Try the command registry, then fall back to workbench.
            if app.command_registry.has(cmd) {
                let _ = app.command_registry.execute(cmd, vec![]);
            } else {
                app.workbench.execute_command(cmd);
            }
        }
    }

    // Fire onCommand activation event for extensions.
    let cmd_event = format!("onCommand:{}", cmd);
    let extensions_to_activate: Vec<String> = app
        .ext_host
        .should_activate(&cmd_event)
        .iter()
        .map(|ext| ext.id.clone())
        .collect();
    for ext_id in &extensions_to_activate {
        tracing::debug!("Firing activation event: {} (extension: {})", cmd_event, ext_id);
        app.ext_host.mark_activated(ext_id);
    }

    // Fire onView activation event when a sidebar panel changes.
    if let Some(view_id) = cmd.strip_prefix("workbench.view.") {
        let view_event = format!("onView:{}", view_id);
        tracing::debug!("Firing onView:{} activation event", view_id);
        let exts: Vec<String> = app
            .ext_host
            .should_activate(&view_event)
            .iter()
            .map(|ext| ext.id.clone())
            .collect();
        for ext_id in &exts {
            app.ext_host.mark_activated(ext_id);
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Workbench action handling
// ---------------------------------------------------------------------------

/// Handle a `WorkbenchAction` returned by the workbench. Returns `true` to quit.
fn handle_workbench_action(action: &WorkbenchAction, app: &mut AppState) -> bool {
    match action {
        WorkbenchAction::ExecuteCommand(cmd) => dispatch_command(cmd, app),
        WorkbenchAction::Quit => true,
        WorkbenchAction::WaitingForChord => false,
        WorkbenchAction::None => false,
    }
}

// ---------------------------------------------------------------------------
// Context key maintenance
// ---------------------------------------------------------------------------

fn update_context_keys(app: &mut AppState) {
    let focused = &app.workbench.focused;
    app.context_keys.set_context(
        "editorFocus",
        ContextKeyValue::Bool(*focused == FocusedPart::Editor),
    );
    app.context_keys.set_context(
        "editorTextFocus",
        ContextKeyValue::Bool(*focused == FocusedPart::Editor),
    );
    app.context_keys.set_context(
        "inputFocus",
        ContextKeyValue::Bool(
            *focused == FocusedPart::CommandPalette || *focused == FocusedPart::QuickInput,
        ),
    );
    app.context_keys.set_context(
        "terminalFocus",
        ContextKeyValue::Bool(
            *focused == FocusedPart::Panel
                && app.workbench.active_panel == ActivePanelView::Terminal,
        ),
    );
}

// ---------------------------------------------------------------------------
// Terminal PTY helpers
// ---------------------------------------------------------------------------

/// Convert a crossterm key event into bytes suitable for writing to a PTY.
fn crossterm_key_to_bytes(key_event: crossterm::event::KeyEvent) -> Vec<u8> {
    let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
    match key_event.code {
        KeyCode::Char(c) if ctrl => {
            // Ctrl+A..Z → 0x01..0x1A
            let byte = (c.to_ascii_lowercase() as u8).wrapping_sub(b'a').wrapping_add(1);
            if byte <= 26 { vec![byte] } else { vec![] }
        }
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            s.as_bytes().to_vec()
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn exec_editor_mutating(app: &mut AppState, action: EditorAction) {
    app.controller.execute_action(action);
    mark_modified(app);
    sync_state(app);
}

fn mark_modified(app: &mut AppState) {
    app.workbench.is_modified = true;
    if let Some(tab) = app.workbench.tab_service.get_active_tab() {
        let id = tab.id;
        app.workbench.tab_service.set_modified(id, true);
    }
}

fn save_active_file(app: &mut AppState) {
    let save_path = app
        .workbench
        .tab_service
        .get_active_tab()
        .and_then(|t| t.file_path.clone())
        .or_else(|| app.file_path.clone());

    if let Some(path) = save_path {
        let value = app.controller.model.get_value();
        // Create backup before overwriting.
        app.backup_service
            .create_backup(&path.display().to_string(), &value);
        match std::fs::write(&path, &value) {
            Ok(()) => {
                tracing::info!("Saved: {}", path.display());
                app.workbench.is_modified = false;
                if let Some(tab) = app.workbench.tab_service.get_active_tab() {
                    let id = tab.id;
                    app.workbench.tab_service.set_modified(id, false);
                }
                // Notify extension host about save.
                notify_ext_did_save(app, &path);
                // Refresh git branch in case of external commits.
                refresh_git_branch(app);
            }
            Err(e) => {
                tracing::error!("Failed to save: {e}");
                app.notification_service
                    .error(format!("Failed to save {}: {e}", path.display()));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Extension host document notifications
// ---------------------------------------------------------------------------

/// Notify the extension host that a document was opened.
fn notify_ext_did_open(app: &mut AppState, path: &std::path::Path, content: &str) {
    let uri = format!("file://{}", path.display());
    let lang_id = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| match ext {
            "rs" => "rust",
            "py" | "pyw" => "python",
            "js" | "mjs" | "cjs" => "javascript",
            "ts" | "mts" | "cts" => "typescript",
            "tsx" => "typescriptreact",
            "jsx" => "javascriptreact",
            "json" | "jsonc" => "json",
            "toml" => "toml",
            "yaml" | "yml" => "yaml",
            "md" | "markdown" => "markdown",
            "html" | "htm" => "html",
            "css" => "css",
            "go" => "go",
            "java" => "java",
            "c" | "h" => "c",
            "cpp" | "cc" | "hpp" => "cpp",
            "rb" | "rake" => "ruby",
            "sh" | "bash" | "zsh" => "shellscript",
            "sql" => "sql",
            "xml" => "xml",
            _ => "plaintext",
        })
        .unwrap_or("plaintext")
        .to_string();

    let event = vsedit_ext_host::handlers::DocumentEvent::DidOpen {
        uri,
        language_id: lang_id,
        version: 1,
        content: content.to_string(),
    };
    let (method, params) = event.to_rpc_notification();
    send_ext_event(app, &method, params);
}

/// Notify the extension host that a document was saved.
fn notify_ext_did_save(app: &mut AppState, path: &std::path::Path) {
    let event = vsedit_ext_host::handlers::DocumentEvent::DidSave {
        uri: format!("file://{}", path.display()),
    };
    let (method, params) = event.to_rpc_notification();
    send_ext_event(app, &method, params);
}

/// Notify the extension host that a document was closed.
fn notify_ext_did_close(app: &mut AppState, path: &std::path::Path) {
    let event = vsedit_ext_host::handlers::DocumentEvent::DidClose {
        uri: format!("file://{}", path.display()),
    };
    let (method, params) = event.to_rpc_notification();
    send_ext_event(app, &method, params);
}

/// Send an RPC event to the extension host process.
fn send_ext_event(app: &mut AppState, event_name: &str, data: serde_json::Value) {
    if let Some(proc) = app.ext_host.process_mut() {
        let msg = vsedit_ext_rpc::RpcMessage::Event(vsedit_ext_rpc::RpcEvent {
            proxy_id: "mainThread".to_string(),
            event_name: event_name.to_string(),
            data,
        });
        if let Err(e) = proc.send_message(&msg) {
            tracing::debug!("Failed to send event to ext host: {e}");
        }
    }
}

/// Re-read the current git branch and update the status bar.
fn refresh_git_branch(app: &mut AppState) {
    if let Some(root) = app._workspace.get_workspace_root() {
        if vsedit_ext_scm::git::GitCli::is_git_repo(&root) {
            let git = vsedit_ext_scm::git::GitCli::new(root);
            if let Ok(branch) = git.current_branch() {
                app.workbench
                    .statusbar
                    .update_item("statusbar.branch", &format!("⎇ {}", branch));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Breakpoint toggling (DAP bridge)
// ---------------------------------------------------------------------------

fn toggle_breakpoint(app: &mut AppState) {
    let line = app.controller.cursors.get_primary().position().line;
    let path = app.file_path.clone().unwrap_or_default();
    let entry = app.breakpoints.entry(path.clone()).or_default();
    if let Some(pos) = entry.iter().position(|&l| l == line) {
        entry.remove(pos);
    } else {
        entry.push(line);
        entry.sort();
    }
    // Mirror into the DAP breakpoint store.
    let path_str = path.display().to_string();
    app.breakpoint_store.toggle_breakpoint(&path_str, line);
    let bp_count: usize = app.breakpoints.values().map(|v| v.len()).sum();
    tracing::debug!("Breakpoint toggled: {}:{} (total: {})", path_str, line, bp_count);
}

// ---------------------------------------------------------------------------
// LSP diagnostics → status bar
// ---------------------------------------------------------------------------

/// Refresh the cached LSP diagnostic summary and update the status bar.
fn update_lsp_status_bar(app: &mut AppState) {
    let errors = app.lsp_diagnostics.count_severity(LspSeverity::Error);
    let warnings = app.lsp_diagnostics.count_severity(LspSeverity::Warning);
    let infos = app.lsp_diagnostics.count_severity(LspSeverity::Info);
    let hints = app.lsp_diagnostics.count_severity(LspSeverity::Hint);
    app.lsp_diagnostic_summary = LspDiagnostic { errors, warnings, infos, hints };
    app.workbench.statusbar.update_item(
        "statusbar.diagnostics",
        &format!("✖ {} ⚠ {}", errors, warnings),
    );
}

fn sync_state(app: &mut AppState) {
    let value = app.controller.model.get_value();
    let path_str = app
        .workbench
        .tab_service
        .get_active_tab()
        .and_then(|t| t.file_path.as_ref())
        .map(|p| p.display().to_string());
    app.workbench.set_editor_content(&value, path_str);
    // Update tab content.
    if let Some(tab) = app.workbench.tab_service.get_active_tab_mut() {
        tab.content = value;
    }
    let pos = app.controller.cursors.get_primary().position();
    app.workbench.set_cursor_info(pos.line, pos.column);
    // Multi-cursor count in statusbar.
    let cursor_count = app.controller.cursors.get_all().len();
    if cursor_count > 1 {
        app.workbench.statusbar.update_item(
            "statusbar.lineColumn",
            &format!(
                "Ln {}, Col {} ({} cursors)",
                pos.line, pos.column, cursor_count
            ),
        );
    }
    // Refresh LSP diagnostics in the status bar.
    update_lsp_status_bar(app);
}

// ---------------------------------------------------------------------------
// Tab switching helpers
// ---------------------------------------------------------------------------

fn switch_to_next_tab(app: &mut AppState) {
    app.workbench.execute_command("workbench.action.nextEditor");
    load_active_tab_into_controller(app);
}

fn switch_to_prev_tab(app: &mut AppState) {
    app.workbench.execute_command("workbench.action.previousEditor");
    load_active_tab_into_controller(app);
}

fn load_active_tab_into_controller(app: &mut AppState) {
    if let Some(tab) = app.workbench.tab_service.get_active_tab() {
        let content = tab.content.clone();
        let line = tab.cursor_line;
        app.controller.model = vsedit_text_model::TextModel::new(&content);
        app.controller.cursors = vsedit_cursor::CursorController::new();
        if line > 0 {
            app.controller.execute_action(EditorAction::GoToLine(line));
        }
    }
    sync_state(app);
}

// ---------------------------------------------------------------------------
// Save all dirty tabs
// ---------------------------------------------------------------------------

fn save_all_dirty_tabs(app: &mut AppState) {
    let tabs: Vec<(usize, Option<PathBuf>, String)> = app
        .workbench
        .tab_service
        .get_tabs()
        .iter()
        .filter(|t| t.is_modified)
        .map(|t| (t.id, t.file_path.clone(), t.content.clone()))
        .collect();

    for (id, path, content) in tabs {
        if let Some(path) = path {
            app.backup_service
                .create_backup(&path.display().to_string(), &content);
            match std::fs::write(&path, &content) {
                Ok(()) => {
                    tracing::info!("Saved: {}", path.display());
                    app.workbench.tab_service.set_modified(id, false);
                }
                Err(e) => {
                    tracing::error!("Failed to save {}: {e}", path.display());
                    app.notification_service
                        .error(format!("Failed to save {}: {e}", path.display()));
                }
            }
        }
    }
    app.workbench.is_modified = false;
    sync_state(app);
}

// ---------------------------------------------------------------------------
// Mouse event handling
// ---------------------------------------------------------------------------

fn handle_mouse_event(
    mouse_event: crossterm::event::MouseEvent,
    app: &mut AppState,
) -> bool {
    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let row = mouse_event.row;
            let col = mouse_event.column;

            if row == 0 {
                // Title bar — ignore.
                return false;
            }

            if row == 1 {
                // Tab bar area — attempt to switch tab by click position.
                // Approximate: each tab is ~20 chars wide.
                let tab_count = app.workbench.tab_service.tab_count();
                if tab_count > 0 {
                    let tab_width = 20u16;
                    let idx = (col / tab_width) as usize;
                    if idx < tab_count {
                        let tabs = app.workbench.tab_service.get_tabs();
                        let id = tabs[idx].id;
                        app.workbench.tab_service.set_active_tab(id);
                        load_active_tab_into_controller(app);
                    }
                }
                return false;
            }

            // Editor area — move cursor to approximate position.
            // Row 2+ maps to editor lines; adjust for header offset.
            let editor_row_offset = 2u16;
            let editor_col_offset = if app.workbench.layout.is_part_visible(
                vsedit_wb_layout::Part::Sidebar,
            ) {
                app.workbench.layout.get_sidebar_width() as u16
            } else {
                0
            };

            if col >= editor_col_offset && row >= editor_row_offset {
                let line = (row - editor_row_offset) as u32 + 1;
                let column = (col - editor_col_offset) as u32 + 1;
                let max_line = app.controller.model.get_line_count();
                let target_line = line.min(max_line).max(1);
                app.controller.execute_action(EditorAction::GoToLine(target_line));
                // GoToLine places cursor at column 1; approximate column.
                let line_len = app.controller.model.get_line_content(target_line).len() as u32;
                let target_col = column.min(line_len + 1).max(1);
                use vsedit_cursor::CursorState;
                use vsedit_editor_types::Position;
                app.controller.cursors.set_state(
                    0,
                    CursorState::from_position(Position::new(target_line, target_col)),
                );
                sync_state(app);
            }
            false
        }
        MouseEventKind::ScrollUp => {
            app.controller.execute_action(EditorAction::PageUp(3));
            sync_state(app);
            false
        }
        MouseEventKind::ScrollDown => {
            app.controller.execute_action(EditorAction::PageDown(3));
            sync_state(app);
            false
        }
        _ => false,
    }
}
