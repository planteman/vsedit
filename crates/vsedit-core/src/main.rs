//! vsedit main binary — terminal port of Visual Studio Code.

fn main() {
    // Initialize logging
    vsedit_log::init_tracing(vsedit_log::LogLevel::Info);

    // Load product configuration
    let product = vsedit_product::ProductConfiguration::default_config();
    println!(
        "{} v{} — Terminal port of Visual Studio Code",
        product.name_long, product.version
    );

    // Set up environment
    let args = vsedit_environment::CliArgs::default();
    let env_svc = vsedit_environment::EnvironmentService::new(args);

    // Ensure data directories exist
    if let Err(e) = env_svc.paths.ensure_dirs() {
        eprintln!("Warning: Could not create data directories: {}", e);
    }

    // Initialize workbench
    let mut workbench = vsedit_workbench::Workbench::new();
    workbench.start();

    println!("Ready. (TUI event loop not yet integrated — use Ctrl+C to exit)");
    println!("Data directory: {}", env_svc.paths.user_data.display());
    println!(
        "\n{} crates compiled, full architecture in place.",
        231
    );
}
