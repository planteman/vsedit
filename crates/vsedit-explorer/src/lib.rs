//! File explorer view — equivalent to VS Code's explorer sidebar.
//!
//! Provides a tree-based directory browser with expand/collapse, selection,
//! scrolling, file operations, clipboard, keyboard navigation, and rendering
//! via ratatui.

use std::collections::HashSet;
use std::fmt;
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
// FileSearchResult — search-within-explorer results
// ---------------------------------------------------------------------------

/// A result from searching for files within the explorer tree.
#[derive(Debug, Clone, PartialEq)]
pub struct FileSearchResult {
    /// Full path to the matching file.
    pub path: PathBuf,
    /// Relevance score (higher is better). Exact prefix matches score highest.
    pub score: u32,
    /// Byte positions within the file name where the query matched.
    pub match_positions: Vec<usize>,
}

impl FileSearchResult {
    /// Create a new search result.
    pub fn new(path: PathBuf, score: u32, match_positions: Vec<usize>) -> Self {
        Self {
            path,
            score,
            match_positions,
        }
    }

    /// The file name component of the matched path.
    pub fn file_name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
    }
}

impl fmt::Display for FileSearchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (score: {})", self.path.display(), self.score)
    }
}

/// Perform a fuzzy file-name search over a flat list of entries.
///
/// Returns results sorted by descending score. The algorithm gives higher
/// scores to exact prefix matches and consecutive character runs.
pub fn search_files(entries: &[FlatFileEntry], query: &str) -> Vec<FileSearchResult> {
    if query.is_empty() {
        return Vec::new();
    }
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let name_lower = entry.name.to_lowercase();
        if let Some((score, positions)) = fuzzy_match(&name_lower, &query_lower) {
            results.push(FileSearchResult::new(entry.path.clone(), score, positions));
        }
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results
}

/// Simple fuzzy matcher: returns (score, match_positions) if all query chars
/// appear in order within `text`. Consecutive matches and prefix matches
/// score higher.
fn fuzzy_match(text: &str, query: &str) -> Option<(u32, Vec<usize>)> {
    let text_bytes = text.as_bytes();
    let query_bytes = query.as_bytes();
    let mut positions = Vec::with_capacity(query_bytes.len());
    let mut ti = 0;
    for &qb in query_bytes {
        let mut found = false;
        while ti < text_bytes.len() {
            if text_bytes[ti] == qb {
                positions.push(ti);
                ti += 1;
                found = true;
                break;
            }
            ti += 1;
        }
        if !found {
            return None;
        }
    }

    // Score: base 10 per matched char, +5 for consecutive, +10 for starting at 0
    let mut score: u32 = positions.len() as u32 * 10;
    if positions.first() == Some(&0) {
        score += 10;
    }
    for w in positions.windows(2) {
        if w[1] == w[0] + 1 {
            score += 5;
        }
    }
    Some((score, positions))
}

// ---------------------------------------------------------------------------
// FileFilter — include/exclude files by pattern
// ---------------------------------------------------------------------------

/// Filter specification for showing/hiding files in the explorer.
#[derive(Debug, Clone, Default)]
pub struct FileFilter {
    /// File extensions to include (e.g. `["rs", "toml"]`). Empty means all.
    pub include_extensions: Vec<String>,
    /// File extensions to exclude (e.g. `["log", "tmp"]`).
    pub exclude_extensions: Vec<String>,
    /// Exact file names to exclude.
    pub exclude_names: Vec<String>,
    /// Whether to hide hidden files (names starting with `.`).
    pub hide_hidden: bool,
}

impl FileFilter {
    /// Create a new empty filter that accepts everything.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a file entry passes through this filter.
    /// Directories always pass to preserve tree structure.
    pub fn matches(&self, name: &str, is_dir: bool) -> bool {
        if is_dir {
            return true;
        }
        if self.hide_hidden && name.starts_with('.') {
            return false;
        }
        if self.exclude_names.iter().any(|n| n == name) {
            return false;
        }
        let ext = name.rsplit('.').next().unwrap_or("");
        if self
            .exclude_extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(ext))
        {
            return false;
        }
        if !self.include_extensions.is_empty()
            && !self
                .include_extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case(ext))
        {
            return false;
        }
        true
    }

    /// Apply this filter to a list of flat entries, returning only those that pass.
    pub fn apply<'a>(&self, entries: &'a [FlatFileEntry]) -> Vec<&'a FlatFileEntry> {
        entries
            .iter()
            .filter(|e| self.matches(&e.name, e.is_dir))
            .collect()
    }
}

