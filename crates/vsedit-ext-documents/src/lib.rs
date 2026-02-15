//! Ext API: Documents.
//!
//! RPC bridge between the extension host and the main thread for documents.

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_documents";

/// Initialize the documents extension API bridge.
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
