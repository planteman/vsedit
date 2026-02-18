//! Comments view (code review comments).

use std::collections::HashMap;
use std::fmt;
#[derive(Debug, Clone)]
pub struct CommentReaction {
    pub label: String,
    pub count: u32,
    pub has_reacted: bool,
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub id: u64,
    pub author: String,
    pub body: String,
    pub timestamp: u64,
    pub reactions: Vec<CommentReaction>,
}

impl std::fmt::Display for Comment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.author, self.body)
    }
}

#[derive(Debug, Clone)]
pub struct CommentThread {
    pub id: String,
    pub uri: String,
    pub line: u32,
    pub comments: Vec<Comment>,
    pub resolved: bool,
    pub collapsed: bool,
}

impl CommentThread {
    pub fn comment_count(&self) -> usize {
        self.comments.len()
    }

    pub fn last_comment(&self) -> Option<&Comment> {
        self.comments.last()
    }

    pub fn add_reply(&mut self, author: &str, body: &str, timestamp: u64) {
        let id = self.comments.len() as u64 + 1;
        self.comments.push(Comment {
            id,
            author: author.to_string(),
            body: body.to_string(),
            timestamp,
            reactions: Vec::new(),
        });
    }

    pub fn resolve(&mut self) {
        self.resolved = true;
    }

    pub fn unresolve(&mut self) {
        self.resolved = false;
    }

    pub fn latest_comment(&self) -> Option<&Comment> {
        self.comments.last()
    }
}

pub struct CommentsService {
    threads: Vec<CommentThread>,
}

impl CommentsService {
    pub fn new() -> Self {
        Self {
            threads: Vec::new(),
        }
    }

    pub fn add_thread(&mut self, thread: CommentThread) {
        self.threads.push(thread);
    }

    pub fn add_comment(&mut self, thread_id: &str, comment: Comment) {
        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == thread_id) {
            thread.comments.push(comment);
        }
    }

    pub fn resolve_thread(&mut self, id: &str) {
        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == id) {
            thread.resolved = true;
        }
    }

    pub fn unresolve_thread(&mut self, id: &str) {
        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == id) {
            thread.resolved = false;
        }
    }

    pub fn get_threads_for_uri(&self, uri: &str) -> Vec<&CommentThread> {
        self.threads.iter().filter(|t| t.uri == uri).collect()
    }

    pub fn unresolved_count(&self) -> usize {
        self.threads.iter().filter(|t| !t.resolved).count()
    }

    pub fn get_thread(&self, id: &str) -> Option<&CommentThread> {
        self.threads.iter().find(|t| t.id == id)
    }

    pub fn get_thread_mut(&mut self, id: &str) -> Option<&mut CommentThread> {
        self.threads.iter_mut().find(|t| t.id == id)
    }

    pub fn remove_thread(&mut self, id: &str) -> bool {
        let len_before = self.threads.len();
        self.threads.retain(|t| t.id != id);
        self.threads.len() < len_before
    }

    pub fn toggle_collapsed(&mut self, id: &str) {
        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == id) {
            thread.collapsed = !thread.collapsed;
        }
    }

    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    pub fn total_comment_count(&self) -> usize {
        self.threads.iter().map(|t| t.comments.len()).sum()
    }

    pub fn get_all_threads(&self) -> &[CommentThread] {
        &self.threads
    }

    pub fn resolved_count(&self) -> usize {
        self.threads.iter().filter(|t| t.resolved).count()
    }
}

impl Default for CommentsService {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for CommentThread {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialEq for Comment {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentSortOrder {
    ByTimestamp,
    ByAuthor,
    ByLine,
}

impl std::fmt::Display for CommentSortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommentSortOrder::ByTimestamp => write!(f, "ByTimestamp"),
            CommentSortOrder::ByAuthor => write!(f, "ByAuthor"),
            CommentSortOrder::ByLine => write!(f, "ByLine"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommentFilter {
    pub author_filter: Option<String>,
    pub resolved_filter: Option<bool>,
    pub uri_filter: Option<String>,
}

impl CommentFilter {
    pub fn new() -> Self {
        Self {
            author_filter: None,
            resolved_filter: None,
            uri_filter: None,
        }
    }

    pub fn by_author(mut self, author: &str) -> Self {
        self.author_filter = Some(author.to_string());
        self
    }

    pub fn by_resolved(mut self, resolved: bool) -> Self {
        self.resolved_filter = Some(resolved);
        self
    }

    pub fn by_uri(mut self, uri: &str) -> Self {
        self.uri_filter = Some(uri.to_string());
        self
    }

    pub fn matches(&self, thread: &CommentThread) -> bool {
        if let Some(ref uri) = self.uri_filter {
            if thread.uri != *uri {
                return false;
            }
        }
        if let Some(resolved) = self.resolved_filter {
            if thread.resolved != resolved {
                return false;
            }
        }
        if let Some(ref author) = self.author_filter {
            if !thread.comments.iter().any(|c| c.author == *author) {
                return false;
            }
        }
        true
    }

    pub fn matches_comment(&self, comment: &Comment) -> bool {
        if let Some(ref author) = self.author_filter {
            if comment.author != *author {
                return false;
            }
        }
        true
    }
}

impl CommentThread {
    pub fn authors(&self) -> Vec<&str> {
        let mut seen = Vec::new();
        for c in &self.comments {
            if !seen.contains(&c.author.as_str()) {
                seen.push(c.author.as_str());
            }
        }
        seen
    }

    pub fn is_empty(&self) -> bool {
        self.comments.is_empty()
    }

    pub fn first_comment(&self) -> Option<&Comment> {
        self.comments.first()
    }

    pub fn latest_timestamp(&self) -> Option<u64> {
        self.comments.iter().map(|c| c.timestamp).max()
    }

    pub fn total_reactions(&self) -> u32 {
        self.comments.iter().flat_map(|c| &c.reactions).map(|r| r.count).sum()
    }
}

impl CommentsService {
    pub fn sort_threads(&mut self, order: &CommentSortOrder) {
        match order {
            CommentSortOrder::ByTimestamp => {
                self.threads.sort_by(|a, b| {
                    let ts_a = a.latest_timestamp().unwrap_or(0);
                    let ts_b = b.latest_timestamp().unwrap_or(0);
                    ts_a.cmp(&ts_b)
                });
            }
            CommentSortOrder::ByAuthor => {
                self.threads.sort_by(|a, b| {
                    let author_a = a.first_comment().map(|c| c.author.as_str()).unwrap_or("");
                    let author_b = b.first_comment().map(|c| c.author.as_str()).unwrap_or("");
                    author_a.cmp(author_b)
                });
            }
            CommentSortOrder::ByLine => {
                self.threads.sort_by_key(|t| t.line);
            }
        }
    }

    pub fn filter_threads(&self, filter: &CommentFilter) -> Vec<&CommentThread> {
        self.threads.iter().filter(|t| filter.matches(t)).collect()
    }

    pub fn threads_for_line_range(&self, uri: &str, start: u32, end: u32) -> Vec<&CommentThread> {
        self.threads
            .iter()
            .filter(|t| t.uri == uri && t.line >= start && t.line <= end)
            .collect()
    }

    pub fn all_authors(&self) -> Vec<String> {
        let mut authors = Vec::new();
        for thread in &self.threads {
            for comment in &thread.comments {
                if !authors.contains(&comment.author) {
                    authors.push(comment.author.clone());
                }
            }
        }
        authors
    }

    pub fn collapse_all(&mut self) {
        for thread in &mut self.threads {
            thread.collapsed = true;
        }
    }

    pub fn expand_all(&mut self) {
        for thread in &mut self.threads {
            thread.collapsed = false;
        }
    }

    pub fn resolve_all(&mut self) {
        for thread in &mut self.threads {
            thread.resolved = true;
        }
    }
}

impl std::fmt::Display for CommentThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Thread {}: {} comments", self.id, self.comment_count())
    }
}

impl std::fmt::Display for CommentReaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.label, self.count)
    }
}

/// Track resolution state changes for a comment thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionEvent {
    pub thread_id: String,
    pub resolved: bool,
    pub timestamp: u64,
    pub actor: String,
}

impl CommentsService {
    /// Search all threads for comments whose body contains the query string.
    pub fn search_comments(&self, query: &str) -> Vec<(&CommentThread, &Comment)> {
        let lower_query = query.to_ascii_lowercase();
        let mut results = Vec::new();
        for thread in &self.threads {
            for comment in &thread.comments {
                if comment.body.to_ascii_lowercase().contains(&lower_query) {
                    results.push((thread, comment));
                }
            }
        }
        results
    }

    /// Compute aggregate statistics for all threads.
    pub fn compute_statistics(&self) -> CommentStatistics {
        let total_threads = self.threads.len() as u32;
        let resolved_threads = self.threads.iter().filter(|t| t.resolved).count() as u32;
        let total_comments: u32 = self.threads.iter().map(|t| t.comments.len() as u32).sum();
        let total_reactions: u32 = self.threads.iter().map(|t| t.total_reactions()).sum();
        let unique_authors = self.all_authors().len() as u32;
        CommentStatistics {
            total_threads,
            resolved_threads,
            unresolved_threads: total_threads - resolved_threads,
            total_comments,
            total_reactions,
            unique_authors,
        }
    }

    /// Sort comments within each thread by timestamp.
    pub fn sort_comments_in_threads(&mut self) {
        for thread in &mut self.threads {
            thread.comments.sort_by_key(|c| c.timestamp);
        }
    }
}

/// Aggregate statistics for a comments service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentStatistics {
    pub total_threads: u32,
    pub resolved_threads: u32,
    pub unresolved_threads: u32,
    pub total_comments: u32,
    pub total_reactions: u32,
    pub unique_authors: u32,
}

impl fmt::Display for CommentStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "threads={}, comments={}, resolved={}",
            self.total_threads, self.total_comments, self.resolved_threads
        )
    }
}

pub fn comment_range_overlap<'a>(
    threads: &[&'a CommentThread],
    start_line: u32,
    end_line: u32,
) -> Vec<&'a CommentThread> {
    threads
        .iter()
        .filter(|t| t.line >= start_line && t.line <= end_line)
        .copied()
        .collect()
}

pub struct CommentController {
    threads: HashMap<String, Vec<CommentThread>>,
}

impl CommentController {
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
        }
    }

    pub fn add_thread(&mut self, uri: &str, thread: CommentThread) {
        self.threads
            .entry(uri.to_string())
            .or_default()
            .push(thread);
    }

    pub fn get_threads(&self, uri: &str) -> Vec<&CommentThread> {
        self.threads
            .get(uri)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn remove_thread(&mut self, uri: &str, thread_id: &str) -> bool {
        if let Some(v) = self.threads.get_mut(uri) {
            let before = v.len();
            v.retain(|t| t.id != thread_id);
            v.len() < before
        } else {
            false
        }
    }

    pub fn thread_count(&self, uri: &str) -> usize {
        self.threads.get(uri).map(|v| v.len()).unwrap_or(0)
    }

    pub fn all_uris(&self) -> Vec<&str> {
        self.threads.keys().map(|s| s.as_str()).collect()
    }

    pub fn resolve_all(&mut self, uri: &str) {
        if let Some(v) = self.threads.get_mut(uri) {
            for t in v.iter_mut() {
                t.resolved = true;
            }
        }
    }

    pub fn unresolved_count(&self, uri: &str) -> usize {
        self.threads
            .get(uri)
            .map(|v| v.iter().filter(|t| !t.resolved).count())
            .unwrap_or(0)
    }
}

/// Formats comment threads into different text representations.
pub struct CommentFormatter;