impl fmt::Display for FileFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if !self.include_extensions.is_empty() {
            parts.push(format!("+[{}]", self.include_extensions.join(",")));
        }
        if !self.exclude_extensions.is_empty() {
            parts.push(format!("-[{}]", self.exclude_extensions.join(",")));
        }
        if self.hide_hidden {
            parts.push("no-hidden".into());
        }
        if parts.is_empty() {
            write!(f, "FileFilter(all)")
        } else {
            write!(f, "FileFilter({})", parts.join(" "))
        }
    }
}

// ---------------------------------------------------------------------------
// DirectoryStats — aggregate statistics for a directory tree
// ---------------------------------------------------------------------------

/// Aggregate statistics computed from a directory tree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirectoryStats {
    /// Total number of files.
    pub file_count: usize,
    /// Total number of directories (not counting the root).
    pub dir_count: usize,
    /// Estimated total size in bytes (sum of file sizes).
    pub total_size: u64,
    /// Deepest nesting level encountered.
    pub deepest_level: usize,
}

impl DirectoryStats {
    /// Compute stats by walking a directory on disk.
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let mut stats = Self::default();
        Self::walk(path, 0, &mut stats)?;
        Ok(stats)
    }

    fn walk(dir: &Path, depth: usize, stats: &mut Self) -> std::io::Result<()> {
        if depth > stats.deepest_level {
            stats.deepest_level = depth;
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stats.dir_count += 1;
                Self::walk(&entry.path(), depth + 1, stats)?;
            } else if ft.is_file() {
                stats.file_count += 1;
                stats.total_size += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        Ok(())
    }

    /// Compute stats from an already-flattened entry list (no disk I/O).
    pub fn from_entries(entries: &[FlatFileEntry]) -> Self {
        let mut stats = Self::default();
        for e in entries {
            if e.is_dir {
                stats.dir_count += 1;
            } else {
                stats.file_count += 1;
            }
            if e.depth > stats.deepest_level {
                stats.deepest_level = e.depth;
            }
        }
        stats
    }
}

impl fmt::Display for DirectoryStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} files, {} dirs, {} bytes, depth {}",
            self.file_count, self.dir_count, self.total_size, self.deepest_level,
        )
    }
}

// ---------------------------------------------------------------------------
// BreadcrumbPath — clickable path segments
// ---------------------------------------------------------------------------

/// A single segment in a breadcrumb path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    /// Display label for this segment.
    pub label: String,
    /// Full path up to and including this segment.
    pub full_path: PathBuf,
}

/// Splits a filesystem path into clickable breadcrumb segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbPath {
    /// Ordered segments from root to leaf.
    pub segments: Vec<BreadcrumbSegment>,
}

impl BreadcrumbPath {
    /// Create breadcrumbs from a path, optionally relative to a root.
    ///
    /// If `root` is provided, only the portion of `path` below `root` is used
    /// and the first segment is labelled with the root directory name.
    pub fn from_path(path: &Path, root: Option<&Path>) -> Self {
        let effective = match root {
            Some(r) => path.strip_prefix(r).unwrap_or(path),
            None => path,
        };

        let mut segments = Vec::new();

        // If we have a root, add it as the first segment.
        if let Some(r) = root {
            let label = r
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| r.to_string_lossy().to_string());
            segments.push(BreadcrumbSegment {
                label,
                full_path: r.to_path_buf(),
            });
        }

        let mut accumulated = root.map(|r| r.to_path_buf()).unwrap_or_default();
        for component in effective.components() {
            let label = component.as_os_str().to_string_lossy().to_string();
            accumulated = accumulated.join(&label);
            segments.push(BreadcrumbSegment {
                label,
                full_path: accumulated.clone(),
            });
        }

