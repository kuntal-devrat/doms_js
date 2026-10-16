//! Chrome DevTools Protocol JSON-RPC request dispatcher.

use super::*;

mod events;
mod json;
mod requests;

/// Dispatcher for Chrome DevTools Protocol JSON-RPC requests.
pub struct CdpDispatcher {
    pub session: DebuggerSession,
}

impl CdpDispatcher {
    pub fn new() -> Self {
        Self {
            session: DebuggerSession::new(),
        }
    }
}

impl Default for CdpDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