impl CommentFormatter {
    /// Format a thread as markdown with heading, author, body, and reactions.
    pub fn format_as_markdown(thread: &CommentThread) -> String {
        let mut out = String::new();
        let status = if thread.resolved { "✅ Resolved" } else { "❌ Unresolved" };
        out.push_str(&format!("## Thread {} ({})\n\n", thread.id, status));
        out.push_str(&format!("**File:** `{}` line {}\n\n", thread.uri, thread.line));
        for comment in &thread.comments {
            out.push_str(&format!("### {} (id: {})\n\n", comment.author, comment.id));
            out.push_str(&comment.body);
            out.push('\n');
            if !comment.reactions.is_empty() {
                let reactions: Vec<String> = comment
                    .reactions
                    .iter()
                    .map(|r| format!("{} ×{}", r.label, r.count))
                    .collect();
                out.push_str(&format!("\n> Reactions: {}\n", reactions.join(", ")));
            }
            out.push('\n');
        }
        out
    }

    /// Format a thread as plain text.
    pub fn format_as_plain(thread: &CommentThread) -> String {
        let mut out = String::new();
        let status = if thread.resolved { "Resolved" } else { "Unresolved" };
        out.push_str(&format!(
            "Thread {} [{}] - {}:{}\n",
            thread.id, status, thread.uri, thread.line
        ));
        for comment in &thread.comments {
            out.push_str(&format!("  {}: {}\n", comment.author, comment.body));
            if !comment.reactions.is_empty() {
                let reactions: Vec<String> = comment
                    .reactions
                    .iter()
                    .map(|r| format!("{} ({})", r.label, r.count))
                    .collect();
                out.push_str(&format!("    Reactions: {}\n", reactions.join(", ")));
            }
        }
        out
    }

    /// Produce a summary string for a slice of threads.
    pub fn format_summary(threads: &[CommentThread]) -> String {
        let total = threads.len();
        let resolved = threads.iter().filter(|t| t.resolved).count();
        let total_comments: usize = threads.iter().map(|t| t.comments.len()).sum();
        format!(
            "{} threads, {} resolved, {} total comments",
            total, resolved, total_comments
        )
    }
}

/// Builder-style search over comment threads.
#[derive(Debug, Clone, Default)]
pub struct CommentSearch {
    author: Option<String>,
    body_contains: Option<String>,
    resolved: Option<bool>,
}

impl CommentSearch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_author(mut self, author: &str) -> Self {
        self.author = Some(author.to_string());
        self
    }

    pub fn with_body_contains(mut self, text: &str) -> Self {
        self.body_contains = Some(text.to_ascii_lowercase());
        self
    }

    pub fn with_resolved(mut self, resolved: bool) -> Self {
        self.resolved = Some(resolved);
        self
    }

    /// Search and return owned clones of matching threads (useful for serialization).
    pub fn search_cloned(&self, threads: &[CommentThread]) -> Vec<CommentThread> {
        self.search(threads).into_iter().cloned().collect()
    }

    pub fn search<'a>(&self, threads: &'a [CommentThread]) -> Vec<&'a CommentThread> {
        threads
            .iter()
            .filter(|thread| {
                if let Some(resolved) = self.resolved {
                    if thread.resolved != resolved {
                        return false;
                    }
                }
                if let Some(ref author) = self.author {
                    if !thread.comments.iter().any(|c| c.author == *author) {
                        return false;
                    }
                }
                if let Some(ref text) = self.body_contains {
                    if !thread
                        .comments
                        .iter()
                        .any(|c| c.body.to_ascii_lowercase().contains(text.as_str()))
                    {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Comment age classification
// ---------------------------------------------------------------------------

/// Classifies a comment's age relative to a reference timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentAge {
    /// Created in the last hour.
    Recent,
    /// Created in the last 24 hours.
    Today,
    /// Created in the last 7 days.
    ThisWeek,
    /// Older than 7 days.
    Older,
}

impl fmt::Display for CommentAge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recent => write!(f, "Recent"),
            Self::Today => write!(f, "Today"),
            Self::ThisWeek => write!(f, "This Week"),
            Self::Older => write!(f, "Older"),
        }
    }
}

/// Classify a comment's age given the current time in seconds since epoch.
pub fn classify_comment_age(comment_timestamp: u64, now: u64) -> CommentAge {
    let elapsed = now.saturating_sub(comment_timestamp);
    const HOUR: u64 = 3600;
    const DAY: u64 = 86400;
    const WEEK: u64 = 604800;
    if elapsed < HOUR {
        CommentAge::Recent
    } else if elapsed < DAY {
        CommentAge::Today
    } else if elapsed < WEEK {
        CommentAge::ThisWeek
    } else {
        CommentAge::Older
    }
}

// ---------------------------------------------------------------------------
// Author statistics
// ---------------------------------------------------------------------------

/// Per-author statistics gathered from a set of comment threads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorStats {
    pub author: String,
    pub comment_count: u32,
    pub thread_count: u32,
    pub total_reactions_received: u32,
}

/// Collect author statistics from a slice of threads.
pub fn gather_author_stats(threads: &[CommentThread]) -> Vec<AuthorStats> {
    let mut map: HashMap<String, (u32, std::collections::HashSet<String>, u32)> = HashMap::new();
    for thread in threads {
        for comment in &thread.comments {
            let entry = map
                .entry(comment.author.clone())
                .or_insert_with(|| (0, std::collections::HashSet::new(), 0));
            entry.0 += 1;
            entry.1.insert(thread.id.clone());
            entry.2 += comment.reactions.iter().map(|r| r.count).sum::<u32>();
        }
    }
    let mut stats: Vec<AuthorStats> = map
        .into_iter()
        .map(|(author, (comment_count, threads_set, reactions))| AuthorStats {
            author,
            comment_count,
            thread_count: threads_set.len() as u32,
            total_reactions_received: reactions,
        })
        .collect();
    stats.sort_by(|a, b| b.comment_count.cmp(&a.comment_count));
    stats
}

// ---------------------------------------------------------------------------
// Thread grouping by URI
// ---------------------------------------------------------------------------

/// Groups threads by their URI, returning a sorted list of (uri, thread_count) pairs.
pub fn group_threads_by_uri(threads: &[CommentThread]) -> Vec<(String, usize)> {
    let mut map: HashMap<String, usize> = HashMap::new();
    for t in threads {
        *map.entry(t.uri.clone()).or_insert(0) += 1;
    }
    let mut groups: Vec<(String, usize)> = map.into_iter().collect();
    groups.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    groups
}

/// Return only threads that have at least one comment newer than `since`.
pub fn threads_with_recent_activity(threads: &[CommentThread], since: u64) -> Vec<&CommentThread> {
    threads
        .iter()
        .filter(|t| t.comments.iter().any(|c| c.timestamp >= since))
        .collect()
}

// ---------------------------------------------------------------------------
// Comment threading / nesting with depth tracking
// ---------------------------------------------------------------------------

/// A comment that supports nesting via `parent_id` and tracks its depth.
#[derive(Debug, Clone)]
pub struct NestedComment {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub author: String,
    pub body: String,
    pub timestamp: u64,
    pub depth: u32,
}

