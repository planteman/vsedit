//! File explorer view — equivalent to VS Code's explorer sidebar.
//!
//! Provides a tree-based directory browser with expand/collapse, selection,
//! scrolling, and rendering via ratatui.

use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

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
        }
    }

    /// Recursively load children from the filesystem.
    fn load_children(&mut self) {
        self.children.clear();
        if !self.is_dir {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&self.path) else {
            return;
        };
        let mut nodes: Vec<FileNode> = entries
            .filter_map(|e| e.ok())
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
}

impl FileTree {
    /// Create an empty tree.
    pub fn new() -> Self {
        Self { roots: Vec::new() }
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
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let mut root = FileNode::new(name, path.to_path_buf(), true, 0);
        root.load_children();
        root.is_expanded = true;
        Ok(Self { roots: vec![root] })
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
        if let Some(node) = Self::node_at_path_mut(&mut self.roots, &path) {
            if node.is_dir {
                node.is_expanded = !node.is_expanded;
                if node.is_expanded && node.children.is_empty() {
                    node.load_children();
                }
            }
        }
    }

    /// Total number of currently visible entries.
    pub fn visible_count(&self) -> usize {
        self.flatten().len()
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
}

impl Default for FileTree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ExplorerView
// ---------------------------------------------------------------------------

/// The file explorer view with tree navigation, selection, and rendering.
pub struct ExplorerView {
    tree: FileTree,
    selected_index: usize,
    scroll_offset: usize,
}

impl ExplorerView {
    /// Create a new empty `ExplorerView`.
    pub fn new() -> Self {
        Self {
            tree: FileTree::new(),
            selected_index: 0,
            scroll_offset: 0,
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

    /// Get the path of the currently selected entry, if any.
    pub fn get_selected_path(&self) -> Option<PathBuf> {
        let flat = self.tree.flatten();
        flat.get(self.selected_index)
            .map(|entry| entry.path.clone())
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

    /// Render the file tree into a ratatui buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let entries = self.tree.flatten();
        let height = area.height as usize;

        for (i, row) in (0..height).enumerate() {
            let entry_idx = self.scroll_offset + i;
            if entry_idx >= entries.len() {
                break;
            }
            let entry = &entries[entry_idx];
            let indent = "  ".repeat(entry.depth);
            let icon = if entry.is_dir {
                if entry.is_expanded {
                    "▾ "
                } else {
                    "▸ "
                }
            } else {
                "  "
            };
            let text = format!("{indent}{icon}{}", entry.name);

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
            let y = area.y + row as u16;
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
            assert!(
                di < fi,
                "dirs should sort before files"
            );
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
}
