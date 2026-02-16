//! File explorer view — equivalent to VS Code's explorer sidebar.
//!
//! Provides a tree-based directory browser with expand/collapse, selection,
//! scrolling, file operations, clipboard, keyboard navigation, and rendering
//! via ratatui.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

// ---------------------------------------------------------------------------
// Ignore patterns (.gitignore basic support)
// ---------------------------------------------------------------------------

/// Load ignore patterns from a `.gitignore` file in the given directory.
fn load_gitignore_patterns(dir: &Path) -> HashSet<String> {
    let gitignore = dir.join(".gitignore");
    let mut patterns = HashSet::new();
    if let Ok(content) = std::fs::read_to_string(&gitignore) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Strip trailing slash for directory patterns
            let pat = trimmed.trim_end_matches('/');
            patterns.insert(pat.to_string());
        }
    }
    patterns
}

/// Check if a filename should be ignored based on patterns.
fn should_ignore(name: &str, patterns: &HashSet<String>) -> bool {
    patterns.contains(name)
}

// ---------------------------------------------------------------------------
// File icons
// ---------------------------------------------------------------------------

/// Return a unicode icon for a file based on its name/extension.
pub fn file_icon(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "📁";
    }
    match name.rsplit('.').next() {
        Some("rs") => "🦀",
        Some("toml") => "📦",
        Some("json" | "yaml" | "yml" | "ini" | "cfg" | "conf") => "🔧",
        Some("lock") => "🔧",
        _ => {
            // Config-like filenames without extensions
            if name.starts_with('.') || name == "Makefile" || name == "Dockerfile" {
                "🔧"
            } else {
                "📄"
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FileNode
// ---------------------------------------------------------------------------

/// A single file or directory entry in the explorer tree.
#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub children: Vec<FileNode>,
    pub depth: usize,
    pub is_selected: bool,
}

impl FileNode {
    /// Create a new `FileNode`.
    pub fn new(name: String, path: PathBuf, is_dir: bool, depth: usize) -> Self {
        Self {
            name,
            path,
            is_dir,
            is_expanded: false,
            children: Vec::new(),
            depth,
            is_selected: false,
        }
    }

    /// Load children, optionally filtering out ignored names.
    fn load_children_filtered(&mut self, ignore: &HashSet<String>) {
        self.children.clear();
        if !self.is_dir {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&self.path) else {
            return;
        };
        let mut nodes: Vec<FileNode> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                !should_ignore(&name, ignore)
            })
            .map(|e| {
                let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                let name = e.file_name().to_string_lossy().to_string();
                FileNode::new(name, e.path(), is_dir, self.depth + 1)
            })
            .collect();
        // Sort: directories first, then alphabetically.
        nodes.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self.children = nodes;
    }

    /// Flatten this node and its visible children into a list.
    fn flatten_visible(&self, out: &mut Vec<FlatFileEntry>) {
        out.push(FlatFileEntry {
            name: self.name.clone(),
            path: self.path.clone(),
            is_dir: self.is_dir,
            is_expanded: self.is_expanded,
            depth: self.depth,
        });
        if self.is_expanded {
            for child in &self.children {
                child.flatten_visible(out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FlatFileEntry
// ---------------------------------------------------------------------------

/// A flattened entry produced by walking visible nodes.
#[derive(Debug, Clone)]
pub struct FlatFileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub depth: usize,
}

// ---------------------------------------------------------------------------
// FileTree
// ---------------------------------------------------------------------------

/// Builds and manages a tree of `FileNode`s rooted at a directory.
#[derive(Debug)]
pub struct FileTree {
    roots: Vec<FileNode>,
    root_path: Option<PathBuf>,
    ignore_patterns: HashSet<String>,
}

impl FileTree {
    /// Create an empty tree.
    pub fn new() -> Self {
        Self {
            roots: Vec::new(),
            root_path: None,
            ignore_patterns: HashSet::new(),
        }
    }

    /// Build a tree from a directory path. Only the top level is loaded
    /// initially; subdirectories are loaded on expand.
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        if !path.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "path is not a directory",
            ));
        }
        let ignore = load_gitignore_patterns(path);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let mut root = FileNode::new(name, path.to_path_buf(), true, 0);
        root.load_children_filtered(&ignore);
        root.is_expanded = true;
        Ok(Self {
            roots: vec![root],
            root_path: Some(path.to_path_buf()),
            ignore_patterns: ignore,
        })
    }

    /// Flatten all visible nodes into a list.
    pub fn flatten(&self) -> Vec<FlatFileEntry> {
        let mut out = Vec::new();
        for root in &self.roots {
            root.flatten_visible(&mut out);
        }
        out
    }

    /// Toggle expand/collapse of the node at the given flat index.
    /// When expanding a directory, its children are loaded from disk.
    pub fn toggle(&mut self, flat_index: usize) {
        let path = self.path_for_flat_index(flat_index);
        let ignore = self.ignore_patterns.clone();
        if let Some(node) = Self::node_at_path_mut(&mut self.roots, &path) {
            if node.is_dir {
                node.is_expanded = !node.is_expanded;
                if node.is_expanded && node.children.is_empty() {
                    node.load_children_filtered(&ignore);
                }
            }
        }
    }

    /// Expand the node at the given filesystem path (lazy-load children).
    pub fn expand_node(&mut self, fs_path: &Path) {
        let ignore = self.ignore_patterns.clone();
        if let Some(node) = Self::find_node_by_fs_path_mut(&mut self.roots, fs_path) {
            if node.is_dir && !node.is_expanded {
                node.is_expanded = true;
                if node.children.is_empty() {
                    node.load_children_filtered(&ignore);
                }
            }
        }
    }

    /// Collapse the node at the given filesystem path and free children.
    pub fn collapse_node(&mut self, fs_path: &Path) {
        if let Some(node) = Self::find_node_by_fs_path_mut(&mut self.roots, fs_path) {
            if node.is_dir && node.is_expanded {
                node.is_expanded = false;
                node.children.clear();
            }
        }
    }

    /// Refresh a specific directory node by reloading from disk.
    pub fn refresh_node(&mut self, fs_path: &Path) {
        let ignore = self.ignore_patterns.clone();
        if let Some(node) = Self::find_node_by_fs_path_mut(&mut self.roots, fs_path) {
            if node.is_dir {
                let was_expanded = node.is_expanded;
                node.load_children_filtered(&ignore);
                node.is_expanded = was_expanded;
            }
        }
    }

    /// Total number of currently visible entries.
    pub fn visible_count(&self) -> usize {
        self.flatten().len()
    }

    /// Get a reference to the root path, if set.
    pub fn root_path(&self) -> Option<&Path> {
        self.root_path.as_deref()
    }

    fn path_for_flat_index(&self, target: usize) -> Vec<usize> {
        let mut counter = 0;
        for (i, root) in self.roots.iter().enumerate() {
            let mut path = vec![i];
            if Self::find_path(root, target, &mut counter, &mut path) {
                return path;
            }
        }
        Vec::new()
    }

    fn find_path(
        node: &FileNode,
        target: usize,
        counter: &mut usize,
        path: &mut Vec<usize>,
    ) -> bool {
        if *counter == target {
            return true;
        }
        *counter += 1;
        if node.is_expanded {
            for (i, child) in node.children.iter().enumerate() {
                path.push(i);
                if Self::find_path(child, target, counter, path) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }

    fn node_at_path_mut<'a>(
        nodes: &'a mut [FileNode],
        path: &[usize],
    ) -> Option<&'a mut FileNode> {
        match path {
            [] => None,
            [idx] => nodes.get_mut(*idx),
            [idx, rest @ ..] => nodes
                .get_mut(*idx)
                .and_then(|n| Self::node_at_path_mut(&mut n.children, rest)),
        }
    }

    fn find_node_by_fs_path_mut<'a>(
        nodes: &'a mut [FileNode],
        target: &Path,
    ) -> Option<&'a mut FileNode> {
        for node in nodes.iter_mut() {
            if node.path == target {
                return Some(node);
            }
            if node.is_expanded {
                if let Some(found) = Self::find_node_by_fs_path_mut(&mut node.children, target) {
                    return Some(found);
                }
            }
        }
        None
    }
}

