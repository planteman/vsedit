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

// ---------------------------------------------------------------------------
// ExplorerSortSwitcher - explorer sort mode switcher
// ---------------------------------------------------------------------------

/// Severity level for explorer sort mode switcher issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExplorerSortSwitcherSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ExplorerSortSwitcherSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ExplorerSortSwitcher].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerSortSwitcherEntry {
    pub id: String,
    pub label: String,
    pub severity: ExplorerSortSwitcherSeverity,
    pub detail: Option<String>,
    pub file_count: usize,
    enabled: bool,
}

impl ExplorerSortSwitcherEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ExplorerSortSwitcherSeverity::Low,
            detail: None,
            file_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ExplorerSortSwitcherSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_file_count(mut self, val: usize) -> Self {
        self.file_count = val;
        self
    }

    pub fn is_sorted(&self) -> bool {
        self.enabled && self.severity >= ExplorerSortSwitcherSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.file_count, det)
    }
}

impl fmt::Display for ExplorerSortSwitcherEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ExplorerSortSwitcherEntry] items.
#[derive(Debug, Clone)]
pub struct ExplorerSortSwitcher {
    entries: Vec<ExplorerSortSwitcherEntry>,
    name: String,
    capacity: usize,
}

impl ExplorerSortSwitcher {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ExplorerSortSwitcherEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ExplorerSortSwitcherEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ExplorerSortSwitcherEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn file_count(&self) -> usize { self.entries.len() }

    pub fn is_sorted(&self) -> bool {
        self.entries.iter().any(|e| e.is_sorted())
    }

    pub fn entries_by_severity(&self, severity: ExplorerSortSwitcherSeverity) -> Vec<&ExplorerSortSwitcherEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ExplorerSortSwitcherSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ExplorerSortSwitcherEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&ExplorerSortSwitcherEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// ExplorerFilterByType - explorer filter by type
// ---------------------------------------------------------------------------

/// Configuration for [ExplorerFilterByType].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerFilterByTypeConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub filter_count: usize,
}

impl ExplorerFilterByTypeConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, filter_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_filter_count(mut self, val: usize) -> Self { self.filter_count = val; self }
}

impl Default for ExplorerFilterByTypeConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ExplorerFilterByType].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerFilterByTypeItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ExplorerFilterByTypeItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_filter(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ExplorerFilterByTypeItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ExplorerFilterByTypeItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ExplorerFilterByType {
    config: ExplorerFilterByTypeConfig,
    items: Vec<ExplorerFilterByTypeItem>,
}

impl ExplorerFilterByType {
    pub fn new(config: ExplorerFilterByTypeConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ExplorerFilterByTypeItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ExplorerFilterByTypeItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ExplorerFilterByTypeItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn filter_count(&self) -> usize { self.items.len() }

    pub fn has_filter(&self) -> bool {
        self.items.iter().any(|i| i.has_filter())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ExplorerFilterByTypeItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ExplorerFilterByTypeItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ExplorerFilterByTypeConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



/// Tracks file tree expansion state for the explorer.
pub struct ExpansionState {
    expanded: std::collections::HashSet<String>,
}

impl ExpansionState {
    pub fn new() -> Self {
        Self { expanded: std::collections::HashSet::new() }
    }

    pub fn expand(&mut self, path: &str) {
        self.expanded.insert(path.to_string());
    }

    pub fn collapse(&mut self, path: &str) {
        self.expanded.remove(path);
    }

    pub fn toggle(&mut self, path: &str) {
        if self.is_expanded(path) { self.collapse(path); } else { self.expand(path); }
    }

    pub fn is_expanded(&self, path: &str) -> bool {
        self.expanded.contains(path)
    }

    pub fn expanded_count(&self) -> usize { self.expanded.len() }

    pub fn collapse_all(&mut self) { self.expanded.clear(); }

    pub fn expand_all(&mut self, paths: &[&str]) {
        for p in paths { self.expanded.insert(p.to_string()); }
    }

    pub fn expanded_paths(&self) -> Vec<String> {
        let mut v: Vec<_> = self.expanded.iter().cloned().collect();
        v.sort();
        v
    }
}

/// Filters explorer entries based on glob-like patterns.
pub struct ExplorerFilter {
    patterns: Vec<String>,
    exclude: bool,
}

impl ExplorerFilter {
    pub fn new_include(patterns: Vec<String>) -> Self {
        Self { patterns, exclude: false }
    }

    pub fn new_exclude(patterns: Vec<String>) -> Self {
        Self { patterns, exclude: true }
    }

    pub fn matches(&self, name: &str) -> bool {
        let matched = self.patterns.iter().any(|p| {
            if p.starts_with("*.") {
                name.ends_with(&p[1..])
            } else {
                name == p
            }
        });
        if self.exclude { !matched } else { matched }
    }

    pub fn pattern_count(&self) -> usize { self.patterns.len() }
    pub fn is_exclude(&self) -> bool { self.exclude }
}

/// Computes file tree statistics for the explorer sidebar.
pub struct ExplorerStats {
    pub file_count: usize,
    pub dir_count: usize,
    pub total_size: u64,
    pub max_depth: usize,
}

impl ExplorerStats {
    pub fn new() -> Self {
        Self { file_count: 0, dir_count: 0, total_size: 0, max_depth: 0 }
    }

    pub fn add_file(&mut self, size: u64, depth: usize) {
        self.file_count += 1;
        self.total_size += size;
        if depth > self.max_depth { self.max_depth = depth; }
    }

    pub fn add_dir(&mut self, depth: usize) {
        self.dir_count += 1;
        if depth > self.max_depth { self.max_depth = depth; }
    }

    pub fn total_items(&self) -> usize { self.file_count + self.dir_count }

    pub fn average_file_size(&self) -> f64 {
        if self.file_count == 0 { 0.0 } else { self.total_size as f64 / self.file_count as f64 }
    }
}



// ---------------------------------------------------------------------------
// explorer – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for file explorer view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExplorerExplorerSortBy {
    Name,
    Modified,
    Type,
    Size,
}

impl YExplorerExplorerSortBy {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Name => 0,
            Self::Modified => 1,
            Self::Type => 2,
            Self::Size => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Modified => "Modified",
            Self::Type => "Type",
            Self::Size => "Size",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExplorerExplorerSortBy] {
        &[
            YExplorerExplorerSortBy::Name,
            YExplorerExplorerSortBy::Modified,
            YExplorerExplorerSortBy::Type,
            YExplorerExplorerSortBy::Size,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExplorerExplorerSortBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks explorer node data.
#[derive(Debug, Clone)]
pub struct YExplorerExplorerNode {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

impl YExplorerExplorerNode {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            name: String::new(),
            is_dir: false,
            size: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExplorerExplorerNode({}: {:?})", "name", self.name)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_explorer_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_explorer_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_explorer_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_explorer_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_explorer_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_explorer_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_explorer_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_explorer_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// explorer – Extended explorer filter helpers
// ---------------------------------------------------------------------------

/// Priority levels for explorer filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExplorerPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExplorerPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZExplorerPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExplorerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks explorer filter data.
#[derive(Debug, Clone)]
pub struct ZExplorerExplorerFilter {
    pub glob_patterns: Vec<String>,
    pub show_hidden: bool,
    pub max_depth: usize,
}

impl ZExplorerExplorerFilter {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            glob_patterns: Vec::new(),
            show_hidden: false,
            max_depth: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.glob_patterns.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.glob_patterns.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.glob_patterns.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExplorerExplorerFilter[show_hidden={:?}, max_depth={:?}]", self.show_hidden, self.max_depth)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for explorer filter.
pub fn z_explorer_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_explorer_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_explorer_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_explorer_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_explorer_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_explorer_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_explorer_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 47
// ---------------------------------------------------------------------------

/// Generic object pool `Xc47Pool<T>`.
pub struct Xc47Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc47Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc47PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc47Pool<T> {
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
    pub fn stats(&self) -> Xc47PoolStats {
        Xc47PoolStats {
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

impl<T> Default for Xc47Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc47Scheduler`.
pub struct Xc47Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc47Scheduler {
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

impl Default for Xc47Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_47 hash for the given byte slice.
pub fn xc_47_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_47 convention.
pub fn xc_47_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_6 deepening: state machine + event bus ---

/// States for the Xd6 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd6State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd6State {
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
pub struct Xd6Transition {
    pub from: Xd6State,
    pub to: Xd6State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd6StateMachine {
    current: Xd6State,
    history: Vec<Xd6Transition>,
    step_counter: usize,
}

impl Xd6StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd6State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd6State {
        self.current
    }

    pub fn history(&self) -> &[Xd6Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd6State) -> Result<Xd6State, String> {
        let allowed = match (self.current, target) {
            (Xd6State::Idle, Xd6State::Running) => true,
            (Xd6State::Running, Xd6State::Paused) => true,
            (Xd6State::Running, Xd6State::Done) => true,
            (Xd6State::Paused, Xd6State::Running) => true,
            (Xd6State::Paused, Xd6State::Done) => true,
            (Xd6State::Done, Xd6State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_6: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd6Transition {
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
            "Xd6SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd6State> {
        let prefix = "Xd6SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd6State::Idle),
            "Running" => Some(Xd6State::Running),
            "Paused" => Some(Xd6State::Paused),
            "Done" => Some(Xd6State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd6State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd6 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd6Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd6Event {
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

type Xd6HandlerFn = Box<dyn Fn(&Xd6Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd6EventBus {
    handlers: Vec<(usize, Option<String>, Xd6HandlerFn)>,
    next_id: usize,
    published: Vec<Xd6Event>,
}

impl Xd6EventBus {
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
        F: Fn(&Xd6Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd6Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd6Event) {
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

    pub fn published_events(&self) -> &[Xd6Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #4
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf4Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf4TrieNode {
    children: std::collections::HashMap<char, Xf4TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf4Trie {
    root: Xf4TrieNode,
    count: usize,
}

impl Xf4Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf4TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf4TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf4TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf4BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf4BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 46).
pub struct Xh46SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh46SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 88 as u64,
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

/// A compact bit set supporting boolean operations (variant 46).
pub struct Xh46BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh46BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 46).
pub struct Xi46Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi46Deque<T> {
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
pub struct Xi46Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi46Interval {
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

/// A simple interval tree (variant 46).
pub struct Xi46IntervalTree {
    xi_intervals: Vec<Xi46Interval>,
}

impl Xi46IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi46Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi46Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi46Interval) -> Vec<&Xi46Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi46Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi46Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi46Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi46Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi46Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi46Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 46) ---

/// Disjoint set / union-find for crate 46.
pub struct Xj46UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj46UnionFind {
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

const XJ46_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 46.
pub struct Xj46BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj46BTreeNode<K, V>>>,
    len: usize,
}

struct Xj46BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj46BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj46BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ46_BTREE_ORDER - 1
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
        let mid = XJ46_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj46BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj46BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj46BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj46BTreeNode::xj_new_leaf();
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


// --- xk_46 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk46SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk46SegmentTree {
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
pub struct Xk46DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk46DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_46).
#[derive(Debug, Clone)]
pub struct Xl46Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl46Rope {
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

/// Suffix array for efficient string searching (xl_46).
#[derive(Debug, Clone)]
pub struct Xl46SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl46SuffixArray {
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


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm46MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm46MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm46Tokenizer {
    text: String,
}

impl Xm46Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 46.
pub struct Xn46Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn46Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 46 -----

#[derive(Debug, Clone)]
struct Xn46AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn46AvlNode<K, V>>>,
    right: Option<Box<Xn46AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 46.
#[derive(Debug, Clone)]
pub struct Xn46AVL<K, V> {
    root: Option<Box<Xn46AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn46AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn46AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn46AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn46AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn46AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn46AvlNode<K, V>>) -> Box<Xn46AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn46AvlNode<K, V>>) -> Box<Xn46AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn46AvlNode<K, V>>) -> Box<Xn46AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn46AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn46AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn46AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn46AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn46AvlNode<K, V>>) -> &Xn46AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn46AvlNode<K, V>>) -> (Box<Xn46AvlNode<K, V>>, Option<Box<Xn46AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn46AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn46AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn46AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn46AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn46AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn46AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn46AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}

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

#[test]
    fn explorersortswitcher_severity_ordering() {
        assert!(ExplorerSortSwitcherSeverity::Critical > ExplorerSortSwitcherSeverity::High);
        assert!(ExplorerSortSwitcherSeverity::High > ExplorerSortSwitcherSeverity::Medium);
        assert!(ExplorerSortSwitcherSeverity::Medium > ExplorerSortSwitcherSeverity::Low);
    }

    #[test]
    fn explorersortswitcher_severity_display() {
        assert_eq!(ExplorerSortSwitcherSeverity::Low.to_string(), "low");
        assert_eq!(ExplorerSortSwitcherSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn explorersortswitcher_entry_creation() {
        let e = ExplorerSortSwitcherEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ExplorerSortSwitcherSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn explorersortswitcher_entry_builder() {
        let e = ExplorerSortSwitcherEntry::new("e2", "Entry 2")
            .with_severity(ExplorerSortSwitcherSeverity::High)
            .with_detail("some detail")
            .with_file_count(42);
        assert_eq!(e.severity, ExplorerSortSwitcherSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.file_count, 42);
    }

    #[test]
    fn explorersortswitcher_entry_enable_disable() {
        let mut e = ExplorerSortSwitcherEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn explorersortswitcher_add_and_count() {
        let mut mgr = ExplorerSortSwitcher::new("test");
        mgr.add(ExplorerSortSwitcherEntry::new("a", "A"));
        mgr.add(ExplorerSortSwitcherEntry::new("b", "B").with_severity(ExplorerSortSwitcherSeverity::High));
        assert_eq!(mgr.file_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn explorersortswitcher_remove() {
        let mut mgr = ExplorerSortSwitcher::new("test");
        mgr.add(ExplorerSortSwitcherEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn explorersortswitcher_capacity() {
        let mut mgr = ExplorerSortSwitcher::new("test").with_capacity(1);
        assert!(mgr.add(ExplorerSortSwitcherEntry::new("a", "A")));
        assert!(!mgr.add(ExplorerSortSwitcherEntry::new("b", "B")));
    }

    #[test]
    fn explorersortswitcher_sorted_by_severity() {
        let mut mgr = ExplorerSortSwitcher::new("test");
        mgr.add(ExplorerSortSwitcherEntry::new("lo", "Low"));
        mgr.add(ExplorerSortSwitcherEntry::new("hi", "High").with_severity(ExplorerSortSwitcherSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ExplorerSortSwitcherSeverity::Critical);
    }

    #[test]
    fn explorersortswitcher_summary() {
        let mgr = ExplorerSortSwitcher::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn explorerfilterbytype_config_defaults() {
        let cfg = ExplorerFilterByTypeConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn explorerfilterbytype_item_creation() {
        let item = ExplorerFilterByTypeItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn explorerfilterbytype_add_and_get() {
        let mut mgr = ExplorerFilterByType::new(ExplorerFilterByTypeConfig::new("test"));
        mgr.add(ExplorerFilterByTypeItem::new("k1", "v1"));
        assert_eq!(mgr.filter_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn explorerfilterbytype_remove_item() {
        let mut mgr = ExplorerFilterByType::new(ExplorerFilterByTypeConfig::new("test"));
        mgr.add(ExplorerFilterByTypeItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn explorerfilterbytype_sorted_by_priority() {
        let mut mgr = ExplorerFilterByType::new(ExplorerFilterByTypeConfig::new("test"));
        mgr.add(ExplorerFilterByTypeItem::new("lo", "low").with_priority(1));
        mgr.add(ExplorerFilterByTypeItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn explorerfilterbytype_items_with_tag() {
        let mut mgr = ExplorerFilterByType::new(ExplorerFilterByTypeConfig::new("test"));
        mgr.add(ExplorerFilterByTypeItem::new("a", "1").with_tag("x"));
        mgr.add(ExplorerFilterByTypeItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn explorerfilterbytype_report() {
        let mgr = ExplorerFilterByType::new(ExplorerFilterByTypeConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn expansion_state_expand_collapse() {
        let mut es = ExpansionState::new();
        es.expand("/src");
        assert!(es.is_expanded("/src"));
        es.collapse("/src");
        assert!(!es.is_expanded("/src"));
    }

    #[test]
    fn expansion_state_toggle() {
        let mut es = ExpansionState::new();
        es.toggle("/a");
        assert!(es.is_expanded("/a"));
        es.toggle("/a");
        assert!(!es.is_expanded("/a"));
    }

    #[test]
    fn expansion_state_collapse_all() {
        let mut es = ExpansionState::new();
        es.expand("/a");
        es.expand("/b");
        es.collapse_all();
        assert_eq!(es.expanded_count(), 0);
    }

    #[test]
    fn expansion_state_expand_all() {
        let mut es = ExpansionState::new();
        es.expand_all(&["/a", "/b", "/c"]);
        assert_eq!(es.expanded_count(), 3);
    }

    #[test]
    fn expansion_state_sorted_paths() {
        let mut es = ExpansionState::new();
        es.expand("/z");
        es.expand("/a");
        let paths = es.expanded_paths();
        assert_eq!(paths, vec!["/a", "/z"]);
    }

    #[test]
    fn explorer_filter_include() {
        let f = ExplorerFilter::new_include(vec!["*.rs".into(), "*.toml".into()]);
        assert!(f.matches("main.rs"));
        assert!(!f.matches("main.py"));
    }

    #[test]
    fn explorer_filter_exclude() {
        let f = ExplorerFilter::new_exclude(vec!["*.log".into()]);
        assert!(!f.matches("debug.log"));
        assert!(f.matches("main.rs"));
    }

    #[test]
    fn explorer_filter_exact_match() {
        let f = ExplorerFilter::new_include(vec!["Cargo.toml".into()]);
        assert!(f.matches("Cargo.toml"));
        assert!(!f.matches("cargo.toml"));
    }

    #[test]
    fn explorer_stats_add_files() {
        let mut s = ExplorerStats::new();
        s.add_file(100, 1);
        s.add_file(200, 2);
        assert_eq!(s.file_count, 2);
        assert_eq!(s.total_size, 300);
        assert_eq!(s.max_depth, 2);
    }

    #[test]
    fn explorer_stats_add_dirs() {
        let mut s = ExplorerStats::new();
        s.add_dir(1);
        s.add_dir(3);
        assert_eq!(s.dir_count, 2);
        assert_eq!(s.total_items(), 2);
    }

    #[test]
    fn explorer_stats_avg_file_size() {
        let mut s = ExplorerStats::new();
        s.add_file(100, 0);
        s.add_file(300, 0);
        assert_eq!(s.average_file_size(), 200.0);
    }

    #[test]
    fn explorer_stats_empty() {
        let s = ExplorerStats::new();
        assert_eq!(s.total_items(), 0);
        assert_eq!(s.average_file_size(), 0.0);
    }


    // -- explorer extended domain tests ----------------------------------------

    #[test]
    fn y_explorer_enum_index() {
        assert_eq!(YExplorerExplorerSortBy::Name.index(), 0);
        assert_eq!(YExplorerExplorerSortBy::Modified.index(), 1);
        assert_eq!(YExplorerExplorerSortBy::Type.index(), 2);
        assert_eq!(YExplorerExplorerSortBy::Size.index(), 3);
    }

    #[test]
    fn y_explorer_enum_label() {
        assert_eq!(YExplorerExplorerSortBy::Name.label(), "Name");
        assert_eq!(YExplorerExplorerSortBy::Modified.label(), "Modified");
        assert_eq!(YExplorerExplorerSortBy::Type.label(), "Type");
        assert_eq!(YExplorerExplorerSortBy::Size.label(), "Size");
    }

    #[test]
    fn y_explorer_enum_all() {
        let all = YExplorerExplorerSortBy::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_explorer_enum_is_default() {
        assert!(YExplorerExplorerSortBy::Name.is_default());
        assert!(!YExplorerExplorerSortBy::Size.is_default());
    }

    #[test]
    fn y_explorer_enum_display() {
        assert_eq!(format!("{}", YExplorerExplorerSortBy::Name), "Name");
    }

    #[test]
    fn y_explorer_struct_new() {
        let s = YExplorerExplorerNode::new();
        let _ = s.summary();
    }

    #[test]
    fn y_explorer_fingerprint_deterministic() {
        let h1 = y_explorer_fingerprint("hello");
        let h2 = y_explorer_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_explorer_fingerprint("a"), y_explorer_fingerprint("b"));
    }

    #[test]
    fn y_explorer_truncate_short() {
        assert_eq!(y_explorer_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_explorer_truncate_long() {
        let r = y_explorer_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_explorer_normalize_key_basic() {
        assert_eq!(y_explorer_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_explorer_split_path_basic() {
        let parts = y_explorer_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_explorer_count_occurrences_basic() {
        assert_eq!(y_explorer_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_explorer_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_explorer_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_explorer_in_range_basic() {
        assert!(y_explorer_in_range(5, 1, 10));
        assert!(y_explorer_in_range(1, 1, 10));
        assert!(y_explorer_in_range(10, 1, 10));
        assert!(!y_explorer_in_range(0, 1, 10));
        assert!(!y_explorer_in_range(11, 1, 10));
    }

    #[test]
    fn y_explorer_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_explorer_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_explorer_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_explorer_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- explorer Z-extended tests -----------------------------------------------

    #[test]
    fn z_explorer_priority_weight() {
        assert_eq!(ZExplorerPriority::Idle.weight(), 0);
        assert_eq!(ZExplorerPriority::Normal.weight(), 2);
        assert_eq!(ZExplorerPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_explorer_priority_label() {
        assert_eq!(ZExplorerPriority::Low.label(), "low");
        assert_eq!(ZExplorerPriority::High.label(), "high");
    }

    #[test]
    fn z_explorer_priority_is_elevated() {
        assert!(!ZExplorerPriority::Normal.is_elevated());
        assert!(ZExplorerPriority::High.is_elevated());
        assert!(ZExplorerPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_explorer_priority_display() {
        assert_eq!(format!("{}", ZExplorerPriority::Idle), "idle");
    }

    #[test]
    fn z_explorer_priority_all_asc() {
        let all = ZExplorerPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExplorerPriority::Idle);
        assert_eq!(all[4], ZExplorerPriority::Realtime);
    }

    #[test]
    fn z_explorer_struct_new() {
        let s = ZExplorerExplorerFilter::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_explorer_struct_toggled_clone() {
        let s = ZExplorerExplorerFilter::new();
        let t = s.toggled_clone();
        let _ = t.max_depth;
    }

    #[test]
    fn z_explorer_rolling_hash_deterministic() {
        let h1 = z_explorer_rolling_hash(b"test");
        let h2 = z_explorer_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_explorer_rolling_hash(b"a"), z_explorer_rolling_hash(b"b"));
    }

    #[test]
    fn z_explorer_pad_to_basic() {
        assert_eq!(z_explorer_pad_to("hi", 5), "hi   ");
        assert_eq!(z_explorer_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_explorer_is_identifier_basic() {
        assert!(z_explorer_is_identifier("foo_bar"));
        assert!(z_explorer_is_identifier("abc123"));
        assert!(!z_explorer_is_identifier(""));
        assert!(!z_explorer_is_identifier("has space"));
    }

    #[test]
    fn z_explorer_levenshtein_basic() {
        assert_eq!(z_explorer_levenshtein("", ""), 0);
        assert_eq!(z_explorer_levenshtein("abc", "abc"), 0);
        assert_eq!(z_explorer_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_explorer_unique_words_basic() {
        let w = z_explorer_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_explorer_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_explorer_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_explorer_common_prefix_basic() {
        assert_eq!(z_explorer_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_explorer_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_explorer_struct_clear() {
        let mut s = ZExplorerExplorerFilter::new();
        s.glob_patterns.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_explorer_rolling_hash_empty() {
        let h = z_explorer_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 47 ----

    #[test]
    fn xc_47_pool_new_empty() {
        let pool: super::Xc47Pool<i32> = super::Xc47Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_47_pool_release_acquire() {
        let mut pool = super::Xc47Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_47_pool_acquire_empty() {
        let mut pool: super::Xc47Pool<i32> = super::Xc47Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_47_pool_full() {
        let mut pool = super::Xc47Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_47_pool_drain() {
        let mut pool = super::Xc47Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_47_pool_stats() {
        let mut pool = super::Xc47Pool::new(8);
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
    fn xc_47_pool_clear() {
        let mut pool = super::Xc47Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_47_pool_shrink() {
        let mut pool = super::Xc47Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_47_pool_default() {
        let pool: super::Xc47Pool<String> = super::Xc47Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_47_pool_extend() {
        let mut pool = super::Xc47Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_47_pool_retain() {
        let mut pool = super::Xc47Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_47_scheduler_round_robin() {
        let mut sched = super::Xc47Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_47_scheduler_empty() {
        let mut sched = super::Xc47Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_47_scheduler_reset() {
        let mut sched = super::Xc47Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_47_scheduler_add_remove() {
        let mut sched = super::Xc47Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_47_scheduler_targets() {
        let sched = super::Xc47Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_47_hash_empty() {
        assert_eq!(super::xc_47_hash(b""), 5381);
    }

    #[test]
    fn xc_47_hash_data() {
        let h = super::xc_47_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_47_hash(b"hello"), h);
    }

    #[test]
    fn xc_47_reverse_str() {
        assert_eq!(super::xc_47_reverse("abc"), "cba");
        assert_eq!(super::xc_47_reverse(""), "");
    }


    // --- xd_6 deepening tests ---

    #[test]
    fn xd_6_sm_initial_state() {
        let sm = Xd6StateMachine::new();
        assert_eq!(sm.current_state(), Xd6State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_6_sm_valid_idle_to_running() {
        let mut sm = Xd6StateMachine::new();
        assert!(sm.transition(Xd6State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd6State::Running);
    }

    #[test]
    fn xd_6_sm_valid_running_to_paused() {
        let mut sm = Xd6StateMachine::new();
        sm.transition(Xd6State::Running).unwrap();
        assert!(sm.transition(Xd6State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd6State::Paused);
    }

    #[test]
    fn xd_6_sm_valid_running_to_done() {
        let mut sm = Xd6StateMachine::new();
        sm.transition(Xd6State::Running).unwrap();
        assert!(sm.transition(Xd6State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd6State::Done);
    }

    #[test]
    fn xd_6_sm_valid_paused_to_running() {
        let mut sm = Xd6StateMachine::new();
        sm.transition(Xd6State::Running).unwrap();
        sm.transition(Xd6State::Paused).unwrap();
        assert!(sm.transition(Xd6State::Running).is_ok());
    }

    #[test]
    fn xd_6_sm_valid_done_to_idle() {
        let mut sm = Xd6StateMachine::new();
        sm.transition(Xd6State::Running).unwrap();
        sm.transition(Xd6State::Done).unwrap();
        assert!(sm.transition(Xd6State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd6State::Idle);
    }

    #[test]
    fn xd_6_sm_invalid_idle_to_done() {
        let mut sm = Xd6StateMachine::new();
        assert!(sm.transition(Xd6State::Done).is_err());
    }

    #[test]
    fn xd_6_sm_invalid_idle_to_paused() {
        let mut sm = Xd6StateMachine::new();
        assert!(sm.transition(Xd6State::Paused).is_err());
    }

    #[test]
    fn xd_6_sm_history_tracking() {
        let mut sm = Xd6StateMachine::new();
        sm.transition(Xd6State::Running).unwrap();
        sm.transition(Xd6State::Paused).unwrap();
        sm.transition(Xd6State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd6State::Idle);
        assert_eq!(sm.history()[0].to, Xd6State::Running);
        assert_eq!(sm.history()[1].from, Xd6State::Running);
        assert_eq!(sm.history()[2].to, Xd6State::Done);
    }

    #[test]
    fn xd_6_sm_serialize_deserialize() {
        let mut sm = Xd6StateMachine::new();
        sm.transition(Xd6State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd6StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd6State::Running));
    }

    #[test]
    fn xd_6_sm_deserialize_invalid() {
        assert_eq!(Xd6StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_6_sm_reset() {
        let mut sm = Xd6StateMachine::new();
        sm.transition(Xd6State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd6State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_6_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd6EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd6Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_6_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd6EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd6Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd6Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_6_bus_unsubscribe() {
        let mut bus = Xd6EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_6_event_kind_and_payload() {
        let e = Xd6Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd6Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_6_bus_clear_history() {
        let mut bus = Xd6EventBus::new();
        bus.publish(Xd6Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_6_sm_step_counter_increments() {
        let mut sm = Xd6StateMachine::new();
        sm.transition(Xd6State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd6State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #4 --

    #[test]
    fn xf4_trie_insert_search() {
        let mut t = Xf4Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf4_trie_starts_with() {
        let mut t = Xf4Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf4_trie_remove() {
        let mut t = Xf4Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf4_trie_word_count() {
        let mut t = Xf4Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf4_trie_longest_prefix() {
        let mut t = Xf4Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf4_trie_all_words() {
        let mut t = Xf4Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf4_trie_autocomplete() {
        let mut t = Xf4Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf4_trie_empty_search() {
        let t = Xf4Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf4_bloom_add_contains() {
        let mut bf = Xf4BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf4_bloom_probably_absent() {
        let bf = Xf4BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf4_bloom_false_positive_rate() {
        let mut bf = Xf4BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf4_bloom_clear() {
        let mut bf = Xf4BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf4_bloom_union() {
        let mut a = Xf4BloomFilter::xf_new(512, 2);
        let mut b = Xf4BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf4_bloom_intersection_estimate() {
        let mut a = Xf4BloomFilter::xf_new(512, 2);
        let mut b = Xf4BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf4_bloom_union_size_mismatch() {
        let a = Xf4BloomFilter::xf_new(256, 2);
        let b = Xf4BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh46_skip_insert_contains() {
        let mut sl = super::Xh46SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh46_skip_remove() {
        let mut sl = super::Xh46SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh46_skip_len() {
        let mut sl = super::Xh46SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh46_skip_range_query() {
        let mut sl = super::Xh46SkipList::xh_new(4);
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
    fn xh46_skip_floor_ceiling() {
        let mut sl = super::Xh46SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh46_skip_rank() {
        let mut sl = super::Xh46SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh46_skip_empty() {
        let sl = super::Xh46SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh46_skip_duplicates() {
        let mut sl = super::Xh46SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh46_bitset_set_test() {
        let mut bs = super::Xh46BitSet::xh_new(256);
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
    fn xh46_bitset_clear_count() {
        let mut bs = super::Xh46BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh46_bitset_and_or_xor() {
        let mut a = super::Xh46BitSet::xh_new(128);
        let mut b = super::Xh46BitSet::xh_new(128);
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
    fn xh46_bitset_iter_ones() {
        let mut bs = super::Xh46BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh46_bitset_first_last() {
        let mut bs = super::Xh46BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh46_bitset_empty() {
        let bs = super::Xh46BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi46_deque_push_pop_back() {
        let mut dq = super::Xi46Deque::xi_new(4);
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
    fn xi46_deque_push_pop_front() {
        let mut dq = super::Xi46Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi46_deque_mixed_ops() {
        let mut dq = super::Xi46Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi46_deque_get_and_split() {
        let mut dq = super::Xi46Deque::xi_new(8);
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
    fn xi46_deque_rotate_left() {
        let mut dq = super::Xi46Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi46_deque_rotate_right() {
        let mut dq = super::Xi46Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi46_deque_grow() {
        let mut dq = super::Xi46Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi46_deque_empty() {
        let dq = super::Xi46Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi46_interval_tree_insert_query() {
        let mut tree = super::Xi46IntervalTree::xi_new();
        tree.xi_insert(super::Xi46Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi46Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi46Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi46_interval_tree_overlap() {
        let mut tree = super::Xi46IntervalTree::xi_new();
        tree.xi_insert(super::Xi46Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi46Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi46Interval::xi_new(12, 20));
        let q = super::Xi46Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi46_interval_tree_remove() {
        let mut tree = super::Xi46IntervalTree::xi_new();
        tree.xi_insert(super::Xi46Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi46Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi46_interval_tree_gaps() {
        let mut tree = super::Xi46IntervalTree::xi_new();
        tree.xi_insert(super::Xi46Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi46Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi46Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi46Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi46Interval::xi_new(8, 10));
    }

    #[test]
    fn xi46_interval_tree_merge() {
        let mut tree = super::Xi46IntervalTree::xi_new();
        tree.xi_insert(super::Xi46Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi46Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi46Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi46Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi46Interval::xi_new(10, 15));
    }

    #[test]
    fn xi46_interval_tree_all() {
        let mut tree = super::Xi46IntervalTree::xi_new();
        tree.xi_insert(super::Xi46Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi46Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi46_interval_tree_empty() {
        let tree = super::Xi46IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi46_interval_tree_contains_point() {
        let iv = super::Xi46Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 46) ---

    #[test]
    fn xj_46_uf_make_and_find() {
        let mut uf = super::Xj46UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_46_uf_union_connected() {
        let mut uf = super::Xj46UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_46_uf_component_count() {
        let mut uf = super::Xj46UnionFind::xj_new();
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
    fn xj_46_uf_component_size() {
        let mut uf = super::Xj46UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_46_uf_largest_component() {
        let mut uf = super::Xj46UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_46_uf_many_elements() {
        let mut uf = super::Xj46UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_46_uf_separate_components() {
        let mut uf = super::Xj46UnionFind::xj_new();
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
    fn xj_46_uf_path_compression() {
        let mut uf = super::Xj46UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_46_bt_insert_get() {
        let mut bt = super::Xj46BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_46_bt_contains_len() {
        let mut bt = super::Xj46BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_46_bt_replace() {
        let mut bt = super::Xj46BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_46_bt_remove() {
        let mut bt = super::Xj46BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_46_bt_keys_values() {
        let mut bt = super::Xj46BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_46_bt_range() {
        let mut bt = super::Xj46BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_46_bt_min_max() {
        let mut bt = super::Xj46BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_46_bt_many_inserts() {
        let mut bt = super::Xj46BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_46 segment tree tests ---

    #[test]
    fn xk_46_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk46SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_46_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk46SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_46_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk46SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_46_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk46SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_46_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk46SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_46_st_single_element() {
        let data = vec![42];
        let st = super::Xk46SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_46_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk46SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_46_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk46SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_46 disjoint intervals tests ---

    #[test]
    fn xk_46_di_add_and_count() {
        let mut di = super::Xk46DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_46_di_merge_overlap() {
        let mut di = super::Xk46DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_46_di_contains() {
        let mut di = super::Xk46DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_46_di_remove() {
        let mut di = super::Xk46DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_46_di_covered_length() {
        let mut di = super::Xk46DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_46_di_gaps() {
        let mut di = super::Xk46DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_46_di_merge_adjacent() {
        let mut di = super::Xk46DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_46_di_empty() {
        let di = super::Xk46DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_46_rope_new_empty() {
        let rope = super::Xl46Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_46_rope_from_str() {
        let rope = super::Xl46Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_46_rope_insert_at() {
        let mut rope = super::Xl46Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_46_rope_delete_range() {
        let mut rope = super::Xl46Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_46_rope_char_at() {
        let rope = super::Xl46Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_46_rope_split_concat() {
        let rope = super::Xl46Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_46_rope_line_count() {
        let rope = super::Xl46Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_46_rope_line_at() {
        let rope = super::Xl46Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_46_sa_build_and_search() {
        let sa = super::Xl46SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_46_sa_count() {
        let sa = super::Xl46SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_46_sa_longest_repeated() {
        let sa = super::Xl46SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_46_sa_all_positions() {
        let sa = super::Xl46SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_46_sa_len() {
        let sa = super::Xl46SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_46_sa_empty() {
        let sa = super::Xl46SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_46_rope_slice() {
        let rope = super::Xl46Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_46_sa_search_start() {
        let sa = super::Xl46SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_46_sparse_set_get() {
        let mut m = super::Xm46MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_46_sparse_row_col() {
        let mut m = super::Xm46MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_46_sparse_transpose() {
        let mut m = super::Xm46MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_46_sparse_multiply_vec() {
        let mut m = super::Xm46MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_46_sparse_nnz_density() {
        let mut m = super::Xm46MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_46_sparse_clear() {
        let mut m = super::Xm46MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_46_sparse_overwrite_zero() {
        let mut m = super::Xm46MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_46_tokenizer_basic() {
        let t = super::Xm46Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_46_tokenizer_count() {
        let t = super::Xm46Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_46_tokenizer_unique() {
        let t = super::Xm46Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_46_tokenizer_frequency() {
        let t = super::Xm46Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_46_tokenizer_delimiter() {
        let t = super::Xm46Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_46_tokenizer_whitespace() {
        let t = super::Xm46Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_46_tokenizer_empty() {
        let t = super::Xm46Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 46 ----

    #[test]
    fn xn_46_fenwick_prefix_sum() {
        let mut ft = super::Xn46Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_46_fenwick_range_sum() {
        let mut ft = super::Xn46Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_46_fenwick_point_query() {
        let mut ft = super::Xn46Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_46_fenwick_len() {
        let ft = super::Xn46Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_46_fenwick_multiple_updates() {
        let mut ft = super::Xn46Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_46_fenwick_single_element() {
        let mut ft = super::Xn46Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_46_fenwick_find_kth() {
        let mut ft = super::Xn46Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_46_fenwick_negative_delta() {
        let mut ft = super::Xn46Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 46 ----

    #[test]
    fn xn_46_avl_insert_get() {
        let mut m = super::Xn46AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_46_avl_remove() {
        let mut m = super::Xn46AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_46_avl_in_order() {
        let mut m = super::Xn46AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_46_avl_min_max() {
        let mut m = super::Xn46AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_46_avl_floor_ceiling() {
        let mut m = super::Xn46AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_46_avl_height_balanced() {
        let mut m = super::Xn46AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_46_avl_overwrite() {
        let mut m = super::Xn46AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_46_avl_empty() {
        let m: super::Xn46AVL<i32, i32> = super::Xn46AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