/// Builds a tree of nested comments from a flat list, computing depth for each.
pub fn compute_comment_depths(comments: &mut [NestedComment]) {
    // Index parent depths.
    let depth_map: HashMap<u64, u32> = comments
        .iter()
        .filter(|c| c.parent_id.is_none())
        .map(|c| (c.id, 0))
        .collect();

    // Multi-pass: propagate depths until stable.
    let mut depth_map = depth_map;
    let max_iter = comments.len();
    for _ in 0..max_iter {
        let mut changed = false;
        for c in comments.iter() {
            if depth_map.contains_key(&c.id) {
                continue;
            }
            if let Some(pid) = c.parent_id {
                if let Some(&parent_depth) = depth_map.get(&pid) {
                    depth_map.insert(c.id, parent_depth + 1);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    for c in comments.iter_mut() {
        c.depth = depth_map.get(&c.id).copied().unwrap_or(0);
    }
}

/// Returns the direct children of a given comment id from a flat list.
pub fn children_of(comments: &[NestedComment], parent_id: u64) -> Vec<&NestedComment> {
    comments
        .iter()
        .filter(|c| c.parent_id == Some(parent_id))
        .collect()
}

// ---------------------------------------------------------------------------
// Comment date-range filtering
// ---------------------------------------------------------------------------

/// Filter threads to those that have at least one comment within `[start, end]`.
pub fn threads_in_date_range<'a>(
    threads: &'a [CommentThread],
    start: u64,
    end: u64,
) -> Vec<&'a CommentThread> {
    threads
        .iter()
        .filter(|t| {
            t.comments
                .iter()
                .any(|c| c.timestamp >= start && c.timestamp <= end)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Batch operations
// ---------------------------------------------------------------------------

impl CommentsService {
    /// Resolve every thread whose first comment was authored by `author`.
    pub fn resolve_all_by_author(&mut self, author: &str) -> usize {
        let mut count = 0;
        for thread in &mut self.threads {
            if !thread.resolved
                && thread
                    .comments
                    .first()
                    .map_or(false, |c| c.author == author)
            {
                thread.resolved = true;
                count += 1;
            }
        }
        count
    }

    /// Remove all threads whose first comment was authored by `author`.
    pub fn remove_threads_by_author(&mut self, author: &str) -> usize {
        let before = self.threads.len();
        self.threads.retain(|t| {
            t.comments
                .first()
                .map_or(true, |c| c.author != author)
        });
        before - self.threads.len()
    }

    /// Delete all comments by a specific author across every thread, returning
    /// the number of comments removed.
    pub fn delete_comments_by_author(&mut self, author: &str) -> usize {
        let mut removed = 0;
        for thread in &mut self.threads {
            let before = thread.comments.len();
            thread.comments.retain(|c| c.author != author);
            removed += before - thread.comments.len();
        }
        removed
    }
}

// ---------------------------------------------------------------------------
// Comment diff tracking
// ---------------------------------------------------------------------------

/// Represents a range of changed lines in a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub start_line: u32,
    pub end_line: u32,
}

/// Identifies which threads sit on lines that were changed in a diff.
pub fn threads_on_changed_lines<'a>(
    threads: &'a [CommentThread],
    uri: &str,
    hunks: &[DiffHunk],
) -> Vec<&'a CommentThread> {
    threads
        .iter()
        .filter(|t| {
            t.uri == uri
                && hunks
                    .iter()
                    .any(|h| t.line >= h.start_line && t.line <= h.end_line)
        })
        .collect()
}

/// Returns threads that are *not* on any changed line (potentially outdated).
pub fn threads_outside_changed_lines<'a>(
    threads: &'a [CommentThread],
    uri: &str,
    hunks: &[DiffHunk],
) -> Vec<&'a CommentThread> {
    threads
        .iter()
        .filter(|t| {
            t.uri == uri
                && !hunks
                    .iter()
                    .any(|h| t.line >= h.start_line && t.line <= h.end_line)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Multi-thread markdown export
// ---------------------------------------------------------------------------

impl CommentFormatter {
    /// Export multiple threads to a single markdown document.
    pub fn export_threads_as_markdown(threads: &[CommentThread]) -> String {
        let mut out = String::from("# Code Review Comments\n\n");
        out.push_str(&format!(
            "**Total threads:** {} | **Resolved:** {} | **Unresolved:** {}\n\n---\n\n",
            threads.len(),
            threads.iter().filter(|t| t.resolved).count(),
            threads.iter().filter(|t| !t.resolved).count(),
        ));
        for thread in threads {
            out.push_str(&Self::format_as_markdown(thread));
            out.push_str("---\n\n");
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Comment draft management
// ---------------------------------------------------------------------------

/// A draft comment that has not yet been submitted.
#[derive(Debug, Clone)]
pub struct CommentDraft {
    pub thread_id: Option<String>,
    pub uri: String,
    pub line: u32,
    pub author: String,
    pub body: String,
    pub created_at: u64,
    pub modified_at: u64,
}

/// Manages pending comment drafts across multiple files.
pub struct DraftManager {
    drafts: Vec<CommentDraft>,
}

impl DraftManager {
    pub fn new() -> Self {
        Self {
            drafts: Vec::new(),
        }
    }

    /// Add a new draft. Returns the index at which it was inserted.
    pub fn add_draft(&mut self, draft: CommentDraft) -> usize {
        self.drafts.push(draft);
        self.drafts.len() - 1
    }

    /// Get all drafts for a given URI.
    pub fn drafts_for_uri(&self, uri: &str) -> Vec<&CommentDraft> {
        self.drafts.iter().filter(|d| d.uri == uri).collect()
    }

    /// Get all drafts by a given author.
    pub fn drafts_by_author(&self, author: &str) -> Vec<&CommentDraft> {
        self.drafts.iter().filter(|d| d.author == author).collect()
    }

    /// Update the body of a draft at the given index. Returns `true` if updated.
    pub fn update_draft(&mut self, index: usize, new_body: &str, modified_at: u64) -> bool {
        if let Some(draft) = self.drafts.get_mut(index) {
            draft.body = new_body.to_string();
            draft.modified_at = modified_at;
            true
        } else {
            false
        }
    }

    /// Remove a draft at the given index. Returns the removed draft if valid.
    pub fn remove_draft(&mut self, index: usize) -> Option<CommentDraft> {
        if index < self.drafts.len() {
            Some(self.drafts.remove(index))
        } else {
            None
        }
    }

    /// Discard all drafts.
    pub fn clear(&mut self) {
        self.drafts.clear();
    }

    /// Total number of pending drafts.
    pub fn count(&self) -> usize {
        self.drafts.len()
    }

    /// Convert a draft into a real `Comment` and append it to the given thread.
    /// The draft is consumed (removed) and a new comment ID is assigned.
    pub fn submit_draft(
        &mut self,
        draft_index: usize,
        thread: &mut CommentThread,
        submit_time: u64,
    ) -> bool {
        if let Some(draft) = self.remove_draft(draft_index) {
            let id = thread.comments.len() as u64 + 1;
            thread.comments.push(Comment {
                id,
                author: draft.author,
                body: draft.body,
                timestamp: submit_time,
                reactions: Vec::new(),
            });
            true
        } else {
            false
        }
    }
}

impl Default for DraftManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Comment navigation – next/previous unresolved thread
// ---------------------------------------------------------------------------

/// Navigate among unresolved threads within a single URI, ordered by line.
pub struct ThreadNavigator;

impl ThreadNavigator {
    /// Find the next unresolved thread after the given line in the same URI.
    /// Wraps around to the beginning if no match is found after `current_line`.
    pub fn next_unresolved<'a>(
        threads: &'a [CommentThread],
        uri: &str,
        current_line: u32,
    ) -> Option<&'a CommentThread> {
        let mut candidates: Vec<&CommentThread> = threads
            .iter()
            .filter(|t| t.uri == uri && !t.resolved)
            .collect();
        candidates.sort_by_key(|t| t.line);

        // Try after current_line first, then wrap.
        candidates
            .iter()
            .find(|t| t.line > current_line)
            .or_else(|| candidates.first())
            .copied()
    }

    /// Find the previous unresolved thread before the given line in the same URI.
    /// Wraps around to the end if no match is found before `current_line`.
    pub fn prev_unresolved<'a>(
        threads: &'a [CommentThread],
        uri: &str,
        current_line: u32,
    ) -> Option<&'a CommentThread> {
        let mut candidates: Vec<&CommentThread> = threads
            .iter()
            .filter(|t| t.uri == uri && !t.resolved)
            .collect();
        candidates.sort_by_key(|t| t.line);

        candidates
            .iter()
            .rev()
            .find(|t| t.line < current_line)
            .or_else(|| candidates.last())
            .copied()
    }

    /// Count how many unresolved threads remain for a given URI.
    pub fn unresolved_remaining(threads: &[CommentThread], uri: &str) -> usize {
        threads
            .iter()
            .filter(|t| t.uri == uri && !t.resolved)
            .count()
    }
}

// ---------------------------------------------------------------------------
// Comment text helpers
// ---------------------------------------------------------------------------

/// Utilities for processing comment text.
pub struct CommentText;

impl CommentText {
    /// Extract all `@mention` user-names from a comment body.
    pub fn extract_mentions(body: &str) -> Vec<&str> {
        let mut mentions = Vec::new();
        for word in body.split_whitespace() {
            if let Some(name) = word.strip_prefix('@') {
                // Trim trailing punctuation so "@alice," becomes "alice"
                let name = name.trim_end_matches(|c: char| c.is_ascii_punctuation());
                if !name.is_empty() && !mentions.contains(&name) {
                    mentions.push(name);
                }
            }
        }
        mentions
    }

    /// Truncate a comment body to `max_len` characters, appending "…" if truncated.
    pub fn truncate(body: &str, max_len: usize) -> String {
        if body.len() <= max_len {
            body.to_string()
        } else {
            let mut s = body[..max_len].to_string();
            s.push('…');
            s
        }
    }

    /// Count the number of words in a comment body.
    pub fn word_count(body: &str) -> usize {
        body.split_whitespace().count()
    }

    /// Check whether the body contains a code block (fenced with ```).
    pub fn has_code_block(body: &str) -> bool {
        body.contains("```")
    }
}

// ---------------------------------------------------------------------------
// Per-file comment summary
// ---------------------------------------------------------------------------

/// A compact summary of comment activity for a single file URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCommentSummary {
    pub uri: String,
    pub total_threads: usize,
    pub resolved_threads: usize,
    pub unresolved_threads: usize,
    pub total_comments: usize,
    pub unique_authors: usize,
}

/// Build per-file summaries from a set of threads.
pub fn build_file_summaries(threads: &[CommentThread]) -> Vec<FileCommentSummary> {
    let mut map: HashMap<String, (usize, usize, usize, Vec<String>)> = HashMap::new();
    for t in threads {
        let entry = map.entry(t.uri.clone()).or_insert_with(|| (0, 0, 0, Vec::new()));
        entry.0 += 1; // total_threads
        if t.resolved {
            entry.1 += 1; // resolved
        }
        entry.2 += t.comments.len(); // total_comments
        for c in &t.comments {
            if !entry.3.contains(&c.author) {
                entry.3.push(c.author.clone());
            }
        }
    }
    let mut summaries: Vec<FileCommentSummary> = map
        .into_iter()
        .map(|(uri, (total, resolved, comments, authors))| FileCommentSummary {
            uri,
            total_threads: total,
            resolved_threads: resolved,
            unresolved_threads: total - resolved,
            total_comments: comments,
            unique_authors: authors.len(),
        })
        .collect();
    summaries.sort_by(|a, b| b.unresolved_threads.cmp(&a.unresolved_threads).then_with(|| a.uri.cmp(&b.uri)));
    summaries
}

impl fmt::Display for FileCommentSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} threads ({} unresolved), {} comments",
            self.uri, self.total_threads, self.unresolved_threads, self.total_comments,
        )
    }
}


// ---------------------------------------------------------------------------
// CommentsTreeCollapser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommentsTreeCollapser {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl CommentsTreeCollapser {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for CommentsTreeCollapser {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for CommentsTreeCollapser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "CommentsTreeCollapser({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// CommentSortOptions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommentSortOptions {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl CommentSortOptions {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for CommentSortOptions {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for CommentSortOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "CommentSortOptions({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// CommentsTreeCollapserSnapshot — point-in-time snapshot of CommentsTreeCollapser state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommentsTreeCollapserSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl CommentsTreeCollapserSnapshot {
    pub fn capture(source: &CommentsTreeCollapser, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for CommentsTreeCollapserSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// CommentSortOptionsStats — aggregate statistics for CommentSortOptions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct CommentSortOptionsStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl CommentSortOptionsStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for CommentSortOptionsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// CommentsTreeCollapserConfig — configuration for CommentsTreeCollapser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommentsTreeCollapserConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl CommentsTreeCollapserConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for CommentsTreeCollapserConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for CommentsTreeCollapserConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// CommentThreadGroup — group and query threads
// ---------------------------------------------------------------------------

/// Groups comment threads by file URI for efficient querying.
#[derive(Debug, Clone)]
pub struct CommentThreadGroup {
    threads: Vec<CommentThread>,
}

impl CommentThreadGroup {
    pub fn new() -> Self {
        Self { threads: Vec::new() }
    }

    pub fn from_threads(threads: Vec<CommentThread>) -> Self {
        Self { threads }
    }

    pub fn add(&mut self, thread: CommentThread) {
        self.threads.push(thread);
    }

    /// Get all threads for a given file URI.
    pub fn threads_for_file(&self, uri: &str) -> Vec<&CommentThread> {
        self.threads.iter().filter(|t| t.uri == uri).collect()
    }

    /// Get unique file URIs that have threads.
    pub fn files_with_threads(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.threads.iter().map(|t| t.uri.as_str()).collect();
        uris.sort();
        uris.dedup();
        uris
    }

    /// Count of threads by resolved status.
    pub fn resolved_count(&self) -> usize {
        self.threads.iter().filter(|t| t.resolved).count()
    }

    pub fn unresolved_count(&self) -> usize {
        self.threads.iter().filter(|t| !t.resolved).count()
    }

    /// Total number of comments across all threads.
    pub fn total_comments(&self) -> usize {
        self.threads.iter().map(|t| t.comments.len()).sum()
    }

    /// Unique authors across all comments.
    pub fn unique_authors(&self) -> Vec<String> {
        let mut authors: Vec<String> = self.threads.iter()
            .flat_map(|t| t.comments.iter().map(|c| c.author.clone()))
            .collect();
        authors.sort();
        authors.dedup();
        authors
    }

    /// Threads sorted by most recent activity (latest comment timestamp).
    pub fn by_recent_activity(&self) -> Vec<&CommentThread> {
        let mut sorted: Vec<&CommentThread> = self.threads.iter().collect();
        sorted.sort_by(|a, b| {
            let a_ts = a.comments.last().map(|c| c.timestamp).unwrap_or(0);
            let b_ts = b.comments.last().map(|c| c.timestamp).unwrap_or(0);
            b_ts.cmp(&a_ts)
        });
        sorted
    }

    pub fn len(&self) -> usize { self.threads.len() }
    pub fn is_empty(&self) -> bool { self.threads.is_empty() }
}

impl Default for CommentThreadGroup {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// CommentMentionExtractor — find @mentions in comment bodies
// ---------------------------------------------------------------------------

/// Extract @mentions from comment text.
pub struct CommentMentionExtractor;

impl CommentMentionExtractor {
    /// Extract all @mentions from a body string.
    pub fn extract_mentions(body: &str) -> Vec<String> {
        let mut mentions = Vec::new();
        let mut chars = body.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '@' {
                let mut name = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' || nc == '-' {
                        name.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !name.is_empty() {
                    mentions.push(name);
                }
            }
        }
        mentions
    }

    /// Extract mentions from all comments in a thread.
    pub fn thread_mentions(thread: &CommentThread) -> Vec<String> {
        let mut all: Vec<String> = thread.comments.iter()
            .flat_map(|c| Self::extract_mentions(&c.body))
            .collect();
        all.sort();
        all.dedup();
        all
    }

    /// Check if a specific user is mentioned in a thread.
    pub fn is_mentioned(thread: &CommentThread, username: &str) -> bool {
        thread.comments.iter().any(|c| {
            Self::extract_mentions(&c.body).iter().any(|m| m == username)
        })
    }
}

// ---------------------------------------------------------------------------
// CommentActivityTracker — track comment activity over time
// ---------------------------------------------------------------------------

/// Tracks comment activity timestamps for rate/frequency analysis.
#[derive(Debug, Clone)]
pub struct CommentActivityTracker {
    events: Vec<(String, u64)>,
}

impl CommentActivityTracker {
    pub fn new() -> Self { Self { events: Vec::new() } }

    pub fn record(&mut self, author: impl Into<String>, timestamp: u64) {
        self.events.push((author.into(), timestamp));
    }

    /// Events within a time range.
    pub fn events_in_range(&self, start: u64, end: u64) -> Vec<&(String, u64)> {
        self.events.iter().filter(|(_, ts)| *ts >= start && *ts <= end).collect()
    }

    /// Most active author by event count.
    pub fn most_active_author(&self) -> Option<String> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (author, _) in &self.events {
            *counts.entry(author.as_str()).or_insert(0) += 1;
        }
        counts.into_iter().max_by_key(|&(_, c)| c).map(|(a, _)| a.to_string())
    }

    /// Total number of tracked events.
    pub fn total_events(&self) -> usize { self.events.len() }
}

