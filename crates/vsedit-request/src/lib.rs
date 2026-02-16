//! Cancellable async request service.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RequestError {
    RequestNotFound(RequestId),
    AlreadyCompleted(RequestId),
    InvalidTransition { id: RequestId, from: String, to: String },
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestError::RequestNotFound(id) => write!(f, "request {} not found", id),
            RequestError::AlreadyCompleted(id) => write!(f, "request {} already completed", id),
            RequestError::InvalidTransition { id, from, to } => {
                write!(f, "invalid transition for {}: {} -> {}", id, from, to)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(pub u64);

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequestState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
    Failed(String),
}

impl fmt::Display for RequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestState::Pending => write!(f, "Pending"),
            RequestState::InProgress => write!(f, "InProgress"),
            RequestState::Completed => write!(f, "Completed"),
            RequestState::Cancelled => write!(f, "Cancelled"),
            RequestState::Failed(reason) => write!(f, "Failed({})", reason),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub id: RequestId,
    pub method: String,
    pub state: RequestState,
    pub created_at: u64,
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Request(id={}, method={}, state={})", self.id.0, self.method, self.state)
    }
}

pub struct RequestBuilder {
    method: String,
    created_at: Option<u64>,
}

impl RequestBuilder {
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            created_at: None,
        }
    }

    pub fn created_at(mut self, ts: u64) -> Self {
        self.created_at = Some(ts);
        self
    }

    pub fn build(self, service: &mut RequestService) -> RequestId {
        let id = RequestId(service.next_id);
        service.next_id += 1;
        service.requests.push(Request {
            id,
            method: self.method,
            state: RequestState::Pending,
            created_at: self.created_at.unwrap_or(0),
        });
        id
    }
}

pub struct RequestService {
    requests: Vec<Request>,
    next_id: u64,
}

impl RequestService {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_request(&mut self, method: impl Into<String>) -> RequestId {
        let id = RequestId(self.next_id);
        self.next_id += 1;
        self.requests.push(Request {
            id,
            method: method.into(),
            state: RequestState::Pending,
            created_at: 0,
        });
        id
    }

    fn set_state(&mut self, id: RequestId, state: RequestState) {
        if let Some(req) = self.requests.iter_mut().find(|r| r.id == id) {
            req.state = state;
        }
    }

    pub fn start(&mut self, id: RequestId) {
        self.set_state(id, RequestState::InProgress);
    }

    pub fn complete(&mut self, id: RequestId) {
        self.set_state(id, RequestState::Completed);
    }

    pub fn cancel(&mut self, id: RequestId) {
        self.set_state(id, RequestState::Cancelled);
    }

    pub fn fail(&mut self, id: RequestId, reason: impl Into<String>) {
        self.set_state(id, RequestState::Failed(reason.into()));
    }

    pub fn get_state(&self, id: RequestId) -> Option<&RequestState> {
        self.requests.iter().find(|r| r.id == id).map(|r| &r.state)
    }

    pub fn pending_count(&self) -> usize {
        self.requests
            .iter()
            .filter(|r| r.state == RequestState::Pending)
            .count()
    }

    pub fn cancel_all(&mut self) {
        for req in &mut self.requests {
            if matches!(req.state, RequestState::Pending | RequestState::InProgress) {
                req.state = RequestState::Cancelled;
            }
        }
    }

    pub fn get_request(&self, id: RequestId) -> Option<&Request> {
        self.requests.iter().find(|r| r.id == id)
    }

    pub fn try_cancel(&mut self, id: RequestId) -> Result<(), RequestError> {
        let req = self.requests.iter_mut().find(|r| r.id == id)
            .ok_or(RequestError::RequestNotFound(id))?;
        if req.state == RequestState::Completed {
            return Err(RequestError::AlreadyCompleted(id));
        }
        req.state = RequestState::Cancelled;
        Ok(())
    }

    pub fn in_progress_count(&self) -> usize {
        self.requests.iter().filter(|r| r.state == RequestState::InProgress).count()
    }

    pub fn completed_count(&self) -> usize {
        self.requests.iter().filter(|r| r.state == RequestState::Completed).count()
    }

    pub fn list_by_state(&self, state: &RequestState) -> Vec<&Request> {
        self.requests.iter().filter(|r| r.state == *state).collect()
    }

    pub fn remove_completed(&mut self) {
        self.requests.retain(|r| {
            !matches!(r.state, RequestState::Completed | RequestState::Cancelled | RequestState::Failed(_))
        });
    }

    pub fn total_count(&self) -> usize {
        self.requests.len()
    }
}

