# vsedit

A full-fidelity terminal port of Visual Studio Code, implemented in Rust.

## Goals

- **Binary extension compatibility** — Run VS Code extensions unmodified via an embedded V8/Deno runtime
- **Configuration compatibility** — Read/write VS Code's `settings.json`, `keybindings.json`, `tasks.json`, `launch.json`
- **Feature parity** — Every VS Code feature rendered in the terminal via Ratatui/Crossterm
- **Performance** — Sub-500ms startup, <16ms keystroke latency

## Architecture

230 Rust crates organized in 7 layers:

| Layer | Count | Description |
|-------|-------|-------------|
| Foundation | 22 | Events, lifecycle, async, URI, collections, key codes |
| TUI | 20 | Terminal rendering, widgets, input, layout |
| Platform | 47 | DI, commands, config, files, keybinding, storage |
| Editor | 41 | Text model (Ropey), cursor, viewmodel, 25+ contributions |
| Workbench | 38 | Shell layout, services, themes, search, terminal |
| Extension | 30 | Extension host, RPC protocol, VS Code API surface |
| Contributions | 32 | File explorer, debug, SCM, notebook, chat |

## Building

```bash
cargo check --workspace   # Type check all crates
cargo build               # Build the main binary
cargo test --workspace    # Run all tests
cargo run                 # Run vsedit
```

## License

MIT
