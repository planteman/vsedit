//! HTTP request service.

/// Core type for request.
pub struct Request {
    _initialized: bool,
}

impl Request {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Request {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Request::new();
        assert!(v._initialized);
    }
}