impl Default for FileTree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// File operations
// ---------------------------------------------------------------------------

/// Create a new empty file inside `parent_dir` with the given `name`.
pub fn create_file(parent_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let path = parent_dir.join(name);
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("file already exists: {}", path.display()),
        ));
    }
    std::fs::write(&path, "")?;
    Ok(path)
}

/// Create a new directory inside `parent_dir` with the given `name`.
pub fn create_directory(parent_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let path = parent_dir.join(name);
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("directory already exists: {}", path.display()),
        ));
    }
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Rename a file or directory. `new_name` is the new filename (not full path).
pub fn rename_node(old_path: &Path, new_name: &str) -> std::io::Result<PathBuf> {
    let new_path = old_path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent"))?
        .join(new_name);
    std::fs::rename(old_path, &new_path)?;
    Ok(new_path)
}

/// Delete a file or directory (recursive for directories).
pub fn delete_node(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// Move a file or directory from `from` to `to` (destination is a directory).
pub fn move_node(from: &Path, to_dir: &Path) -> std::io::Result<PathBuf> {
    let name = from
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no filename"))?;
    let dest = to_dir.join(name);
    std::fs::rename(from, &dest)?;
    Ok(dest)
}

/// Duplicate a file with a " Copy" suffix before the extension.
pub fn duplicate_file(path: &Path) -> std::io::Result<PathBuf> {
    if !path.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a file",
        ));
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().to_string());
    let new_name = match ext {
        Some(e) => format!("{stem} Copy.{e}"),
        None => format!("{stem} Copy"),
    };
    let new_path = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent"))?
        .join(new_name);
    std::fs::copy(path, &new_path)?;
    Ok(new_path)
}

