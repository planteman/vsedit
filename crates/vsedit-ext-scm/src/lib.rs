//! Ext API: Source control.
//!
//! RPC bridge between the extension host and the main thread for scm.

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_scm";

/// Initialize the scm extension API bridge.
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