        Self { segments }
    }

    /// Number of breadcrumb segments.
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Whether the breadcrumb path is empty.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get the segment at a given index.
    pub fn get(&self, index: usize) -> Option<&BreadcrumbSegment> {
        self.segments.get(index)
    }

    /// The last (leaf) segment, if any.
    pub fn leaf(&self) -> Option<&BreadcrumbSegment> {
        self.segments.last()
    }
}

impl fmt::Display for BreadcrumbPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let labels: Vec<&str> = self.segments.iter().map(|s| s.label.as_str()).collect();
        write!(f, "{}", labels.join(" › "))
    }
}

impl From<&Path> for BreadcrumbPath {
    fn from(path: &Path) -> Self {
        Self::from_path(path, None)
    }
}

impl From<PathBuf> for BreadcrumbPath {
    fn from(path: PathBuf) -> Self {
        Self::from_path(&path, None)
    }
}

// ---------------------------------------------------------------------------
// ExplorerDragDrop – file move/copy via drag-and-drop
// ---------------------------------------------------------------------------

/// Represents a drag-and-drop operation in the explorer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DragDropEffect {
    Move,
    Copy,
}

impl fmt::Display for DragDropEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Move => write!(f, "Move"),
            Self::Copy => write!(f, "Copy"),
        }
    }
}

/// Manages drag-and-drop state for the file explorer.
#[derive(Debug, Clone)]
pub struct ExplorerDragDrop {
    /// Source path being dragged.
    pub source: Option<PathBuf>,
    /// Current drop target directory.
    pub target: Option<PathBuf>,
    /// The effect (move or copy).
    pub effect: DragDropEffect,
    /// Whether a drag is in progress.
    pub active: bool,
}

impl ExplorerDragDrop {
    pub fn new() -> Self {
        Self {
            source: None,
            target: None,
            effect: DragDropEffect::Move,
            active: false,
        }
    }

    /// Start a drag from a source path.
    pub fn start_drag(&mut self, source: PathBuf) {
        self.source = Some(source);
        self.active = true;
    }

    /// Update the drop target.
    pub fn set_target(&mut self, target: PathBuf) {
        self.target = Some(target);
    }

    /// Set to copy mode (e.g., when Ctrl is held).
    pub fn set_copy(&mut self) {
        self.effect = DragDropEffect::Copy;
    }

    /// Cancel the drag operation.
    pub fn cancel(&mut self) {
        self.source = None;
        self.target = None;
        self.active = false;
        self.effect = DragDropEffect::Move;
    }

    /// Check if a drop is valid (source and target are set, target is a directory,
    /// target is not the source or a child of source).
    pub fn can_drop(&self) -> bool {
        match (&self.source, &self.target) {
            (Some(src), Some(tgt)) => {
                src != tgt && !tgt.starts_with(src)
            }
            _ => false,
        }
    }

    /// Compute the destination path after drop.
    pub fn destination_path(&self) -> Option<PathBuf> {
        let src = self.source.as_ref()?;
        let tgt = self.target.as_ref()?;
        let name = src.file_name()?;
        Some(tgt.join(name))
    }
}

impl Default for ExplorerDragDrop {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExplorerDragDrop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.active {
            write!(f, "DragDrop({}, active)", self.effect)
        } else {
            write!(f, "DragDrop(idle)")
        }
    }
}

// ---------------------------------------------------------------------------
// ExplorerInlineRename – in-place file rename handler
// ---------------------------------------------------------------------------

/// State for an in-place rename operation in the explorer.
#[derive(Debug, Clone)]
pub struct ExplorerInlineRename {
    pub original_path: PathBuf,
    pub new_name: String,
    pub cursor_pos: usize,
    pub is_active: bool,
}

impl ExplorerInlineRename {
    /// Start a rename operation.
    pub fn start(path: PathBuf) -> Self {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let cursor_pos = name.len();
        Self {
            original_path: path,
            new_name: name,
            cursor_pos,
            is_active: true,
        }
    }