// ---------------------------------------------------------------------------
// Clipboard operations
// ---------------------------------------------------------------------------

/// Return the full absolute path as a string.
pub fn copy_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// Return the path relative to a root directory.
pub fn copy_relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

/// Clipboard state for cut/paste operations.
#[derive(Debug, Clone, Default)]
pub struct ClipboardState {
    /// Path that was cut (will be moved on paste).
    pub cut_path: Option<PathBuf>,
    /// Path that was copied (will be duplicated on paste).
    pub copied_path: Option<PathBuf>,
}

impl ClipboardState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a node as cut.
    pub fn cut_node(&mut self, path: PathBuf) {
        self.cut_path = Some(path);
        self.copied_path = None;
    }

    /// Mark a node as copied.
    pub fn copy_node(&mut self, path: PathBuf) {
        self.copied_path = Some(path);
        self.cut_path = None;
    }

    /// Paste into a destination directory. Returns the new path.
    pub fn paste_node(&mut self, dest_dir: &Path) -> std::io::Result<PathBuf> {
        if let Some(src) = self.cut_path.take() {
            move_node(&src, dest_dir)
        } else if let Some(ref src) = self.copied_path {
            let name = src
                .file_name()
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "no filename")
                })?;
            let dest = dest_dir.join(name);
            if src.is_dir() {
                copy_dir_recursive(src, &dest)?;
            } else {
                std::fs::copy(src, &dest)?;
            }
            Ok(dest)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "nothing to paste",
            ))
        }
    }

    pub fn has_content(&self) -> bool {
        self.cut_path.is_some() || self.copied_path.is_some()
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_child = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_child)?;
        } else {
            std::fs::copy(entry.path(), &dest_child)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ExplorerAction — returned by keyboard handler
// ---------------------------------------------------------------------------

/// Actions that the explorer can request from the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorerAction {
    /// No action needed.
    None,
    /// Open the file at the given path in an editor.
    OpenFile(PathBuf),
    /// Request to delete the node (caller should confirm).
    DeleteRequest(PathBuf),
    /// Start renaming the selected node.
    RenameRequest(PathBuf),
    /// Create a new folder dialog.
    NewFolderRequest,
    /// The tree was navigated (selection changed).
    Navigated,
}

// ---------------------------------------------------------------------------
// ExplorerView
// ---------------------------------------------------------------------------

/// The file explorer view with tree navigation, selection, and rendering.
pub struct ExplorerView {
    tree: FileTree,
    selected_index: usize,
    scroll_offset: usize,
    clipboard: ClipboardState,
}

impl ExplorerView {
    /// Create a new empty `ExplorerView`.
    pub fn new() -> Self {
        Self {
            tree: FileTree::new(),
            selected_index: 0,
            scroll_offset: 0,
            clipboard: ClipboardState::new(),
        }
    }

    /// Open a directory as the explorer root.
    pub fn open_directory(&mut self, path: &Path) {
        if let Ok(tree) = FileTree::from_path(path) {
            self.tree = tree;
            self.selected_index = 0;
            self.scroll_offset = 0;
        }
    }

    /// Get a reference to the underlying file tree.
    pub fn tree(&self) -> &FileTree {
        &self.tree
    }

    /// Get a mutable reference to the underlying file tree.
    pub fn tree_mut(&mut self) -> &mut FileTree {
        &mut self.tree
    }

