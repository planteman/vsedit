//! App startup and shutdown lifecycle.
//!
//! Equivalent to VS Code's `vs/platform/lifecycle/common/lifecycle.ts`.
//! Manages application phases and shutdown confirmation.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use vsedit_events::{Emitter, Event};

/// Application lifecycle phases (in order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LifecyclePhase {
    /// Services are being created.
    Starting = 0,
    /// Workspace storage is ready.
    Ready = 1,
    /// The workbench layout has been restored.
    Restored = 2,
    /// The workbench is fully interactive.
    Eventually = 3,
}

/// Shutdown reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    /// User requested quit.
    Quit,
    /// Window was closed.
    Close,
    /// Reload window.
    Reload,
    /// Process was killed.
    Kill,
}

/// Veto that can prevent shutdown.
#[derive(Clone)]
pub struct WillShutdownEvent {
    pub reason: ShutdownReason,
    vetoed: Arc<Mutex<bool>>,
}

impl WillShutdownEvent {
    pub fn new(reason: ShutdownReason) -> Self {
        Self {
            reason,
            vetoed: Arc::new(Mutex::new(false)),
        }
    }

    /// Veto the shutdown (e.g., unsaved changes).
    pub fn veto(&self) {
        *self.vetoed.lock().unwrap() = true;
    }

    pub fn is_vetoed(&self) -> bool {
        *self.vetoed.lock().unwrap()
    }
}

/// The lifecycle service.
pub struct LifecycleService {
    phase: AtomicU8,
    on_will_shutdown: Emitter<WillShutdownEvent>,
    on_did_shutdown: Emitter<ShutdownReason>,
    on_phase_change: Emitter<LifecyclePhase>,
}

impl LifecycleService {
    pub fn new() -> Self {
        Self {
            phase: AtomicU8::new(LifecyclePhase::Starting as u8),
            on_will_shutdown: Emitter::new(),
            on_did_shutdown: Emitter::new(),
            on_phase_change: Emitter::new(),
        }
    }

    pub fn phase(&self) -> LifecyclePhase {
        match self.phase.load(Ordering::Relaxed) {
            0 => LifecyclePhase::Starting,
            1 => LifecyclePhase::Ready,
            2 => LifecyclePhase::Restored,
            _ => LifecyclePhase::Eventually,
        }
    }

    /// Advance to the next phase. Fires phase change event.
    pub fn set_phase(&self, phase: LifecyclePhase) {
        let current = self.phase.load(Ordering::Relaxed);
        if (phase as u8) > current {
            self.phase.store(phase as u8, Ordering::Relaxed);
            self.on_phase_change.fire(&phase);
        }
    }

    /// Request shutdown. Returns false if vetoed.
    pub fn request_shutdown(&self, reason: ShutdownReason) -> bool {
        let event = WillShutdownEvent::new(reason);
        self.on_will_shutdown.fire(&event);

        if event.is_vetoed() {
            return false;
        }

        self.on_did_shutdown.fire(&reason);
        true
    }

    pub fn on_will_shutdown(&self) -> Event<WillShutdownEvent> {
        self.on_will_shutdown.event()
    }

    pub fn on_did_shutdown(&self) -> Event<ShutdownReason> {
        self.on_did_shutdown.event()
    }

    pub fn on_phase_change(&self) -> Event<LifecyclePhase> {
        self.on_phase_change.event()
    }
}

impl Default for LifecycleService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn initial_phase() {
        let svc = LifecycleService::new();
        assert_eq!(svc.phase(), LifecyclePhase::Starting);
    }

    #[test]
    fn phase_progression() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Ready);
        assert_eq!(svc.phase(), LifecyclePhase::Ready);
        svc.set_phase(LifecyclePhase::Restored);
        assert_eq!(svc.phase(), LifecyclePhase::Restored);
    }

    #[test]
    fn phase_cannot_go_backwards() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Restored);
        svc.set_phase(LifecyclePhase::Ready); // should be ignored
        assert_eq!(svc.phase(), LifecyclePhase::Restored);
    }

    #[test]
    fn shutdown_not_vetoed() {
        let svc = LifecycleService::new();
        let did_shutdown = Arc::new(AtomicBool::new(false));
        let did_shutdown2 = did_shutdown.clone();
        let _sub = svc.on_did_shutdown().on(move |_: &ShutdownReason| {
            did_shutdown2.store(true, Ordering::Relaxed);
        });
        assert!(svc.request_shutdown(ShutdownReason::Quit));
        assert!(did_shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn shutdown_vetoed() {
        let svc = LifecycleService::new();
        let _sub = svc.on_will_shutdown().on(move |evt: &WillShutdownEvent| {
            evt.veto();
        });
        assert!(!svc.request_shutdown(ShutdownReason::Close));
    }
}