impl Default for RequestService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_lifecycle() {
        let mut svc = RequestService::new();
        let id = svc.create_request("GET /api");
        assert_eq!(svc.get_state(id), Some(&RequestState::Pending));
        svc.start(id);
        assert_eq!(svc.get_state(id), Some(&RequestState::InProgress));
        svc.complete(id);
        assert_eq!(svc.get_state(id), Some(&RequestState::Completed));
    }

    #[test]
    fn cancel_and_fail() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("POST /data");
        let id2 = svc.create_request("PUT /data");
        svc.cancel(id1);
        svc.fail(id2, "timeout");
        assert_eq!(svc.get_state(id1), Some(&RequestState::Cancelled));
        assert_eq!(
            svc.get_state(id2),
            Some(&RequestState::Failed("timeout".into()))
        );
    }

    #[test]
    fn pending_count_and_cancel_all() {
        let mut svc = RequestService::new();
        svc.create_request("a");
        svc.create_request("b");
        let id3 = svc.create_request("c");
        svc.start(id3);
        assert_eq!(svc.pending_count(), 2);
        svc.cancel_all();
        assert_eq!(svc.pending_count(), 0);
    }

    #[test]
    fn get_request_returns_full_request() {
        let mut svc = RequestService::new();
        let id = svc.create_request("GET /users");
        let req = svc.get_request(id).unwrap();
        assert_eq!(req.method, "GET /users");
        assert_eq!(req.state, RequestState::Pending);
        assert!(svc.get_request(RequestId(999)).is_none());
    }

    #[test]
    fn try_cancel_already_completed() {
        let mut svc = RequestService::new();
        let id = svc.create_request("POST /submit");
        svc.start(id);
        svc.complete(id);
        let err = svc.try_cancel(id).unwrap_err();
        assert_eq!(err, RequestError::AlreadyCompleted(id));
    }

    #[test]
    fn try_cancel_not_found() {
        let mut svc = RequestService::new();
        let err = svc.try_cancel(RequestId(42)).unwrap_err();
        assert_eq!(err, RequestError::RequestNotFound(RequestId(42)));
    }

    #[test]
    fn try_cancel_success() {
        let mut svc = RequestService::new();
        let id = svc.create_request("DELETE /item");
        svc.start(id);
        assert!(svc.try_cancel(id).is_ok());
        assert_eq!(svc.get_state(id), Some(&RequestState::Cancelled));
    }

    #[test]
    fn in_progress_and_completed_counts() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        let id3 = svc.create_request("c");
        svc.start(id1);
        svc.start(id2);
        svc.start(id3);
        svc.complete(id3);
        assert_eq!(svc.in_progress_count(), 2);
        assert_eq!(svc.completed_count(), 1);
    }

    #[test]
    fn list_by_state_filters_correctly() {
        let mut svc = RequestService::new();
        svc.create_request("a");
        let id2 = svc.create_request("b");
        svc.create_request("c");
        svc.start(id2);
        let pending = svc.list_by_state(&RequestState::Pending);
        assert_eq!(pending.len(), 2);
        let in_progress = svc.list_by_state(&RequestState::InProgress);
        assert_eq!(in_progress.len(), 1);
        assert_eq!(in_progress[0].method, "b");
    }

    #[test]
    fn remove_completed_cleans_terminal_states() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        let id3 = svc.create_request("c");
        let id4 = svc.create_request("d");
        svc.start(id1);
        svc.complete(id1);
        svc.cancel(id2);
        svc.fail(id3, "err");
        // id4 stays pending
        assert_eq!(svc.total_count(), 4);
        svc.remove_completed();
        assert_eq!(svc.total_count(), 1);
        assert_eq!(svc.get_request(id4).unwrap().method, "d");
    }

    #[test]
    fn total_count_tracks_all() {
        let mut svc = RequestService::new();
        assert_eq!(svc.total_count(), 0);
        svc.create_request("x");
        svc.create_request("y");
        assert_eq!(svc.total_count(), 2);
    }

    #[test]
    fn display_request_state() {
        assert_eq!(format!("{}", RequestState::Pending), "Pending");
        assert_eq!(format!("{}", RequestState::InProgress), "InProgress");
        assert_eq!(format!("{}", RequestState::Completed), "Completed");
        assert_eq!(format!("{}", RequestState::Cancelled), "Cancelled");
        assert_eq!(format!("{}", RequestState::Failed("oops".into())), "Failed(oops)");
    }

    #[test]
    fn display_request_and_id() {
        let req = Request {
            id: RequestId(7),
            method: "GET /health".into(),
            state: RequestState::Pending,
            created_at: 0,
        };
        assert_eq!(format!("{}", req), "Request(id=7, method=GET /health, state=Pending)");
        assert_eq!(format!("{}", RequestId(42)), "req-42");
    }

    #[test]
    fn builder_with_defaults() {
        let mut svc = RequestService::new();
        let id = RequestBuilder::new("PATCH /item").build(&mut svc);
        let req = svc.get_request(id).unwrap();
        assert_eq!(req.method, "PATCH /item");
        assert_eq!(req.created_at, 0);
    }

    #[test]
    fn builder_with_created_at() {
        let mut svc = RequestService::new();
        let id = RequestBuilder::new("GET /ts")
            .created_at(1700000000)
            .build(&mut svc);
        let req = svc.get_request(id).unwrap();
        assert_eq!(req.created_at, 1700000000);
    }

    #[test]
    fn error_display() {
        let e1 = RequestError::RequestNotFound(RequestId(5));
        assert_eq!(format!("{}", e1), "request req-5 not found");
        let e2 = RequestError::AlreadyCompleted(RequestId(3));
        assert_eq!(format!("{}", e2), "request req-3 already completed");
        let e3 = RequestError::InvalidTransition {
            id: RequestId(1),
            from: "Completed".into(),
            to: "Pending".into(),
        };
        assert_eq!(format!("{}", e3), "invalid transition for req-1: Completed -> Pending");
    }
}
