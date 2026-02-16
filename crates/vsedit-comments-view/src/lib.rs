//! Comments view (code review comments).

#[derive(Debug, Clone)]
pub struct Comment {
    pub id: u64,
    pub author: String,
    pub body: String,
    pub timestamp: u64,
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
}
