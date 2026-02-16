//! Network utilities.

// ---------------------------------------------------------------------------
// HTTP method
// ---------------------------------------------------------------------------

/// HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

// ---------------------------------------------------------------------------
// HTTP request / response
// ---------------------------------------------------------------------------

/// An outgoing HTTP request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

/// An incoming HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Returns `true` when the status code is in the 2xx range.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Returns `true` when the status code is in the 3xx range.
    pub fn is_redirect(&self) -> bool {
        (300..400).contains(&self.status)
    }

    /// Interprets the body as a UTF-8 string.
    pub fn body_as_string(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.clone())
    }
}

// ---------------------------------------------------------------------------
// Proxy configuration
// ---------------------------------------------------------------------------

/// Proxy settings for outbound requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

// ---------------------------------------------------------------------------
// Network service
// ---------------------------------------------------------------------------

/// High-level network service.
#[derive(Debug, Clone)]
pub struct NetworkService {
    proxy: Option<ProxyConfig>,
}

impl NetworkService {
    pub fn new() -> Self {
        Self { proxy: None }
    }

    /// Configures a proxy for subsequent requests.
    pub fn set_proxy(&mut self, config: ProxyConfig) {
        self.proxy = Some(config);
    }

    /// Creates a new [`HttpRequest`] with the given method and URL.
    pub fn create_request(&self, method: HttpMethod, url: impl Into<String>) -> HttpRequest {
        HttpRequest {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    /// Stub: returns `true` indicating network connectivity.
    pub fn is_online(&self) -> bool {
        true
    }
}

impl Default for NetworkService {
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

    #[test]
    fn create_request_defaults() {
        let svc = NetworkService::new();
        let req = svc.create_request(HttpMethod::Get, "https://example.com");
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.url, "https://example.com");
        assert!(req.headers.is_empty());
        assert!(req.body.is_none());
    }

    #[test]
    fn response_status_helpers() {
        let ok = HttpResponse { status: 200, headers: vec![], body: vec![] };
        assert!(ok.is_success());
        assert!(!ok.is_redirect());

        let redirect = HttpResponse { status: 301, headers: vec![], body: vec![] };
        assert!(!redirect.is_success());
        assert!(redirect.is_redirect());

        let err = HttpResponse { status: 404, headers: vec![], body: vec![] };
        assert!(!err.is_success());
        assert!(!err.is_redirect());
    }

    #[test]
    fn response_body_as_string() {
        let resp = HttpResponse {
            status: 200,
            headers: vec![],
            body: b"hello world".to_vec(),
        };
        assert_eq!(resp.body_as_string().unwrap(), "hello world");
    }

    #[test]
    fn set_proxy() {
        let mut svc = NetworkService::new();
        assert!(svc.proxy.is_none());
        svc.set_proxy(ProxyConfig {
            host: "proxy.local".into(),
            port: 8080,
            username: None,
            password: None,
        });
        assert_eq!(svc.proxy.as_ref().unwrap().port, 8080);
    }

    #[test]
    fn is_online_stub() {
        let svc = NetworkService::new();
        assert!(svc.is_online());
    }
}
