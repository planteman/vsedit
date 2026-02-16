//! Comments view (code review comments).

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
}