    /// Set the new name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.new_name = name.into();
        self.cursor_pos = self.new_name.len();
    }

    /// Validate the new name (non-empty, no path separators).
    pub fn validate(&self) -> Result<(), String> {
        if self.new_name.trim().is_empty() {
            return Err("Name cannot be empty".to_string());
        }
        if self.new_name.contains('/') || self.new_name.contains('\\') {
            return Err("Name cannot contain path separators".to_string());
        }
        Ok(())
    }

    /// Compute the new full path.
    pub fn new_path(&self) -> PathBuf {
        if let Some(parent) = self.original_path.parent() {
            parent.join(&self.new_name)
        } else {
            PathBuf::from(&self.new_name)
        }
    }

    /// Whether the name actually changed.
    pub fn has_changed(&self) -> bool {
        let original_name = self.original_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.new_name != original_name
    }

    /// Cancel the rename.
    pub fn cancel(&mut self) {
        self.is_active = false;
    }
}

impl fmt::Display for ExplorerInlineRename {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rename({} -> {})", self.original_path.display(), self.new_name)
    }
}

// ---------------------------------------------------------------------------
// ExplorerDecorationsAggregator – combines decorations from multiple sources
// ---------------------------------------------------------------------------

/// A decoration applied to a file in the explorer (e.g., git status, errors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDecoration {
    pub badge: Option<String>,
    pub color: Option<String>,
    pub tooltip: Option<String>,
    pub priority: i32,
}

impl FileDecoration {
    pub fn new() -> Self {
        Self { badge: None, color: None, tooltip: None, priority: 0 }
    }

    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

impl Default for FileDecoration {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregates decorations from multiple sources for a single file.
#[derive(Debug, Clone)]
pub struct ExplorerDecorationsAggregator {
    decorations: Vec<(PathBuf, FileDecoration)>,
}

impl ExplorerDecorationsAggregator {
    pub fn new() -> Self {
        Self { decorations: Vec::new() }
    }

    pub fn add_decoration(&mut self, path: PathBuf, decoration: FileDecoration) {
        self.decorations.push((path, decoration));
    }

    /// Get all decorations for a given path, sorted by priority (highest first).
    pub fn get_decorations(&self, path: &Path) -> Vec<&FileDecoration> {
        let mut result: Vec<_> = self.decorations.iter()
            .filter(|(p, _)| p == path)
            .map(|(_, d)| d)
            .collect();
        result.sort_by(|a, b| b.priority.cmp(&a.priority));
        result
    }

    /// Get the primary (highest priority) decoration for a path.
    pub fn primary_decoration(&self, path: &Path) -> Option<&FileDecoration> {
        self.get_decorations(path).into_iter().next()
    }

    /// Total number of decorations.
    pub fn total_count(&self) -> usize {
        self.decorations.len()
    }

    /// Clear all decorations.
    pub fn clear(&mut self) {
        self.decorations.clear();
    }
}

impl Default for ExplorerDecorationsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExplorerDecorationsAggregator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DecorationsAggregator({} decorations)", self.decorations.len())
    }
}

// ---------------------------------------------------------------------------
// Explorer file nesting rules
// ---------------------------------------------------------------------------

/// A rule that determines how files are nested in the explorer tree.
#[derive(Debug, Clone)]
pub struct FileNestingRule {
    /// Parent file extension pattern (e.g., ".ts").
    pub parent_ext: String,
    /// Nested file suffixes (e.g., [".d.ts", ".js", ".js.map"]).
    pub nested_suffixes: Vec<String>,
}

impl FileNestingRule {
    pub fn new(parent_ext: impl Into<String>, nested: Vec<String>) -> Self {
        Self {
            parent_ext: parent_ext.into(),
            nested_suffixes: nested,
        }
    }

    /// Check if a filename should be nested under a parent.
    pub fn should_nest(&self, parent_name: &str, child_name: &str) -> bool {
        if !parent_name.ends_with(&self.parent_ext) {
            return false;
        }
        let stem = &parent_name[..parent_name.len() - self.parent_ext.len()];
        self.nested_suffixes.iter().any(|suffix| {
            child_name == format!("{stem}{suffix}")
        })
    }
}

impl fmt::Display for FileNestingRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NestingRule({} -> {} suffixes)", self.parent_ext, self.nested_suffixes.len())
    }
}

