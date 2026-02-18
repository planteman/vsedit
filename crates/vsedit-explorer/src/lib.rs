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

}
