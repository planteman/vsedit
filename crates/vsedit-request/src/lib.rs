//! Cancellable async request service.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub enum RequestState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Request {
    pub id: RequestId,
    pub method: String,
    pub state: RequestState,
    pub created_at: u64,
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
}
