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

}