    /// Get a reference to the clipboard state.
    pub fn clipboard(&self) -> &ClipboardState {
        &self.clipboard
    }

    /// Get a mutable reference to the clipboard state.
    pub fn clipboard_mut(&mut self) -> &mut ClipboardState {
        &mut self.clipboard
    }

    /// Get the path of the currently selected entry, if any.
    pub fn get_selected_path(&self) -> Option<PathBuf> {
        let flat = self.tree.flatten();
        flat.get(self.selected_index)
            .map(|entry| entry.path.clone())
    }

    /// Get the selected entry, if any.
    pub fn get_selected_entry(&self) -> Option<FlatFileEntry> {
        let flat = self.tree.flatten();
        flat.get(self.selected_index).cloned()
    }

    /// Move selection up by one row.
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
        self.ensure_visible();
    }

    /// Move selection down by one row.
    pub fn move_down(&mut self) {
        let count = self.tree.visible_count();
        if count > 0 && self.selected_index + 1 < count {
            self.selected_index += 1;
        }
        self.ensure_visible();
    }

    /// Toggle expand/collapse on the selected node.
    pub fn toggle_expand(&mut self) {
        self.tree.toggle(self.selected_index);
    }

    /// Convenience: return the selected entry if it is a file.
    pub fn select_file(&self) -> Option<PathBuf> {
        let flat = self.tree.flatten();
        flat.get(self.selected_index)
            .filter(|e| !e.is_dir)
            .map(|e| e.path.clone())
    }

    /// Current selected index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Current scroll offset.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Handle a key event and return the resulting action.
    pub fn handle_key(&mut self, key: KeyEvent) -> ExplorerAction {
        match key.code {
            KeyCode::Up => {
                self.move_up();
                ExplorerAction::Navigated
            }
            KeyCode::Down => {
                self.move_down();
                ExplorerAction::Navigated
            }
            KeyCode::Enter => {
                let flat = self.tree.flatten();
                if let Some(entry) = flat.get(self.selected_index) {
                    if entry.is_dir {
                        self.toggle_expand();
                        ExplorerAction::Navigated
                    } else {
                        let path = entry.path.clone();
                        ExplorerAction::OpenFile(path)
                    }
                } else {
                    ExplorerAction::None
                }
            }
            KeyCode::Char(' ') => {
                self.toggle_expand();
                ExplorerAction::Navigated
            }
            KeyCode::Delete => {
                if let Some(path) = self.get_selected_path() {
                    ExplorerAction::DeleteRequest(path)
                } else {
                    ExplorerAction::None
                }
            }
            KeyCode::F(2) => {
                if let Some(path) = self.get_selected_path() {
                    ExplorerAction::RenameRequest(path)
                } else {
                    ExplorerAction::None
                }
            }
            KeyCode::Char('N' | 'n')
                if key
                    .modifiers
                    .contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT) =>
            {
                ExplorerAction::NewFolderRequest
            }
            _ => ExplorerAction::None,
        }
    }

    /// Render the file tree into a ratatui buffer with tree lines and icons.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_tree(area, buf, self.scroll_offset);
    }

    /// Render the file tree with an explicit scroll offset.
    pub fn render_tree(&self, area: Rect, buf: &mut Buffer, scroll_offset: usize) {
        let entries = self.tree.flatten();
        let height = area.height as usize;

        for i in 0..height {
            let entry_idx = scroll_offset + i;
            if entry_idx >= entries.len() {
                break;
            }
            let entry = &entries[entry_idx];

            // Build tree-line prefix
            let indent = build_tree_indent(entry.depth, entry_idx, &entries);

            // Expand/collapse arrow or space
            let arrow = if entry.is_dir {
                if entry.is_expanded {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };

            let icon = file_icon(&entry.name, entry.is_dir);
            let text = format!("{indent}{arrow}{icon} {}", entry.name);

            let style = if entry_idx == self.selected_index {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else if entry.is_dir {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };

            let line = Line::from(Span::styled(text, style));
            let y = area.y + i as u16;
            let line_area = Rect::new(area.x, y, area.width, 1);
            line.render(line_area, buf);
        }
    }

    fn ensure_visible(&mut self) {
        // We don't know the viewport height until render time, so we use a
        // conservative default when scrolling outside a render call.
        let viewport = 20;
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + viewport {
            self.scroll_offset = self.selected_index - viewport + 1;
        }
    }
}

impl Default for ExplorerView {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tree indent rendering helpers
// ---------------------------------------------------------------------------

/// Build the tree-line indentation string (│, ├, └) for a node.
fn build_tree_indent(depth: usize, entry_idx: usize, entries: &[FlatFileEntry]) -> String {
    if depth == 0 {
        return String::new();
    }
    let mut parts: Vec<&str> = Vec::with_capacity(depth);

    for d in 1..depth {
        // Check if there's a sibling at this ancestor depth level after us
        if has_sibling_after(entry_idx, d, entries) {
            parts.push("│ ");
        } else {
            parts.push("  ");
        }
    }

    // Last level: are we the last child?
    if is_last_sibling(entry_idx, depth, entries) {
        parts.push("└ ");
    } else {
        parts.push("├ ");
    }

    parts.concat()
}

/// Check if there are more siblings at a given depth level after entry_idx.
fn has_sibling_after(entry_idx: usize, depth: usize, entries: &[FlatFileEntry]) -> bool {
    for e in &entries[entry_idx + 1..] {
        if e.depth < depth {
            return false;
        }
        if e.depth == depth {
            return true;
        }
    }
    false
}

/// Check if entry_idx is the last sibling at its depth level.
fn is_last_sibling(entry_idx: usize, depth: usize, entries: &[FlatFileEntry]) -> bool {
    for e in &entries[entry_idx + 1..] {
        if e.depth < depth {
            return true;
        }
        if e.depth == depth {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: create a temp directory with a known structure.
    fn make_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        std::fs::create_dir(base.join("src")).unwrap();
        std::fs::write(base.join("src").join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(base.join("src").join("lib.rs"), "// lib").unwrap();
        std::fs::create_dir(base.join("tests")).unwrap();
        std::fs::write(base.join("tests").join("test1.rs"), "").unwrap();
        std::fs::write(base.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(base.join("README.md"), "# readme").unwrap();
        dir
    }

    #[test]
    fn file_node_creation() {
        let node = FileNode::new(
            "hello.rs".to_string(),
            PathBuf::from("/tmp/hello.rs"),
            false,
            0,
        );
        assert_eq!(node.name, "hello.rs");
        assert!(!node.is_dir);
        assert!(!node.is_expanded);
        assert!(node.children.is_empty());
        assert_eq!(node.depth, 0);
    }

    #[test]
    fn file_tree_from_path() {
        let dir = make_test_dir();
        let tree = FileTree::from_path(dir.path()).unwrap();
        let flat = tree.flatten();
        // Root is expanded: root + its children (src, tests, Cargo.toml, README.md)
        assert!(flat.len() >= 5);
        // First entry is the root dir itself.
        assert!(flat[0].is_dir);
    }

    #[test]
    fn file_tree_sorting_dirs_first() {
        let dir = make_test_dir();
        let tree = FileTree::from_path(dir.path()).unwrap();
        let flat = tree.flatten();
        // After root (index 0), directories should come before files.
        let children = &flat[1..];
        let first_file_idx = children.iter().position(|e| !e.is_dir);
        let last_dir_idx = children.iter().rposition(|e| e.is_dir);
        if let (Some(fi), Some(di)) = (first_file_idx, last_dir_idx) {
            assert!(di < fi, "dirs should sort before files");
        }
    }

    #[test]
    fn file_tree_toggle_expand() {
        let dir = make_test_dir();
        let mut tree = FileTree::from_path(dir.path()).unwrap();
        let before = tree.visible_count();
        // Find the "src" directory entry and expand it.
        let flat = tree.flatten();
        let src_idx = flat.iter().position(|e| e.name == "src").unwrap();
        tree.toggle(src_idx);
        let after = tree.visible_count();
        // Expanding src should add its children (main.rs, lib.rs).
        assert!(after > before);
    }

    #[test]
    fn file_tree_toggle_collapse() {
        let dir = make_test_dir();
        let mut tree = FileTree::from_path(dir.path()).unwrap();
        // Expand src.
        let flat = tree.flatten();
        let src_idx = flat.iter().position(|e| e.name == "src").unwrap();
        tree.toggle(src_idx);
        let expanded_count = tree.visible_count();
        // Collapse src.
        let flat = tree.flatten();
        let src_idx = flat.iter().position(|e| e.name == "src").unwrap();
        tree.toggle(src_idx);
        let collapsed_count = tree.visible_count();
        assert!(collapsed_count < expanded_count);
    }

    #[test]
    fn file_tree_empty() {
        let tree = FileTree::new();
        assert_eq!(tree.visible_count(), 0);
        assert!(tree.flatten().is_empty());
    }

    #[test]
    fn explorer_view_open_directory() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        assert!(view.get_selected_path().is_some());
        assert_eq!(view.selected_index(), 0);
    }

    #[test]
    fn explorer_view_navigation() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        let first = view.get_selected_path().unwrap();
        view.move_down();
        let second = view.get_selected_path().unwrap();
        assert_ne!(first, second);
        view.move_up();
        let back = view.get_selected_path().unwrap();
        assert_eq!(first, back);
    }

    #[test]
    fn explorer_view_move_up_at_top() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        view.move_up();
        assert_eq!(view.selected_index(), 0);
    }

    #[test]
    fn explorer_view_move_down_at_bottom() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        let count = view.tree.visible_count();
        for _ in 0..count + 5 {
            view.move_down();
        }
        assert_eq!(view.selected_index(), count - 1);
    }

    #[test]
    fn explorer_view_toggle_expand() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        // Navigate to "src" (first child after root).
        view.move_down();
        let flat = view.tree.flatten();
        let entry = &flat[view.selected_index()];
        assert!(entry.is_dir, "should be on a directory");
        let before = view.tree.visible_count();
        view.toggle_expand();
        let after = view.tree.visible_count();
        assert!(after > before, "expanding should show more entries");
    }

    #[test]
    fn explorer_view_select_file() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        // Root is a directory — select_file should return None.
        assert!(view.select_file().is_none());
        // Navigate to a file entry.
        let flat = view.tree.flatten();
        let file_idx = flat.iter().position(|e| !e.is_dir).unwrap();
        for _ in 0..file_idx {
            view.move_down();
        }
        assert!(view.select_file().is_some());
    }

    #[test]
    fn explorer_view_render_does_not_panic() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        // Just verify no panic and something was written.
        let content = buf.content().iter().any(|c| c.symbol() != " ");
        assert!(content, "buffer should contain rendered text");
    }

    #[test]
    fn explorer_view_default() {
        let view = ExplorerView::default();
        assert_eq!(view.selected_index(), 0);
        assert_eq!(view.scroll_offset(), 0);
        assert!(view.get_selected_path().is_none());
    }

    #[test]
    fn file_tree_from_non_directory_fails() {
        let dir = make_test_dir();
        let file_path = dir.path().join("Cargo.toml");
        let result = FileTree::from_path(&file_path);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // New tests for enhanced functionality
    // -----------------------------------------------------------------------

    #[test]
    fn create_file_success() {
        let dir = TempDir::new().unwrap();
        let result = create_file(dir.path(), "new.txt");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    }

    #[test]
    fn create_file_already_exists() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("existing.txt"), "data").unwrap();
        let result = create_file(dir.path(), "existing.txt");
        assert!(result.is_err());
    }

    #[test]
    fn create_directory_success() {
        let dir = TempDir::new().unwrap();
        let result = create_directory(dir.path(), "subdir");
        assert!(result.is_ok());
        assert!(result.unwrap().is_dir());
    }

    #[test]
    fn create_directory_already_exists() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        let result = create_directory(dir.path(), "subdir");
        assert!(result.is_err());
    }

    #[test]
    fn rename_node_success() {
        let dir = TempDir::new().unwrap();
        let old = dir.path().join("old.txt");
        std::fs::write(&old, "content").unwrap();
        let result = rename_node(&old, "new.txt");
        assert!(result.is_ok());
        let new_path = result.unwrap();
        assert!(new_path.exists());
        assert!(!old.exists());
        assert_eq!(std::fs::read_to_string(&new_path).unwrap(), "content");
    }

    #[test]
    fn delete_node_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("del.txt");
        std::fs::write(&file, "bye").unwrap();
        assert!(delete_node(&file).is_ok());
        assert!(!file.exists());
    }

    #[test]
    fn delete_node_directory() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("subdir");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("inner.txt"), "data").unwrap();
        assert!(delete_node(&sub).is_ok());
        assert!(!sub.exists());
    }

    #[test]
    fn move_node_success() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("file.txt");
        let dest_dir = dir.path().join("dest");
        std::fs::write(&src, "data").unwrap();
        std::fs::create_dir(&dest_dir).unwrap();
        let result = move_node(&src, &dest_dir);
        assert!(result.is_ok());
        assert!(!src.exists());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn duplicate_file_success() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("hello.rs");
        std::fs::write(&src, "fn main() {}").unwrap();
        let result = duplicate_file(&src);
        assert!(result.is_ok());
        let dup = result.unwrap();
        assert!(dup.exists());
        assert_eq!(dup.file_name().unwrap().to_str().unwrap(), "hello Copy.rs");
        assert_eq!(
            std::fs::read_to_string(&dup).unwrap(),
            "fn main() {}"
        );
    }

    #[test]
    fn duplicate_file_no_extension() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("Makefile");
        std::fs::write(&src, "all:").unwrap();
        let dup = duplicate_file(&src).unwrap();
        assert_eq!(dup.file_name().unwrap().to_str().unwrap(), "Makefile Copy");
    }

    #[test]
    fn copy_path_returns_full() {
        let p = PathBuf::from("/home/user/project/src/main.rs");
        assert_eq!(copy_path(&p), "/home/user/project/src/main.rs");
    }

    #[test]
    fn copy_relative_path_strips_root() {
        let root = PathBuf::from("/home/user/project");
        let full = PathBuf::from("/home/user/project/src/main.rs");
        assert_eq!(copy_relative_path(&full, &root), "src/main.rs");
    }

    #[test]
    fn clipboard_cut_paste() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("a.txt");
        let dest = dir.path().join("dest");
        std::fs::write(&src, "hello").unwrap();
        std::fs::create_dir(&dest).unwrap();
        let mut clip = ClipboardState::new();
        clip.cut_node(src.clone());
        assert!(clip.has_content());
        let result = clip.paste_node(&dest);
        assert!(result.is_ok());
        assert!(!src.exists());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn clipboard_copy_paste() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("b.txt");
        let dest = dir.path().join("dest");
        std::fs::write(&src, "world").unwrap();
        std::fs::create_dir(&dest).unwrap();
        let mut clip = ClipboardState::new();
        clip.copy_node(src.clone());
        let result = clip.paste_node(&dest);
        assert!(result.is_ok());
        // Source still exists (copy, not move)
        assert!(src.exists());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn clipboard_empty_paste_fails() {
        let dir = TempDir::new().unwrap();
        let mut clip = ClipboardState::new();
        assert!(!clip.has_content());
        assert!(clip.paste_node(dir.path()).is_err());
    }

    #[test]
    fn file_icon_mapping() {
        assert_eq!(file_icon("main.rs", false), "🦀");
        assert_eq!(file_icon("Cargo.toml", false), "📦");
        assert_eq!(file_icon("config.json", false), "🔧");
        assert_eq!(file_icon("settings.yaml", false), "🔧");
        assert_eq!(file_icon("readme.txt", false), "📄");
        assert_eq!(file_icon("src", true), "📁");
        assert_eq!(file_icon(".gitignore", false), "🔧");
        assert_eq!(file_icon("Makefile", false), "🔧");
    }

    #[test]
    fn gitignore_filtering() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();
        std::fs::write(base.join(".gitignore"), "target\n*.log\n").unwrap();
        std::fs::create_dir(base.join("target")).unwrap();
        std::fs::create_dir(base.join("src")).unwrap();
        std::fs::write(base.join("src").join("main.rs"), "").unwrap();
        let tree = FileTree::from_path(base).unwrap();
        let flat = tree.flatten();
        // "target" should be filtered out
        assert!(
            !flat.iter().any(|e| e.name == "target"),
            "target should be ignored"
        );
        // "src" should still be visible
        assert!(flat.iter().any(|e| e.name == "src"));
    }

    #[test]
    fn expand_and_collapse_by_path() {
        let dir = make_test_dir();
        let mut tree = FileTree::from_path(dir.path()).unwrap();
        let src_path = dir.path().join("src");
        let before = tree.visible_count();
        tree.expand_node(&src_path);
        let after_expand = tree.visible_count();
        assert!(after_expand > before, "expand should show children");
        tree.collapse_node(&src_path);
        let after_collapse = tree.visible_count();
        assert_eq!(after_collapse, before, "collapse should hide children");
    }

    #[test]
    fn refresh_node_picks_up_changes() {
        let dir = make_test_dir();
        let mut tree = FileTree::from_path(dir.path()).unwrap();
        let src_path = dir.path().join("src");
        tree.expand_node(&src_path);
        let before = tree.visible_count();
        // Add a new file on disk
        std::fs::write(src_path.join("added.rs"), "").unwrap();
        tree.refresh_node(&src_path);
        let after = tree.visible_count();
        assert!(after > before, "refresh should show new file");
    }

    #[test]
    fn handle_key_up_down() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        let action = view.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(action, ExplorerAction::Navigated);
        assert_eq!(view.selected_index(), 1);
        let action = view.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(action, ExplorerAction::Navigated);
        assert_eq!(view.selected_index(), 0);
    }

    #[test]
    fn handle_key_enter_on_file() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        // Navigate to a file entry
        let flat = view.tree.flatten();
        let file_idx = flat.iter().position(|e| !e.is_dir).unwrap();
        for _ in 0..file_idx {
            view.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        let action = view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match action {
            ExplorerAction::OpenFile(p) => assert!(p.exists()),
            other => panic!("expected OpenFile, got {:?}", other),
        }
    }

    #[test]
    fn handle_key_enter_on_dir_toggles() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        // Move to "src" directory
        view.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let before = view.tree.visible_count();
        let action = view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(action, ExplorerAction::Navigated);
        assert!(view.tree.visible_count() > before);
    }

    #[test]
    fn handle_key_delete() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        view.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let action = view.handle_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
        match action {
            ExplorerAction::DeleteRequest(p) => assert!(p.exists()),
            other => panic!("expected DeleteRequest, got {:?}", other),
        }
    }

    #[test]
    fn handle_key_f2_rename() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        view.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let action = view.handle_key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE));
        match action {
            ExplorerAction::RenameRequest(p) => assert!(p.exists()),
            other => panic!("expected RenameRequest, got {:?}", other),
        }
    }

    #[test]
    fn handle_key_space_toggles() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        view.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let before = view.tree.visible_count();
        let action = view.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert_eq!(action, ExplorerAction::Navigated);
        assert_ne!(view.tree.visible_count(), before);
    }

    #[test]
    fn handle_key_ctrl_shift_n() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        let action = view.handle_key(KeyEvent::new(
            KeyCode::Char('N'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));
        assert_eq!(action, ExplorerAction::NewFolderRequest);
    }

    #[test]
    fn handle_key_unknown_returns_none() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        let action = view.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
        assert_eq!(action, ExplorerAction::None);
    }

    #[test]
    fn render_tree_with_scroll_offset() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        let area = Rect::new(0, 0, 60, 3);
        let mut buf = Buffer::empty(area);
        // Render with scroll offset 1 (skip root)
        view.render_tree(area, &mut buf, 1);
        let content = buf.content().iter().any(|c| c.symbol() != " ");
        assert!(content, "buffer should contain rendered text");
    }

    #[test]
    fn tree_indent_builds_correctly() {
        let entries = vec![
            FlatFileEntry {
                name: "root".into(),
                path: PathBuf::from("/root"),
                is_dir: true,
                is_expanded: true,
                depth: 0,
            },
            FlatFileEntry {
                name: "src".into(),
                path: PathBuf::from("/root/src"),
                is_dir: true,
                is_expanded: false,
                depth: 1,
            },
            FlatFileEntry {
                name: "README.md".into(),
                path: PathBuf::from("/root/README.md"),
                is_dir: false,
                is_expanded: false,
                depth: 1,
            },
        ];
        let indent_root = build_tree_indent(0, 0, &entries);
        assert_eq!(indent_root, "");
        let indent_src = build_tree_indent(1, 1, &entries);
        assert_eq!(indent_src, "├ ");
        let indent_readme = build_tree_indent(1, 2, &entries);
        assert_eq!(indent_readme, "└ ");
    }

    #[test]
    fn get_selected_entry() {
        let dir = make_test_dir();
        let mut view = ExplorerView::new();
        view.open_directory(dir.path());
        let entry = view.get_selected_entry();
        assert!(entry.is_some());
        assert!(entry.unwrap().is_dir);
    }

    #[test]
    fn copy_dir_recursive_works() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src_dir");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("a.txt"), "aaa").unwrap();
        std::fs::create_dir(src.join("sub")).unwrap();
        std::fs::write(src.join("sub").join("b.txt"), "bbb").unwrap();

        let dest = dir.path().join("dest_dir");
        copy_dir_recursive(&src, &dest).unwrap();
        assert!(dest.join("a.txt").exists());
        assert!(dest.join("sub").join("b.txt").exists());
        assert_eq!(std::fs::read_to_string(dest.join("a.txt")).unwrap(), "aaa");
    }
}