impl Default for CommentActivityTracker {
    fn default() -> Self { Self::new() }
}


/// Configuration manager for comments_view functionality.
pub struct CommentsViewConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl CommentsViewConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &CommentsViewConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for comments_view operations.
pub struct CommentsViewRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl CommentsViewRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for comments_view.
pub struct CommentsViewValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl CommentsViewValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &CommentsViewValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Comments panel UI model — extended utilities (yr)
// ---------------------------------------------------------------------------

/// Metric accumulator for cmt_view operations.
#[derive(Debug, Clone)]
pub struct YrMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YrMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for cmt_view.
#[derive(Debug, Clone)]
pub struct YrRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YrRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for cmt_view lookups.
#[derive(Debug, Clone)]
pub struct YrLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YrLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for comments_view
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaCommentsViewRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaCommentsViewRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaCommentsViewCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaCommentsViewCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaCommentsViewCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 21
// ---------------------------------------------------------------------------

/// Generic object pool `Xc21Pool<T>`.
pub struct Xc21Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc21Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc21PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc21Pool<T> {
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
    pub fn stats(&self) -> Xc21PoolStats {
        Xc21PoolStats {
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

impl<T> Default for Xc21Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc21Scheduler`.
pub struct Xc21Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc21Scheduler {
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

impl Default for Xc21Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_21 hash for the given byte slice.
pub fn xc_21_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_21 convention.
pub fn xc_21_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_102 deepening: state machine + event bus ---

/// States for the Xd102 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd102State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd102State {
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
pub struct Xd102Transition {
    pub from: Xd102State,
    pub to: Xd102State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd102StateMachine {
    current: Xd102State,
    history: Vec<Xd102Transition>,
    step_counter: usize,
}

impl Xd102StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd102State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd102State {
        self.current
    }

    pub fn history(&self) -> &[Xd102Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd102State) -> Result<Xd102State, String> {
        let allowed = match (self.current, target) {
            (Xd102State::Idle, Xd102State::Running) => true,
            (Xd102State::Running, Xd102State::Paused) => true,
            (Xd102State::Running, Xd102State::Done) => true,
            (Xd102State::Paused, Xd102State::Running) => true,
            (Xd102State::Paused, Xd102State::Done) => true,
            (Xd102State::Done, Xd102State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_102: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd102Transition {
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
            "Xd102SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd102State> {
        let prefix = "Xd102SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd102State::Idle),
            "Running" => Some(Xd102State::Running),
            "Paused" => Some(Xd102State::Paused),
            "Done" => Some(Xd102State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd102State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd102 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd102Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd102Event {
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

type Xd102HandlerFn = Box<dyn Fn(&Xd102Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd102EventBus {
    handlers: Vec<(usize, Option<String>, Xd102HandlerFn)>,
    next_id: usize,
    published: Vec<Xd102Event>,
}

impl Xd102EventBus {
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
        F: Fn(&Xd102Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd102Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd102Event) {
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

    pub fn published_events(&self) -> &[Xd102Event] {
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
// xg_26: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg26Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg26Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg26Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_26: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg26Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg26Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg26Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg26Heap<T> {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_thread(id: &str, uri: &str, line: u32) -> CommentThread {
        CommentThread {
            id: id.to_string(),
            uri: uri.to_string(),
            line,
            comments: Vec::new(),
            resolved: false,
            collapsed: false,
        }
    }

    #[test]
    fn add_thread_and_comment() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "file.rs", 10));
        svc.add_comment("t1", Comment {
            id: 1,
            author: "alice".into(),
            body: "looks good".into(),
            timestamp: 100,
            reactions: Vec::new(),
        });
        let threads = svc.get_threads_for_uri("file.rs");
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].comments.len(), 1);
        assert_eq!(threads[0].comments[0].author, "alice");
    }

    #[test]
    fn resolve_and_unresolve() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "a.rs", 1));
        svc.add_thread(make_thread("t2", "b.rs", 2));
        assert_eq!(svc.unresolved_count(), 2);
        svc.resolve_thread("t1");
        assert_eq!(svc.unresolved_count(), 1);
        svc.unresolve_thread("t1");
        assert_eq!(svc.unresolved_count(), 2);
    }

    #[test]
    fn filter_by_uri() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "file_a.rs", 5));
        svc.add_thread(make_thread("t2", "file_b.rs", 10));
        svc.add_thread(make_thread("t3", "file_a.rs", 20));
        assert_eq!(svc.get_threads_for_uri("file_a.rs").len(), 2);
        assert_eq!(svc.get_threads_for_uri("file_b.rs").len(), 1);
        assert_eq!(svc.get_threads_for_uri("missing.rs").len(), 0);
    }

    #[test]
    fn get_thread_by_id() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "file.rs", 1));
        assert!(svc.get_thread("t1").is_some());
        assert_eq!(svc.get_thread("t1").unwrap().uri, "file.rs");
        assert!(svc.get_thread("missing").is_none());
    }

    #[test]
    fn get_thread_mut_by_id() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "file.rs", 1));
        svc.get_thread_mut("t1").unwrap().line = 99;
        assert_eq!(svc.get_thread("t1").unwrap().line, 99);
    }

    #[test]
    fn remove_thread_returns_true_when_found() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "file.rs", 1));
        svc.add_thread(make_thread("t2", "file.rs", 2));
        assert!(svc.remove_thread("t1"));
        assert_eq!(svc.thread_count(), 1);
        assert!(!svc.remove_thread("t1"));
    }

    #[test]
    fn toggle_collapsed() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "file.rs", 1));
        assert!(!svc.get_thread("t1").unwrap().collapsed);
        svc.toggle_collapsed("t1");
        assert!(svc.get_thread("t1").unwrap().collapsed);
        svc.toggle_collapsed("t1");
        assert!(!svc.get_thread("t1").unwrap().collapsed);
    }

    #[test]
    fn thread_and_comment_counts() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "a.rs", 1));
        svc.add_thread(make_thread("t2", "b.rs", 2));
        assert_eq!(svc.thread_count(), 2);
        assert_eq!(svc.total_comment_count(), 0);
        svc.add_comment("t1", Comment {
            id: 1,
            author: "bob".into(),
            body: "hi".into(),
            timestamp: 1,
            reactions: Vec::new(),
        });
        svc.add_comment("t2", Comment {
            id: 2,
            author: "carol".into(),
            body: "hello".into(),
            timestamp: 2,
            reactions: Vec::new(),
        });
        assert_eq!(svc.total_comment_count(), 2);
    }

    #[test]
    fn resolved_count_and_get_all_threads() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "a.rs", 1));
        svc.add_thread(make_thread("t2", "b.rs", 2));
        svc.add_thread(make_thread("t3", "c.rs", 3));
        assert_eq!(svc.resolved_count(), 0);
        svc.resolve_thread("t1");
        svc.resolve_thread("t3");
        assert_eq!(svc.resolved_count(), 2);
        assert_eq!(svc.get_all_threads().len(), 3);
    }

    #[test]
    fn comment_thread_helpers() {
        let mut thread = make_thread("t1", "file.rs", 5);
        assert_eq!(thread.comment_count(), 0);
        assert!(thread.last_comment().is_none());
        thread.comments.push(Comment {
            id: 1,
            author: "alice".into(),
            body: "first".into(),
            timestamp: 10,
            reactions: Vec::new(),
        });
        thread.comments.push(Comment {
            id: 2,
            author: "bob".into(),
            body: "second".into(),
            timestamp: 20,
            reactions: Vec::new(),
        });
        assert_eq!(thread.comment_count(), 2);
        assert_eq!(thread.last_comment().unwrap().body, "second");
    }

    #[test]
    fn comment_display() {
        let c = Comment {
            id: 1,
            author: "alice".into(),
            body: "looks good".into(),
            timestamp: 100,
            reactions: Vec::new(),
        };
        assert_eq!(format!("{}", c), "alice: looks good");
    }

    #[test]
    fn comment_reactions() {
        let c = Comment {
            id: 1,
            author: "alice".into(),
            body: "nice".into(),
            timestamp: 100,
            reactions: vec![
                CommentReaction {
                    label: "👍".into(),
                    count: 3,
                    has_reacted: true,
                },
                CommentReaction {
                    label: "❤️".into(),
                    count: 1,
                    has_reacted: false,
                },
            ],
        };
        assert_eq!(c.reactions.len(), 2);
        assert_eq!(c.reactions[0].count, 3);
        assert!(c.reactions[0].has_reacted);
        assert!(!c.reactions[1].has_reacted);
    }

    fn make_comment(id: u64, author: &str, body: &str, timestamp: u64) -> Comment {
        Comment {
            id,
            author: author.to_string(),
            body: body.to_string(),
            timestamp,
            reactions: Vec::new(),
        }
    }

    fn make_comment_with_reactions(id: u64, author: &str, reactions: Vec<CommentReaction>) -> Comment {
        Comment {
            id,
            author: author.to_string(),
            body: "msg".to_string(),
            timestamp: 1,
            reactions,
        }
    }

    #[test]
    fn test_comment_sort_order_display() {
        assert_eq!(format!("{}", CommentSortOrder::ByTimestamp), "ByTimestamp");
        assert_eq!(format!("{}", CommentSortOrder::ByAuthor), "ByAuthor");
        assert_eq!(format!("{}", CommentSortOrder::ByLine), "ByLine");
    }

    #[test]
    fn test_comment_filter_by_author() {
        let filter = CommentFilter::new().by_author("alice");
        assert_eq!(filter.author_filter, Some("alice".to_string()));
        assert_eq!(filter.resolved_filter, None);
        assert_eq!(filter.uri_filter, None);
    }

    #[test]
    fn test_comment_filter_by_resolved() {
        let filter = CommentFilter::new().by_resolved(true);
        assert_eq!(filter.resolved_filter, Some(true));
    }

    #[test]
    fn test_comment_filter_by_uri() {
        let filter = CommentFilter::new().by_uri("main.rs");
        assert_eq!(filter.uri_filter, Some("main.rs".to_string()));
    }

    #[test]
    fn test_comment_filter_matches() {
        let mut thread = make_thread("t1", "file.rs", 1);
        thread.comments.push(make_comment(1, "alice", "hi", 10));
        thread.resolved = true;

        let filter_author = CommentFilter::new().by_author("alice");
        assert!(filter_author.matches(&thread));

        let filter_wrong_author = CommentFilter::new().by_author("bob");
        assert!(!filter_wrong_author.matches(&thread));

        let filter_resolved = CommentFilter::new().by_resolved(true);
        assert!(filter_resolved.matches(&thread));

        let filter_unresolved = CommentFilter::new().by_resolved(false);
        assert!(!filter_unresolved.matches(&thread));

        let filter_uri = CommentFilter::new().by_uri("file.rs");
        assert!(filter_uri.matches(&thread));

        let filter_wrong_uri = CommentFilter::new().by_uri("other.rs");
        assert!(!filter_wrong_uri.matches(&thread));

        let combined = CommentFilter::new().by_author("alice").by_resolved(true).by_uri("file.rs");
        assert!(combined.matches(&thread));
    }

    #[test]
    fn test_thread_authors() {
        let mut thread = make_thread("t1", "file.rs", 1);
        thread.comments.push(make_comment(1, "alice", "hi", 10));
        thread.comments.push(make_comment(2, "bob", "hey", 20));
        thread.comments.push(make_comment(3, "alice", "again", 30));
        let authors = thread.authors();
        assert_eq!(authors, vec!["alice", "bob"]);
    }

    #[test]
    fn test_thread_is_empty() {
        let thread = make_thread("t1", "file.rs", 1);
        assert!(thread.is_empty());
        let mut thread2 = make_thread("t2", "file.rs", 2);
        thread2.comments.push(make_comment(1, "alice", "hi", 10));
        assert!(!thread2.is_empty());
    }

    #[test]
    fn test_thread_first_comment() {
        let thread = make_thread("t1", "file.rs", 1);
        assert!(thread.first_comment().is_none());
        let mut thread2 = make_thread("t2", "file.rs", 2);
        thread2.comments.push(make_comment(1, "alice", "first", 10));
        thread2.comments.push(make_comment(2, "bob", "second", 20));
        assert_eq!(thread2.first_comment().unwrap().body, "first");
    }

    #[test]
    fn test_thread_latest_timestamp() {
        let thread = make_thread("t1", "file.rs", 1);
        assert_eq!(thread.latest_timestamp(), None);
        let mut thread2 = make_thread("t2", "file.rs", 2);
        thread2.comments.push(make_comment(1, "alice", "hi", 10));
        thread2.comments.push(make_comment(2, "bob", "hey", 50));
        thread2.comments.push(make_comment(3, "carol", "yo", 30));
        assert_eq!(thread2.latest_timestamp(), Some(50));
    }

    #[test]
    fn test_thread_total_reactions() {
        let mut thread = make_thread("t1", "file.rs", 1);
        thread.comments.push(make_comment_with_reactions(1, "alice", vec![
            CommentReaction { label: "👍".into(), count: 3, has_reacted: false },
            CommentReaction { label: "❤️".into(), count: 2, has_reacted: false },
        ]));
        thread.comments.push(make_comment_with_reactions(2, "bob", vec![
            CommentReaction { label: "🎉".into(), count: 5, has_reacted: true },
        ]));
        assert_eq!(thread.total_reactions(), 10);
    }

    #[test]
    fn test_service_sort_threads() {
        let mut svc = CommentsService::new();
        let mut t1 = make_thread("t1", "file.rs", 30);
        t1.comments.push(make_comment(1, "carol", "hi", 100));
        let mut t2 = make_thread("t2", "file.rs", 10);
        t2.comments.push(make_comment(2, "alice", "hey", 50));
        let mut t3 = make_thread("t3", "file.rs", 20);
        t3.comments.push(make_comment(3, "bob", "yo", 200));
        svc.add_thread(t1);
        svc.add_thread(t2);
        svc.add_thread(t3);

        svc.sort_threads(&CommentSortOrder::ByLine);
        let threads = svc.get_all_threads();
        assert_eq!(threads[0].id, "t2");
        assert_eq!(threads[1].id, "t3");
        assert_eq!(threads[2].id, "t1");

        svc.sort_threads(&CommentSortOrder::ByTimestamp);
        let threads = svc.get_all_threads();
        assert_eq!(threads[0].id, "t2");
        assert_eq!(threads[2].id, "t3");

        svc.sort_threads(&CommentSortOrder::ByAuthor);
        let threads = svc.get_all_threads();
        assert_eq!(threads[0].id, "t2"); // alice
        assert_eq!(threads[1].id, "t3"); // bob
        assert_eq!(threads[2].id, "t1"); // carol
    }

    #[test]
    fn test_service_filter_threads() {
        let mut svc = CommentsService::new();
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "alice", "hi", 10));
        t1.resolved = true;
        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(2, "bob", "hey", 20));
        svc.add_thread(t1);
        svc.add_thread(t2);

        let filter = CommentFilter::new().by_resolved(true);
        let result = svc.filter_threads(&filter);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "t1");

        let filter2 = CommentFilter::new().by_author("bob");
        let result2 = svc.filter_threads(&filter2);
        assert_eq!(result2.len(), 1);
        assert_eq!(result2[0].id, "t2");
    }

    #[test]
    fn test_service_threads_for_line_range() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "file.rs", 5));
        svc.add_thread(make_thread("t2", "file.rs", 15));
        svc.add_thread(make_thread("t3", "file.rs", 25));
        svc.add_thread(make_thread("t4", "other.rs", 10));

        let result = svc.threads_for_line_range("file.rs", 10, 20);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "t2");

        let result2 = svc.threads_for_line_range("file.rs", 1, 30);
        assert_eq!(result2.len(), 3);
    }

    #[test]
    fn test_service_all_authors() {
        let mut svc = CommentsService::new();
        let mut t1 = make_thread("t1", "file.rs", 1);
        t1.comments.push(make_comment(1, "alice", "hi", 10));
        t1.comments.push(make_comment(2, "bob", "hey", 20));
        let mut t2 = make_thread("t2", "file.rs", 2);
        t2.comments.push(make_comment(3, "alice", "again", 30));
        t2.comments.push(make_comment(4, "carol", "yo", 40));
        svc.add_thread(t1);
        svc.add_thread(t2);

        let authors = svc.all_authors();
        assert_eq!(authors, vec!["alice", "bob", "carol"]);
    }

    #[test]
    fn test_service_collapse_expand_all() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "a.rs", 1));
        svc.add_thread(make_thread("t2", "b.rs", 2));

        svc.collapse_all();
        assert!(svc.get_thread("t1").unwrap().collapsed);
        assert!(svc.get_thread("t2").unwrap().collapsed);

        svc.expand_all();
        assert!(!svc.get_thread("t1").unwrap().collapsed);
        assert!(!svc.get_thread("t2").unwrap().collapsed);
    }

    #[test]
    fn test_service_resolve_all() {
        let mut svc = CommentsService::new();
        svc.add_thread(make_thread("t1", "a.rs", 1));
        svc.add_thread(make_thread("t2", "b.rs", 2));
        assert_eq!(svc.unresolved_count(), 2);

        svc.resolve_all();
        assert_eq!(svc.unresolved_count(), 0);
        assert_eq!(svc.resolved_count(), 2);
    }

    #[test]
    fn test_thread_display() {
        let mut thread = make_thread("t1", "file.rs", 1);
        assert_eq!(format!("{}", thread), "Thread t1: 0 comments");
        thread.comments.push(make_comment(1, "alice", "hi", 10));
        thread.comments.push(make_comment(2, "bob", "hey", 20));
        assert_eq!(format!("{}", thread), "Thread t1: 2 comments");
    }

    #[test]
    fn test_reaction_display() {
        let r = CommentReaction {
            label: "👍".into(),
            count: 5,
            has_reacted: true,
        };
        assert_eq!(format!("{}", r), "👍 (5)");
    }

    #[test]
    fn test_comment_partial_eq() {
        let c1 = make_comment(1, "alice", "hi", 10);
        let c2 = make_comment(1, "bob", "different", 99);
        let c3 = make_comment(2, "alice", "hi", 10);
        assert_eq!(c1, c2);
        assert_ne!(c1, c3);
    }

    #[test]
    fn test_search_comments_found() {
        let mut svc = CommentsService::new();
        let mut t1 = make_thread("t1", "file.rs", 1);
        t1.comments.push(make_comment(1, "alice", "fix the bug", 10));
        t1.comments.push(make_comment(2, "bob", "looks good", 20));
        svc.add_thread(t1);
        let results = svc.search_comments("bug");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.author, "alice");
    }

    #[test]
    fn test_search_comments_case_insensitive() {
        let mut svc = CommentsService::new();
        let mut t1 = make_thread("t1", "file.rs", 1);
        t1.comments.push(make_comment(1, "alice", "LGTM", 10));
        svc.add_thread(t1);
        let results = svc.search_comments("lgtm");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_comments_no_match() {
        let mut svc = CommentsService::new();
        let mut t1 = make_thread("t1", "file.rs", 1);
        t1.comments.push(make_comment(1, "alice", "hello", 10));
        svc.add_thread(t1);
        let results = svc.search_comments("zzz_not_found");
        assert!(results.is_empty());
    }

    #[test]
    fn test_compute_statistics() {
        let mut svc = CommentsService::new();
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "alice", "hi", 10));
        t1.comments.push(make_comment(2, "bob", "hey", 20));
        t1.resolved = true;
        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(3, "alice", "yo", 30));
        svc.add_thread(t1);
        svc.add_thread(t2);
        let stats = svc.compute_statistics();
        assert_eq!(stats.total_threads, 2);
        assert_eq!(stats.resolved_threads, 1);
        assert_eq!(stats.unresolved_threads, 1);
        assert_eq!(stats.total_comments, 3);
        assert_eq!(stats.unique_authors, 2);
    }

    #[test]
    fn test_statistics_display() {
        let stats = CommentStatistics {
            total_threads: 5,
            resolved_threads: 3,
            unresolved_threads: 2,
            total_comments: 12,
            total_reactions: 7,
            unique_authors: 4,
        };
        let s = format!("{}", stats);
        assert!(s.contains("threads=5"));
        assert!(s.contains("comments=12"));
    }

    #[test]
    fn test_sort_comments_in_threads() {
        let mut svc = CommentsService::new();
        let mut t1 = make_thread("t1", "file.rs", 1);
        t1.comments.push(make_comment(2, "bob", "second", 200));
        t1.comments.push(make_comment(1, "alice", "first", 100));
        svc.add_thread(t1);
        svc.sort_comments_in_threads();
        let t = svc.get_thread("t1").unwrap();
        assert_eq!(t.comments[0].timestamp, 100);
        assert_eq!(t.comments[1].timestamp, 200);
    }

    #[test]
    fn test_resolution_event_struct() {
        let evt = ResolutionEvent {
            thread_id: "t1".to_string(),
            resolved: true,
            timestamp: 12345,
            actor: "alice".to_string(),
        };
        assert_eq!(evt.thread_id, "t1");
        assert!(evt.resolved);
    }

    #[test]
    fn test_thread_partial_eq() {
        let t1 = make_thread("t1", "file.rs", 1);
        let t2 = CommentThread {
            id: "t1".to_string(),
            uri: "other.rs".to_string(),
            line: 99,
            comments: Vec::new(),
            resolved: true,
            collapsed: true,
        };
        let t3 = make_thread("t2", "file.rs", 1);
        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
    }

    #[test]
    fn thread_add_reply() {
        let mut t = make_thread("t1", "file.rs", 1);
        t.add_reply("alice", "hello", 100);
        t.add_reply("bob", "world", 200);
        assert_eq!(t.comment_count(), 2);
        assert_eq!(t.comments[0].author, "alice");
        assert_eq!(t.comments[1].author, "bob");
    }

    #[test]
    fn thread_resolve_unresolve() {
        let mut t = make_thread("t1", "file.rs", 1);
        assert!(!t.resolved);
        t.resolve();
        assert!(t.resolved);
        t.unresolve();
        assert!(!t.resolved);
    }

    #[test]
    fn thread_latest_comment() {
        let mut t = make_thread("t1", "file.rs", 1);
        assert!(t.latest_comment().is_none());
        t.add_reply("alice", "first", 10);
        t.add_reply("bob", "second", 20);
        assert_eq!(t.latest_comment().unwrap().body, "second");
    }

    #[test]
    fn thread_authors_unique() {
        let mut t = make_thread("t1", "file.rs", 1);
        t.add_reply("alice", "a", 1);
        t.add_reply("bob", "b", 2);
        t.add_reply("alice", "c", 3);
        let authors = t.authors();
        assert_eq!(authors, vec!["alice", "bob"]);
    }

    #[test]
    fn test_comment_range_overlap() {
        let t1 = make_thread("t1", "f.rs", 5);
        let t2 = make_thread("t2", "f.rs", 10);
        let t3 = make_thread("t3", "f.rs", 15);
        let threads: Vec<&CommentThread> = vec![&t1, &t2, &t3];
        let result = comment_range_overlap(&threads, 5, 10);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "t1");
        assert_eq!(result[1].id, "t2");
    }

    #[test]
    fn test_comment_range_overlap_empty() {
        let t1 = make_thread("t1", "f.rs", 1);
        let threads: Vec<&CommentThread> = vec![&t1];
        let result = comment_range_overlap(&threads, 10, 20);
        assert!(result.is_empty());
    }

    #[test]
    fn controller_add_and_get() {
        let mut ctrl = CommentController::new();
        ctrl.add_thread("a.rs", make_thread("t1", "a.rs", 1));
        ctrl.add_thread("a.rs", make_thread("t2", "a.rs", 5));
        ctrl.add_thread("b.rs", make_thread("t3", "b.rs", 1));
        assert_eq!(ctrl.get_threads("a.rs").len(), 2);
        assert_eq!(ctrl.get_threads("b.rs").len(), 1);
        assert!(ctrl.get_threads("c.rs").is_empty());
    }

    #[test]
    fn controller_remove_thread() {
        let mut ctrl = CommentController::new();
        ctrl.add_thread("a.rs", make_thread("t1", "a.rs", 1));
        ctrl.add_thread("a.rs", make_thread("t2", "a.rs", 2));
        assert!(ctrl.remove_thread("a.rs", "t1"));
        assert_eq!(ctrl.thread_count("a.rs"), 1);
        assert!(!ctrl.remove_thread("a.rs", "t99"));
        assert!(!ctrl.remove_thread("missing.rs", "t1"));
    }

    #[test]
    fn controller_all_uris() {
        let mut ctrl = CommentController::new();
        ctrl.add_thread("a.rs", make_thread("t1", "a.rs", 1));
        ctrl.add_thread("b.rs", make_thread("t2", "b.rs", 1));
        let mut uris = ctrl.all_uris();
        uris.sort();
        assert_eq!(uris, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn controller_resolve_all_and_unresolved_count() {
        let mut ctrl = CommentController::new();
        ctrl.add_thread("a.rs", make_thread("t1", "a.rs", 1));
        ctrl.add_thread("a.rs", make_thread("t2", "a.rs", 2));
        assert_eq!(ctrl.unresolved_count("a.rs"), 2);
        ctrl.resolve_all("a.rs");
        assert_eq!(ctrl.unresolved_count("a.rs"), 0);
        assert_eq!(ctrl.unresolved_count("missing.rs"), 0);
    }

    #[test]
    fn formatter_markdown_basic() {
        let mut thread = make_thread("t1", "main.rs", 42);
        thread.comments.push(make_comment(1, "alice", "LGTM", 100));
        let md = CommentFormatter::format_as_markdown(&thread);
        assert!(md.contains("## Thread t1"));
        assert!(md.contains("alice"));
        assert!(md.contains("LGTM"));
        assert!(md.contains("main.rs"));
        assert!(md.contains("Unresolved"));
    }

    #[test]
    fn formatter_markdown_resolved_with_reactions() {
        let mut thread = make_thread("t2", "lib.rs", 10);
        thread.resolved = true;
        thread.comments.push(make_comment_with_reactions(
            1,
            "bob",
            vec![CommentReaction { label: "👍".into(), count: 3, has_reacted: false }],
        ));
        let md = CommentFormatter::format_as_markdown(&thread);
        assert!(md.contains("Resolved"));
        assert!(md.contains("👍 ×3"));
    }

    #[test]
    fn formatter_plain_text() {
        let mut thread = make_thread("t1", "main.rs", 5);
        thread.comments.push(make_comment(1, "carol", "Fix this", 200));
        let plain = CommentFormatter::format_as_plain(&thread);
        assert!(plain.contains("Thread t1 [Unresolved]"));
        assert!(plain.contains("carol: Fix this"));
    }

    #[test]
    fn formatter_summary() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "a", "c1", 1));
        t1.comments.push(make_comment(2, "b", "c2", 2));
        t1.resolved = true;

        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(3, "c", "c3", 3));

        let mut t3 = make_thread("t3", "c.rs", 3);
        t3.resolved = true;

        let summary = CommentFormatter::format_summary(&[t1, t2, t3]);
        assert_eq!(summary, "3 threads, 2 resolved, 3 total comments");
    }

    #[test]
    fn search_by_author() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "alice", "hi", 1));
        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(2, "bob", "bye", 2));

        let threads = [t1, t2];
        let results = CommentSearch::new().with_author("alice").search(&threads);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "t1");
    }

    #[test]
    fn search_by_body_and_resolved() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "alice", "TODO fix this", 1));
        t1.resolved = true;

        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(2, "bob", "todo later", 2));

        let threads = [t1, t2];
        let results = CommentSearch::new()
            .with_body_contains("todo")
            .with_resolved(false)
            .search(&threads);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "t2");
    }

    #[test]
    fn search_no_filters_returns_all() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "x", "a", 1));
        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(2, "y", "b", 2));

        let threads = [t1, t2];
        let results = CommentSearch::new().search(&threads);
        assert_eq!(results.len(), 2);
    }

    // ── Comment age classification ────────────────────────────────

    #[test]
    fn classify_age_recent() {
        let now = 100_000;
        assert_eq!(classify_comment_age(now - 60, now), CommentAge::Recent);
        assert_eq!(classify_comment_age(now - 3599, now), CommentAge::Recent);
    }

    #[test]
    fn classify_age_today() {
        let now = 100_000;
        assert_eq!(classify_comment_age(now - 3600, now), CommentAge::Today);
        assert_eq!(classify_comment_age(now - 86399, now), CommentAge::Today);
    }

    #[test]
    fn classify_age_this_week() {
        let now = 700_000;
        assert_eq!(classify_comment_age(now - 86400, now), CommentAge::ThisWeek);
        assert_eq!(classify_comment_age(now - 604799, now), CommentAge::ThisWeek);
    }

    #[test]
    fn classify_age_older() {
        let now = 1_000_000;
        assert_eq!(classify_comment_age(now - 604800, now), CommentAge::Older);
    }

    // ── Author statistics ─────────────────────────────────────────

    #[test]
    fn gather_author_stats_counts() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "alice", "hi", 1));
        t1.comments.push(make_comment(2, "alice", "fix", 2));
        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(3, "bob", "ok", 3));

        let stats = gather_author_stats(&[t1, t2]);
        assert_eq!(stats.len(), 2);
        let alice = stats.iter().find(|s| s.author == "alice").unwrap();
        assert_eq!(alice.comment_count, 2);
        assert_eq!(alice.thread_count, 1);
        let bob = stats.iter().find(|s| s.author == "bob").unwrap();
        assert_eq!(bob.comment_count, 1);
    }

    // ── Thread grouping ───────────────────────────────────────────

    #[test]
    fn group_threads_by_uri_sorts_desc() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "x", "a", 1));
        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(2, "x", "a", 1));
        let mut t3 = make_thread("t3", "a.rs", 5);
        t3.comments.push(make_comment(3, "x", "a", 1));

        let groups = group_threads_by_uri(&[t1, t2, t3]);
        assert_eq!(groups[0], ("a.rs".to_string(), 2));
        assert_eq!(groups[1], ("b.rs".to_string(), 1));
    }

    // ── Recent activity ───────────────────────────────────────────

    #[test]
    fn threads_with_recent_activity_filters() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "x", "old", 100));
        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(2, "y", "new", 500));

        let threads = [t1, t2];
        let recent = threads_with_recent_activity(&threads, 400);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, "t2");
    }

    #[test]
    fn comment_age_display() {
        assert_eq!(format!("{}", CommentAge::Recent), "Recent");
        assert_eq!(format!("{}", CommentAge::Older), "Older");
    }

    // ── Nested comments depth tracking ────────────────────────────

    #[test]
    fn compute_comment_depths_builds_tree() {
        let mut comments = vec![
            NestedComment { id: 1, parent_id: None, author: "a".into(), body: "root".into(), timestamp: 1, depth: 0 },
            NestedComment { id: 2, parent_id: Some(1), author: "b".into(), body: "reply".into(), timestamp: 2, depth: 0 },
            NestedComment { id: 3, parent_id: Some(2), author: "c".into(), body: "deep".into(), timestamp: 3, depth: 0 },
            NestedComment { id: 4, parent_id: Some(1), author: "d".into(), body: "reply2".into(), timestamp: 4, depth: 0 },
        ];
        compute_comment_depths(&mut comments);
        assert_eq!(comments[0].depth, 0);
        assert_eq!(comments[1].depth, 1);
        assert_eq!(comments[2].depth, 2);
        assert_eq!(comments[3].depth, 1);

        let kids = children_of(&comments, 1);
        assert_eq!(kids.len(), 2);
    }

    // ── Date-range filtering ──────────────────────────────────────

    #[test]
    fn threads_in_date_range_filters_correctly() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "x", "early", 100));
        let mut t2 = make_thread("t2", "a.rs", 2);
        t2.comments.push(make_comment(2, "y", "mid", 500));
        let mut t3 = make_thread("t3", "a.rs", 3);
        t3.comments.push(make_comment(3, "z", "late", 900));

        let threads = [t1, t2, t3];
        let result = threads_in_date_range(&threads, 200, 600);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "t2");
    }

    // ── Batch operations ──────────────────────────────────────────

    #[test]
    fn batch_resolve_and_delete_by_author() {
        let mut svc = CommentsService::new();
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "alice", "fix", 10));
        let mut t2 = make_thread("t2", "b.rs", 2);
        t2.comments.push(make_comment(2, "bob", "bug", 20));
        let mut t3 = make_thread("t3", "c.rs", 3);
        t3.comments.push(make_comment(3, "alice", "nit", 30));
        svc.add_thread(t1);
        svc.add_thread(t2);
        svc.add_thread(t3);

        assert_eq!(svc.resolve_all_by_author("alice"), 2);
        assert_eq!(svc.resolved_count(), 2);

        assert_eq!(svc.delete_comments_by_author("bob"), 1);
        assert_eq!(svc.total_comment_count(), 2);

        assert_eq!(svc.remove_threads_by_author("alice"), 2);
        assert_eq!(svc.thread_count(), 1);
    }

    // ── Diff tracking ─────────────────────────────────────────────

    #[test]
    fn threads_on_and_outside_changed_lines() {
        let t1 = CommentThread { id: "t1".into(), uri: "src/main.rs".into(), line: 5, comments: vec![], resolved: false, collapsed: false };
        let t2 = CommentThread { id: "t2".into(), uri: "src/main.rs".into(), line: 15, comments: vec![], resolved: false, collapsed: false };
        let t3 = CommentThread { id: "t3".into(), uri: "src/main.rs".into(), line: 25, comments: vec![], resolved: false, collapsed: false };
        let threads = [t1, t2, t3];
        let hunks = vec![DiffHunk { start_line: 10, end_line: 20 }];

        let on = threads_on_changed_lines(&threads, "src/main.rs", &hunks);
        assert_eq!(on.len(), 1);
        assert_eq!(on[0].id, "t2");

        let off = threads_outside_changed_lines(&threads, "src/main.rs", &hunks);
        assert_eq!(off.len(), 2);
    }

    // ── Multi-thread markdown export ──────────────────────────────

    #[test]
    fn export_threads_as_markdown_includes_header() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "alice", "looks good", 100));
        t1.resolved = true;
        let mut t2 = make_thread("t2", "b.rs", 5);
        t2.comments.push(make_comment(2, "bob", "needs work", 200));

        let md = CommentFormatter::export_threads_as_markdown(&[t1, t2]);
        assert!(md.starts_with("# Code Review Comments"));
        assert!(md.contains("**Total threads:** 2"));
        assert!(md.contains("**Resolved:** 1"));
        assert!(md.contains("**Unresolved:** 1"));
        assert!(md.contains("alice"));
        assert!(md.contains("needs work"));
    }

    // ── Draft management ──────────────────────────────────────────

    #[test]
    fn draft_manager_add_update_submit() {
        let mut mgr = DraftManager::new();
        let idx = mgr.add_draft(CommentDraft {
            thread_id: Some("t1".into()),
            uri: "main.rs".into(),
            line: 10,
            author: "alice".into(),
            body: "initial draft".into(),
            created_at: 100,
            modified_at: 100,
        });
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.drafts_for_uri("main.rs").len(), 1);
        assert_eq!(mgr.drafts_by_author("alice").len(), 1);
        assert!(mgr.drafts_by_author("bob").is_empty());

        // Update the draft body
        assert!(mgr.update_draft(idx, "revised draft", 200));
        assert_eq!(mgr.drafts_for_uri("main.rs")[0].body, "revised draft");
        assert_eq!(mgr.drafts_for_uri("main.rs")[0].modified_at, 200);
        // Invalid index
        assert!(!mgr.update_draft(99, "nope", 300));

        // Submit draft into a thread
        let mut thread = make_thread("t1", "main.rs", 10);
        assert!(mgr.submit_draft(0, &mut thread, 500));
        assert_eq!(mgr.count(), 0);
        assert_eq!(thread.comments.len(), 1);
        assert_eq!(thread.comments[0].author, "alice");
        assert_eq!(thread.comments[0].body, "revised draft");
        assert_eq!(thread.comments[0].timestamp, 500);

        // Submit on empty manager fails
        assert!(!mgr.submit_draft(0, &mut thread, 600));
    }

    #[test]
    fn draft_manager_remove_and_clear() {
        let mut mgr = DraftManager::new();
        mgr.add_draft(CommentDraft {
            thread_id: None,
            uri: "a.rs".into(),
            line: 1,
            author: "bob".into(),
            body: "d1".into(),
            created_at: 1,
            modified_at: 1,
        });
        mgr.add_draft(CommentDraft {
            thread_id: None,
            uri: "b.rs".into(),
            line: 2,
            author: "carol".into(),
            body: "d2".into(),
            created_at: 2,
            modified_at: 2,
        });
        assert_eq!(mgr.count(), 2);
        let removed = mgr.remove_draft(0);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().author, "bob");
        assert_eq!(mgr.count(), 1);
        assert!(mgr.remove_draft(99).is_none());
        mgr.clear();
        assert_eq!(mgr.count(), 0);
    }

    // ── Thread navigation ─────────────────────────────────────────

    #[test]
    fn thread_navigator_next_prev_unresolved() {
        let t1 = make_thread("t1", "f.rs", 5);
        let mut t2 = make_thread("t2", "f.rs", 15);
        t2.resolved = true;
        let t3 = make_thread("t3", "f.rs", 25);
        let t4 = make_thread("t4", "f.rs", 35);
        let threads = [t1, t2, t3, t4];

        // Next after line 10 should skip resolved t2 and land on t3
        let next = ThreadNavigator::next_unresolved(&threads, "f.rs", 10);
        assert_eq!(next.unwrap().id, "t3");

        // Next after line 30 should find t4
        let next2 = ThreadNavigator::next_unresolved(&threads, "f.rs", 30);
        assert_eq!(next2.unwrap().id, "t4");

        // Next after line 40 wraps around to t1
        let wrap = ThreadNavigator::next_unresolved(&threads, "f.rs", 40);
        assert_eq!(wrap.unwrap().id, "t1");

        // Prev before line 30 should find t1 (skipping resolved t2)
        let prev = ThreadNavigator::prev_unresolved(&threads, "f.rs", 20);
        assert_eq!(prev.unwrap().id, "t1");

        // Prev before line 5 wraps around to t4
        let wrap_prev = ThreadNavigator::prev_unresolved(&threads, "f.rs", 3);
        assert_eq!(wrap_prev.unwrap().id, "t4");

        assert_eq!(ThreadNavigator::unresolved_remaining(&threads, "f.rs"), 3);
        assert_eq!(ThreadNavigator::unresolved_remaining(&threads, "other.rs"), 0);
    }

    // ── Comment text helpers ──────────────────────────────────────

    #[test]
    fn comment_text_extract_mentions() {
        let mentions = CommentText::extract_mentions("Hey @alice and @bob, please review @alice");
        assert_eq!(mentions, vec!["alice", "bob"]);

        let m2 = CommentText::extract_mentions("@carol, can you look?");
        assert_eq!(m2, vec!["carol"]);

        let m3 = CommentText::extract_mentions("no mentions here");
        assert!(m3.is_empty());
    }

    #[test]
    fn comment_text_truncate_and_word_count() {
        assert_eq!(CommentText::truncate("hello world", 20), "hello world");
        assert_eq!(CommentText::truncate("hello world", 5), "hello…");

        assert_eq!(CommentText::word_count("one two three"), 3);
        assert_eq!(CommentText::word_count(""), 0);

        assert!(CommentText::has_code_block("see ```rust\nfn main()```"));
        assert!(!CommentText::has_code_block("plain text"));
    }

    // ── File comment summaries ────────────────────────────────────

    #[test]
    fn build_file_summaries_groups_by_uri() {
        let mut t1 = make_thread("t1", "a.rs", 1);
        t1.comments.push(make_comment(1, "alice", "c1", 10));
        t1.resolved = true;

        let mut t2 = make_thread("t2", "a.rs", 5);
        t2.comments.push(make_comment(2, "bob", "c2", 20));

        let mut t3 = make_thread("t3", "b.rs", 1);
        t3.comments.push(make_comment(3, "alice", "c3", 30));
        t3.comments.push(make_comment(4, "carol", "c4", 40));

        let summaries = build_file_summaries(&[t1, t2, t3]);
        assert_eq!(summaries.len(), 2);

        // b.rs has 1 unresolved, a.rs has 1 unresolved — sorted by unresolved desc, then name
        let a = summaries.iter().find(|s| s.uri == "a.rs").unwrap();
        assert_eq!(a.total_threads, 2);
        assert_eq!(a.resolved_threads, 1);
        assert_eq!(a.unresolved_threads, 1);
        assert_eq!(a.total_comments, 2);
        assert_eq!(a.unique_authors, 2);

        let b = summaries.iter().find(|s| s.uri == "b.rs").unwrap();
        assert_eq!(b.total_threads, 1);
        assert_eq!(b.total_comments, 2);
        assert_eq!(b.unique_authors, 2);

        // Display impl
        let display = format!("{}", a);
        assert!(display.contains("a.rs"));
        assert!(display.contains("2 threads"));
    }

    #[test] fn commentsTreeCollapser_new() { let s = CommentsTreeCollapser::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn commentsTreeCollapser_add() { let mut s = CommentsTreeCollapser::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn commentsTreeCollapser_remove() { let mut s = CommentsTreeCollapser::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn commentsTreeCollapser_config() { let mut s = CommentsTreeCollapser::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn commentsTreeCollapser_nav() { let mut s = CommentsTreeCollapser::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn commentsTreeCollapser_filter() { let mut s = CommentsTreeCollapser::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn commentsTreeCollapser_display() { assert!(format!("{}", CommentsTreeCollapser::new()).contains("CommentsTreeCollapser")); }
    #[test] fn commentSortOptions_new() { let s = CommentSortOptions::new(); assert!(s.is_empty()); }
    #[test] fn commentSortOptions_add() { let mut s = CommentSortOptions::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn commentSortOptions_active() { let mut s = CommentSortOptions::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn commentSortOptions_error() { let mut s = CommentSortOptions::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn commentSortOptions_rm_group() { let mut s = CommentSortOptions::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn commentSortOptions_display() { assert!(format!("{}", CommentSortOptions::new()).contains("CommentSortOptions")); }


    #[test] fn commentsTreeCollapser_snap_capture() {
        let s = CommentsTreeCollapser::new();
        let snap = CommentsTreeCollapserSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn commentsTreeCollapser_snap_stale() {
        let s = CommentsTreeCollapser::new();
        let snap = CommentsTreeCollapserSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn commentsTreeCollapser_snap_diff() {
        let s = CommentsTreeCollapser::new();
        let s1v = CommentsTreeCollapserSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn commentsTreeCollapser_snap_display() {
        let s = CommentsTreeCollapser::new();
        let snap = CommentsTreeCollapserSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn commentSortOptions_stats_record() {
        let mut st = CommentSortOptionsStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn commentSortOptions_stats_hit_ratio() {
        let mut st = CommentSortOptionsStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn commentSortOptions_stats_merge() {
        let mut a = CommentSortOptionsStats::new();
        a.total_adds = 5;
        let mut b = CommentSortOptionsStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn commentSortOptions_stats_display() {
        let st = CommentSortOptionsStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn commentsTreeCollapser_config_default() {
        let c = CommentsTreeCollapserConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn commentsTreeCollapser_config_builder() {
        let c = CommentsTreeCollapserConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn commentsTreeCollapser_config_labels() {
        let mut c = CommentsTreeCollapserConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn commentsTreeCollapser_config_cleanup_threshold() {
        let c = CommentsTreeCollapserConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn commentsTreeCollapser_config_display() {
        assert!(format!("{}", CommentsTreeCollapserConfig::new()).contains("Config"));
    }
    #[test] fn commentSortOptions_stats_peaks() {
        let mut st = CommentSortOptionsStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- CommentThreadGroup --------------------------------------------------

    #[test]
    fn thread_group_add_and_query() {
        let mut group = CommentThreadGroup::new();
        group.add(make_thread("t1", "file:///a.rs", 10));
        group.add(make_thread("t2", "file:///a.rs", 20));
        group.add(make_thread("t3", "file:///b.rs", 5));
        assert_eq!(group.len(), 3);
        assert_eq!(group.threads_for_file("file:///a.rs").len(), 2);
    }

    #[test]
    fn thread_group_files_with_threads() {
        let mut group = CommentThreadGroup::new();
        group.add(make_thread("t1", "file:///x.rs", 1));
        group.add(make_thread("t2", "file:///y.rs", 1));
        let files = group.files_with_threads();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn thread_group_resolved_counts() {
        let mut group = CommentThreadGroup::new();
        let mut t = make_thread("t1", "f", 1);
        t.resolved = true;
        group.add(t);
        group.add(make_thread("t2", "f", 2));
        assert_eq!(group.resolved_count(), 1);
        assert_eq!(group.unresolved_count(), 1);
    }

    #[test]
    fn thread_group_total_comments() {
        let mut t = make_thread("t1", "f", 1);
        t.add_reply("alice", "hello", 100);
        t.add_reply("bob", "world", 200);
        let group = CommentThreadGroup::from_threads(vec![t]);
        assert_eq!(group.total_comments(), 2);
    }

    #[test]
    fn thread_group_unique_authors() {
        let mut t = make_thread("t1", "f", 1);
        t.add_reply("alice", "hi", 100);
        t.add_reply("bob", "hey", 200);
        t.add_reply("alice", "again", 300);
        let group = CommentThreadGroup::from_threads(vec![t]);
        assert_eq!(group.unique_authors().len(), 2);
    }

    // -- CommentMentionExtractor ----------------------------------------------

    #[test]
    fn mention_extraction_basic() {
        let mentions = CommentMentionExtractor::extract_mentions("Hey @alice and @bob");
        assert_eq!(mentions, vec!["alice", "bob"]);
    }

    #[test]
    fn mention_extraction_no_mentions() {
        let mentions = CommentMentionExtractor::extract_mentions("no mentions here");
        assert!(mentions.is_empty());
    }

    #[test]
    fn mention_extraction_at_sign_alone() {
        let mentions = CommentMentionExtractor::extract_mentions("@ alone");
        assert!(mentions.is_empty());
    }

    #[test]
    fn mention_is_mentioned_in_thread() {
        let mut t = make_thread("t1", "f", 1);
        t.add_reply("admin", "cc @reviewer", 100);
        assert!(CommentMentionExtractor::is_mentioned(&t, "reviewer"));
        assert!(!CommentMentionExtractor::is_mentioned(&t, "nobody"));
    }

    // -- CommentActivityTracker -----------------------------------------------

    #[test]
    fn activity_tracker_records_and_queries() {
        let mut at = CommentActivityTracker::new();
        at.record("alice", 100);
        at.record("bob", 200);
        at.record("alice", 300);
        assert_eq!(at.total_events(), 3);
        assert_eq!(at.events_in_range(150, 250).len(), 1);
    }

    #[test]
    fn activity_tracker_most_active() {
        let mut at = CommentActivityTracker::new();
        at.record("alice", 100);
        at.record("alice", 200);
        at.record("bob", 300);
        assert_eq!(at.most_active_author(), Some("alice".into()));
    }

    #[test]
    fn activity_tracker_empty() {
        let at = CommentActivityTracker::new();
        assert_eq!(at.most_active_author(), None);
        assert_eq!(at.total_events(), 0);
    }


    #[test]
    fn comments_view_config_new() {
        let cfg = CommentsViewConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn comments_view_config_set_get() {
        let mut cfg = CommentsViewConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn comments_view_config_remove() {
        let mut cfg = CommentsViewConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn comments_view_config_keys_sorted() {
        let mut cfg = CommentsViewConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn comments_view_config_bump_version() {
        let mut cfg = CommentsViewConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn comments_view_config_clear() {
        let mut cfg = CommentsViewConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn comments_view_config_merge() {
        let mut cfg1 = CommentsViewConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = CommentsViewConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn comments_view_config_disable() {
        let mut cfg = CommentsViewConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn comments_view_rate_tracker_empty() {
        let rt = CommentsViewRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn comments_view_rate_tracker_record() {
        let mut rt = CommentsViewRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn comments_view_rate_tracker_prune() {
        let mut rt = CommentsViewRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn comments_view_validator_valid() {
        let v = CommentsViewValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn comments_view_validator_errors() {
        let mut v = CommentsViewValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn comments_view_validator_clear() {
        let mut v = CommentsViewValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn comments_view_validator_merge() {
        let mut v1 = CommentsViewValidator::new();
        v1.add_error("e1");
        let mut v2 = CommentsViewValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn comments_view_rate_tracker_clear() {
        let mut rt = CommentsViewRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yr_metrics_empty() {
        let m = YrMetrics::new("cmt_view");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yr_metrics_record_and_mean() {
        let mut m = YrMetrics::new("cmt_view");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yr_metrics_min_max() {
        let mut m = YrMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yr_metrics_variance_and_std() {
        let mut m = YrMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yr_metrics_percentile() {
        let mut m = YrMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yr_metrics_merge() {
        let mut a = YrMetrics::new("a");
        a.record(1.0);
        let mut b = YrMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yr_metrics_reset() {
        let mut m = YrMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yr_rate_window_empty() {
        let rw = YrRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yr_rate_window_tick_and_rate() {
        let mut rw = YrRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yr_lru_cache_basic() {
        let mut c = YrLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yr_lru_cache_contains_and_keys() {
        let mut c = YrLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yr_lru_cache_remove() {
        let mut c = YrLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yr_metrics_sum() {
        let mut m = YrMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yr_metrics_label() {
        let m = YrMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yr_lru_cache_clear() {
        let mut c = YrLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for comments_view
    #[test]
    fn xa_comments_view_ring_new() {
        let rb = super::XaCommentsViewRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_comments_view_ring_push_len() {
        let mut rb = super::XaCommentsViewRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_comments_view_ring_wrap() {
        let mut rb = super::XaCommentsViewRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_comments_view_ring_mean_empty() {
        let rb = super::XaCommentsViewRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_comments_view_ring_mean_values() {
        let mut rb = super::XaCommentsViewRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_comments_view_ring_min_max() {
        let mut rb = super::XaCommentsViewRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_comments_view_ring_iter() {
        let mut rb = super::XaCommentsViewRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_comments_view_counter_new() {
        let c = super::XaCommentsViewCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_comments_view_counter_inc() {
        let mut c = super::XaCommentsViewCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_comments_view_counter_inc_by() {
        let mut c = super::XaCommentsViewCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_comments_view_counter_reset() {
        let mut c = super::XaCommentsViewCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_comments_view_counter_clear() {
        let mut c = super::XaCommentsViewCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_comments_view_counter_default() {
        let c = super::XaCommentsViewCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 21 ----

    #[test]
    fn xc_21_pool_new_empty() {
        let pool: super::Xc21Pool<i32> = super::Xc21Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_21_pool_release_acquire() {
        let mut pool = super::Xc21Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_21_pool_acquire_empty() {
        let mut pool: super::Xc21Pool<i32> = super::Xc21Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_21_pool_full() {
        let mut pool = super::Xc21Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_21_pool_drain() {
        let mut pool = super::Xc21Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_21_pool_stats() {
        let mut pool = super::Xc21Pool::new(8);
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
    fn xc_21_pool_clear() {
        let mut pool = super::Xc21Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_21_pool_shrink() {
        let mut pool = super::Xc21Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_21_pool_default() {
        let pool: super::Xc21Pool<String> = super::Xc21Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_21_pool_extend() {
        let mut pool = super::Xc21Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_21_pool_retain() {
        let mut pool = super::Xc21Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_21_scheduler_round_robin() {
        let mut sched = super::Xc21Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_21_scheduler_empty() {
        let mut sched = super::Xc21Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_21_scheduler_reset() {
        let mut sched = super::Xc21Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_21_scheduler_add_remove() {
        let mut sched = super::Xc21Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_21_scheduler_targets() {
        let sched = super::Xc21Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_21_hash_empty() {
        assert_eq!(super::xc_21_hash(b""), 5381);
    }

    #[test]
    fn xc_21_hash_data() {
        let h = super::xc_21_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_21_hash(b"hello"), h);
    }

    #[test]
    fn xc_21_reverse_str() {
        assert_eq!(super::xc_21_reverse("abc"), "cba");
        assert_eq!(super::xc_21_reverse(""), "");
    }


    // --- xd_102 deepening tests ---

    #[test]
    fn xd_102_sm_initial_state() {
        let sm = Xd102StateMachine::new();
        assert_eq!(sm.current_state(), Xd102State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_102_sm_valid_idle_to_running() {
        let mut sm = Xd102StateMachine::new();
        assert!(sm.transition(Xd102State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd102State::Running);
    }

    #[test]
    fn xd_102_sm_valid_running_to_paused() {
        let mut sm = Xd102StateMachine::new();
        sm.transition(Xd102State::Running).unwrap();
        assert!(sm.transition(Xd102State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd102State::Paused);
    }

    #[test]
    fn xd_102_sm_valid_running_to_done() {
        let mut sm = Xd102StateMachine::new();
        sm.transition(Xd102State::Running).unwrap();
        assert!(sm.transition(Xd102State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd102State::Done);
    }

    #[test]
    fn xd_102_sm_valid_paused_to_running() {
        let mut sm = Xd102StateMachine::new();
        sm.transition(Xd102State::Running).unwrap();
        sm.transition(Xd102State::Paused).unwrap();
        assert!(sm.transition(Xd102State::Running).is_ok());
    }

    #[test]
    fn xd_102_sm_valid_done_to_idle() {
        let mut sm = Xd102StateMachine::new();
        sm.transition(Xd102State::Running).unwrap();
        sm.transition(Xd102State::Done).unwrap();
        assert!(sm.transition(Xd102State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd102State::Idle);
    }

    #[test]
    fn xd_102_sm_invalid_idle_to_done() {
        let mut sm = Xd102StateMachine::new();
        assert!(sm.transition(Xd102State::Done).is_err());
    }

    #[test]
    fn xd_102_sm_invalid_idle_to_paused() {
        let mut sm = Xd102StateMachine::new();
        assert!(sm.transition(Xd102State::Paused).is_err());
    }

    #[test]
    fn xd_102_sm_history_tracking() {
        let mut sm = Xd102StateMachine::new();
        sm.transition(Xd102State::Running).unwrap();
        sm.transition(Xd102State::Paused).unwrap();
        sm.transition(Xd102State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd102State::Idle);
        assert_eq!(sm.history()[0].to, Xd102State::Running);
        assert_eq!(sm.history()[1].from, Xd102State::Running);
        assert_eq!(sm.history()[2].to, Xd102State::Done);
    }

    #[test]
    fn xd_102_sm_serialize_deserialize() {
        let mut sm = Xd102StateMachine::new();
        sm.transition(Xd102State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd102StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd102State::Running));
    }

    #[test]
    fn xd_102_sm_deserialize_invalid() {
        assert_eq!(Xd102StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_102_sm_reset() {
        let mut sm = Xd102StateMachine::new();
        sm.transition(Xd102State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd102State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_102_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd102EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd102Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_102_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd102EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd102Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd102Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_102_bus_unsubscribe() {
        let mut bus = Xd102EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_102_event_kind_and_payload() {
        let e = Xd102Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd102Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_102_bus_clear_history() {
        let mut bus = Xd102EventBus::new();
        bus.publish(Xd102Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_102_sm_step_counter_increments() {
        let mut sm = Xd102StateMachine::new();
        sm.transition(Xd102State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd102State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_26 graph tests ------------------------------------------------

    #[test]
    fn xg_26_graph_empty() {
        let g = super::Xg26Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_26_graph_add_node() {
        let mut g = super::Xg26Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_26_graph_add_edge() {
        let mut g = super::Xg26Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_26_graph_neighbors() {
        let mut g = super::Xg26Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_26_graph_has_path() {
        let mut g = super::Xg26Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_26_graph_self_path() {
        let g = super::Xg26Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_26_graph_topo_sort() {
        let mut g = super::Xg26Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_26_graph_cycle_detect_false() {
        let mut g = super::Xg26Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_26_graph_cycle_detect_true() {
        let mut g = super::Xg26Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_26 heap tests -------------------------------------------------

    #[test]
    fn xg_26_heap_empty() {
        let h: super::Xg26Heap<i32> = super::Xg26Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_26_heap_push_pop() {
        let mut h = super::Xg26Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_26_heap_peek() {
        let mut h = super::Xg26Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_26_heap_drain_sorted() {
        let mut h = super::Xg26Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_26_heap_merge() {
        let mut a = super::Xg26Heap::new();
        let mut b = super::Xg26Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_26_heap_default() {
        let h: super::Xg26Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_26_graph_default() {
        let g: super::Xg26Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }

}
