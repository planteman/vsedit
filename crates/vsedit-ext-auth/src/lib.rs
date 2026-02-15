//! Ext API: Authentication.
//!
//! RPC bridge between the extension host and the main thread for auth.

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_auth";

/// Initialize the auth extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }
}
