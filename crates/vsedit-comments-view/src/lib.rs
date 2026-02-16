//! Comments view (code review comments).

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
}