/// Default file nesting rules (TypeScript/JavaScript ecosystem).
pub fn default_nesting_rules() -> Vec<FileNestingRule> {
    vec![
        FileNestingRule::new(".ts", vec![".js".into(), ".d.ts".into(), ".js.map".into()]),
        FileNestingRule::new(".rs", vec![]),
        FileNestingRule::new(".toml", vec![".lock".into()]),
    ]
}

/// Find which children should be nested under a parent based on rules.
pub fn find_nested_files<'a>(parent_name: &str, children: &'a [&str], rules: &[FileNestingRule]) -> Vec<&'a str> {
    children.iter()
        .filter(|child| rules.iter().any(|r| r.should_nest(parent_name, child)))
        .copied()
        .collect()
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

    // -----------------------------------------------------------------------
    // Tests for FileSearchResult and search_files
    // -----------------------------------------------------------------------

    #[test]
    fn search_files_finds_matching_entries() {
        let entries = vec![
            FlatFileEntry {
                name: "main.rs".into(),
                path: PathBuf::from("/project/src/main.rs"),
                is_dir: false,
                is_expanded: false,
                depth: 2,
            },
            FlatFileEntry {
                name: "lib.rs".into(),
                path: PathBuf::from("/project/src/lib.rs"),
                is_dir: false,
                is_expanded: false,
                depth: 2,
            },
            FlatFileEntry {
                name: "src".into(),
                path: PathBuf::from("/project/src"),
                is_dir: true,
                is_expanded: true,
                depth: 1,
            },
            FlatFileEntry {
                name: "manifest.json".into(),
                path: PathBuf::from("/project/manifest.json"),
                is_dir: false,
                is_expanded: false,
                depth: 1,
            },
        ];
        let results = search_files(&entries, "main");
        // Should match "main.rs" and "manifest.json" (m-a-i-n subsequence), skip dir
        assert!(results.len() >= 1);
        assert_eq!(results[0].file_name(), "main.rs");
        assert!(!results[0].match_positions.is_empty());
        // Display impl
        let display = format!("{}", results[0]);
        assert!(display.contains("score:"));
    }

    #[test]
    fn search_files_empty_query_returns_empty() {
        let entries = vec![FlatFileEntry {
            name: "main.rs".into(),
            path: PathBuf::from("/project/src/main.rs"),
            is_dir: false,
            is_expanded: false,
            depth: 0,
        }];
        assert!(search_files(&entries, "").is_empty());
    }

    // -----------------------------------------------------------------------
    // Tests for FileFilter
    // -----------------------------------------------------------------------

    #[test]
    fn file_filter_include_extensions() {
        let mut filter = FileFilter::new();
        filter.include_extensions = vec!["rs".into(), "toml".into()];
        assert!(filter.matches("main.rs", false));
        assert!(filter.matches("Cargo.toml", false));
        assert!(!filter.matches("readme.md", false));
        // Directories always pass
        assert!(filter.matches("src", true));
        // Display
        let display = format!("{filter}");
        assert!(display.contains("+[rs,toml]"));
    }

    #[test]
    fn file_filter_exclude_and_hidden() {
        let mut filter = FileFilter::new();
        filter.exclude_extensions = vec!["log".into()];
        filter.exclude_names = vec!["Thumbs.db".into()];
        filter.hide_hidden = true;
        assert!(!filter.matches("debug.log", false));
        assert!(!filter.matches("Thumbs.db", false));
        assert!(!filter.matches(".gitignore", false));
        assert!(filter.matches("main.rs", false));
        // Display
        let display = format!("{filter}");
        assert!(display.contains("no-hidden"));
    }

    #[test]
    fn file_filter_default_accepts_all() {
        let filter = FileFilter::new();
        assert!(filter.matches("anything.xyz", false));
        assert!(filter.matches(".hidden", false));
        assert_eq!(format!("{filter}"), "FileFilter(all)");
    }

    // -----------------------------------------------------------------------
    // Tests for DirectoryStats
    // -----------------------------------------------------------------------

    #[test]
    fn directory_stats_from_path() {
        let dir = make_test_dir();
        let stats = DirectoryStats::from_path(dir.path()).unwrap();
        // make_test_dir creates: src/main.rs, src/lib.rs, tests/test1.rs, Cargo.toml, README.md
        assert_eq!(stats.file_count, 5);
        // Dirs: src, tests
        assert_eq!(stats.dir_count, 2);
        assert!(stats.deepest_level >= 1);
        // Display impl
        let display = format!("{stats}");
        assert!(display.contains("5 files"));
        assert!(display.contains("2 dirs"));
    }

    #[test]
    fn directory_stats_from_entries() {
        let entries = vec![
            FlatFileEntry { name: "root".into(), path: PathBuf::from("/r"), is_dir: true, is_expanded: true, depth: 0 },
            FlatFileEntry { name: "src".into(), path: PathBuf::from("/r/src"), is_dir: true, is_expanded: true, depth: 1 },
            FlatFileEntry { name: "main.rs".into(), path: PathBuf::from("/r/src/main.rs"), is_dir: false, is_expanded: false, depth: 2 },
            FlatFileEntry { name: "README.md".into(), path: PathBuf::from("/r/README.md"), is_dir: false, is_expanded: false, depth: 1 },
        ];
        let stats = DirectoryStats::from_entries(&entries);
        assert_eq!(stats.file_count, 2);
        assert_eq!(stats.dir_count, 2);
        assert_eq!(stats.deepest_level, 2);
    }

    // -----------------------------------------------------------------------
    // Tests for BreadcrumbPath
    // -----------------------------------------------------------------------

    #[test]
    fn breadcrumb_from_path_no_root() {
        let path = PathBuf::from("src/utils/helpers.rs");
        let bc = BreadcrumbPath::from(path);
        assert_eq!(bc.len(), 3);
        assert_eq!(bc.segments[0].label, "src");
        assert_eq!(bc.segments[1].label, "utils");
        assert_eq!(bc.segments[2].label, "helpers.rs");
        assert_eq!(bc.leaf().unwrap().label, "helpers.rs");
        // Display: joined by " › "
        let display = format!("{bc}");
        assert_eq!(display, "src › utils › helpers.rs");
    }

    #[test]
    fn breadcrumb_from_path_with_root() {
        let root = Path::new("/home/user/project");
        let path = Path::new("/home/user/project/src/main.rs");
        let bc = BreadcrumbPath::from_path(path, Some(root));
        // First segment is the root label, then "src", then "main.rs"
        assert_eq!(bc.len(), 3);
        assert_eq!(bc.segments[0].label, "project");
        assert_eq!(bc.segments[1].label, "src");
        assert_eq!(bc.segments[2].label, "main.rs");
        assert!(!bc.is_empty());
        assert_eq!(bc.get(1).unwrap().label, "src");
    }

    #[test]
    fn breadcrumb_empty_path() {
        let bc = BreadcrumbPath::from_path(Path::new(""), None);
        assert!(bc.is_empty());
        assert!(bc.leaf().is_none());
        assert_eq!(format!("{bc}"), "");
    }

    // -- ExplorerDragDrop --------------------------------------------------

    #[test]
    fn drag_drop_start_and_cancel() {
        let mut dd = ExplorerDragDrop::new();
        dd.start_drag(PathBuf::from("/src/main.rs"));
        assert!(dd.active);
        dd.cancel();
        assert!(!dd.active);
        assert!(dd.source.is_none());
    }

    #[test]
    fn drag_drop_can_drop() {
        let mut dd = ExplorerDragDrop::new();
        dd.start_drag(PathBuf::from("/src/main.rs"));
        dd.set_target(PathBuf::from("/dest"));
        assert!(dd.can_drop());
    }

    #[test]
    fn drag_drop_cannot_drop_on_self() {
        let mut dd = ExplorerDragDrop::new();
        dd.start_drag(PathBuf::from("/src"));
        dd.set_target(PathBuf::from("/src/child"));
        assert!(!dd.can_drop());
    }

    #[test]
    fn drag_drop_destination_path() {
        let mut dd = ExplorerDragDrop::new();
        dd.start_drag(PathBuf::from("/src/main.rs"));
        dd.set_target(PathBuf::from("/dest"));
        assert_eq!(dd.destination_path(), Some(PathBuf::from("/dest/main.rs")));
    }

    #[test]
    fn drag_drop_display() {
        let dd = ExplorerDragDrop::default();
        assert!(format!("{dd}").contains("idle"));
    }

    // -- ExplorerInlineRename ----------------------------------------------

    #[test]
    fn inline_rename_start() {
        let rename = ExplorerInlineRename::start(PathBuf::from("/src/main.rs"));
        assert_eq!(rename.new_name, "main.rs");
        assert!(rename.is_active);
    }

    #[test]
    fn inline_rename_validate() {
        let mut rename = ExplorerInlineRename::start(PathBuf::from("/src/main.rs"));
        assert!(rename.validate().is_ok());
        rename.set_name("");
        assert!(rename.validate().is_err());
        rename.set_name("bad/name");
        assert!(rename.validate().is_err());
    }

    #[test]
    fn inline_rename_new_path() {
        let mut rename = ExplorerInlineRename::start(PathBuf::from("/src/main.rs"));
        rename.set_name("lib.rs");
        assert_eq!(rename.new_path(), PathBuf::from("/src/lib.rs"));
    }

    #[test]
    fn inline_rename_has_changed() {
        let rename = ExplorerInlineRename::start(PathBuf::from("/src/main.rs"));
        assert!(!rename.has_changed());
        let mut rename2 = ExplorerInlineRename::start(PathBuf::from("/src/main.rs"));
        rename2.set_name("other.rs");
        assert!(rename2.has_changed());
    }

    #[test]
    fn inline_rename_display() {
        let rename = ExplorerInlineRename::start(PathBuf::from("/src/main.rs"));
        let s = format!("{rename}");
        assert!(s.contains("main.rs"));
    }

    // -- ExplorerDecorationsAggregator ------------------------------------

    #[test]
    fn decorations_add_and_get() {
        let mut agg = ExplorerDecorationsAggregator::new();
        agg.add_decoration(
            PathBuf::from("/src/main.rs"),
            FileDecoration::new().with_badge("M").with_priority(10),
        );
        agg.add_decoration(
            PathBuf::from("/src/main.rs"),
            FileDecoration::new().with_badge("E").with_priority(20),
        );
        let decorations = agg.get_decorations(Path::new("/src/main.rs"));
        assert_eq!(decorations.len(), 2);
        assert_eq!(decorations[0].badge.as_deref(), Some("E")); // higher priority first
    }

    #[test]
    fn decorations_primary() {
        let mut agg = ExplorerDecorationsAggregator::new();
        agg.add_decoration(
            PathBuf::from("/file"),
            FileDecoration::new().with_badge("X").with_priority(5),
        );
        let primary = agg.primary_decoration(Path::new("/file")).unwrap();
        assert_eq!(primary.badge.as_deref(), Some("X"));
    }

    #[test]
    fn decorations_display() {
        let agg = ExplorerDecorationsAggregator::default();
        assert!(format!("{agg}").contains("0 decorations"));
    }

    // -- File nesting rules ------------------------------------------------

    #[test]
    fn nesting_rule_should_nest() {
        let rule = FileNestingRule::new(".ts", vec![".js".into(), ".d.ts".into()]);
        assert!(rule.should_nest("app.ts", "app.js"));
        assert!(rule.should_nest("app.ts", "app.d.ts"));
        assert!(!rule.should_nest("app.ts", "other.js"));
        assert!(!rule.should_nest("app.rs", "app.js"));
    }

    #[test]
    fn nesting_rule_display() {
        let rule = FileNestingRule::new(".ts", vec![".js".into()]);
        assert!(format!("{rule}").contains(".ts"));
    }

    #[test]
    fn find_nested_files_basic() {
        let rules = vec![FileNestingRule::new(".ts", vec![".js".into(), ".js.map".into()])];
        let children = vec!["app.js", "app.js.map", "styles.css"];
        let nested = find_nested_files("app.ts", &children, &rules);
        assert_eq!(nested, vec!["app.js", "app.js.map"]);
    }

    #[test]
    fn default_nesting_rules_exist() {
        let rules = default_nesting_rules();
        assert!(!rules.is_empty());
    }
}
