//! Extension directory scanner.
//!
//! Walks an extensions directory looking for `package.json` files at a depth
//! of exactly one subdirectory and parses them into [`ExtensionDescription`]s.

use std::path::Path;

use vsedit_uri::VsUri;

use crate::ExtensionDescription;

/// Scan `dir` for VS Code-style extensions.
///
/// Each immediate subdirectory of `dir` that contains a `package.json` is
/// treated as an extension root. Returns all successfully parsed extensions;
/// directories with invalid or missing `package.json` files are silently
/// skipped.
pub fn scan_extensions(dir: &Path) -> Vec<ExtensionDescription> {
    let mut results = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(path = %dir.display(), error = %e, "could not read extensions directory");
            return results;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let pkg_path = path.join("package.json");
        let json = match std::fs::read_to_string(&pkg_path) {
            Ok(j) => j,
            Err(_) => continue,
        };
        let location = VsUri::file(&path.to_string_lossy());
        match ExtensionDescription::from_package_json(&json, location) {
            Ok(ext) => {
                tracing::debug!(id = %ext.id, "scanned extension");
                results.push(ext);
            }
            Err(e) => {
                tracing::warn!(path = %pkg_path.display(), error = %e, "failed to parse extension");
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn scan_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let exts = scan_extensions(tmp.path());
        assert!(exts.is_empty());
    }

    #[test]
    fn scan_nonexistent_directory() {
        let exts = scan_extensions(Path::new("/tmp/vsedit-definitely-does-not-exist-999"));
        assert!(exts.is_empty());
    }

    #[test]
    fn scan_finds_valid_extension() {
        let tmp = TempDir::new().unwrap();
        let ext_dir = tmp.path().join("my-ext");
        std::fs::create_dir(&ext_dir).unwrap();
        std::fs::write(
            ext_dir.join("package.json"),
            r#"{
                "name": "my-ext",
                "version": "0.1.0",
                "publisher": "test",
                "main": "./out/main.js",
                "activationEvents": ["onLanguage:rust"]
            }"#,
        )
        .unwrap();

        let exts = scan_extensions(tmp.path());
        assert_eq!(exts.len(), 1);
        assert_eq!(exts[0].id, "test.my-ext");
        assert_eq!(exts[0].name, "my-ext");
        assert_eq!(exts[0].main.as_deref(), Some("./out/main.js"));
    }

    #[test]
    fn scan_skips_invalid_package_json() {
        let tmp = TempDir::new().unwrap();

        // Valid extension
        let good = tmp.path().join("good-ext");
        std::fs::create_dir(&good).unwrap();
        std::fs::write(
            good.join("package.json"),
            r#"{ "name": "good", "publisher": "p" }"#,
        )
        .unwrap();

        // Invalid JSON
        let bad = tmp.path().join("bad-ext");
        std::fs::create_dir(&bad).unwrap();
        std::fs::write(bad.join("package.json"), "not valid json {{{").unwrap();

        // No package.json
        let empty = tmp.path().join("empty-ext");
        std::fs::create_dir(&empty).unwrap();

        let exts = scan_extensions(tmp.path());
        assert_eq!(exts.len(), 1);
        assert_eq!(exts[0].name, "good");
    }

    #[test]
    fn scan_ignores_files_at_top_level() {
        let tmp = TempDir::new().unwrap();
        // A file (not a directory) at the top level should be ignored.
        std::fs::write(tmp.path().join("stray-file.txt"), "hello").unwrap();
        let exts = scan_extensions(tmp.path());
        assert!(exts.is_empty());
    }

    #[test]
    fn scan_multiple_extensions() {
        let tmp = TempDir::new().unwrap();
        for name in &["alpha", "beta", "gamma"] {
            let d = tmp.path().join(name);
            std::fs::create_dir(&d).unwrap();
            std::fs::write(
                d.join("package.json"),
                format!(r#"{{ "name": "{name}", "publisher": "org" }}"#),
            )
            .unwrap();
        }
        let exts = scan_extensions(tmp.path());
        assert_eq!(exts.len(), 3);
    }
}
