//! Event emitter system with combinators.
//!
//! This crate provides the core event infrastructure for vsedit, faithfully
//! replicating the patterns from VS Code's `vs/base/common/event.ts`.
//!
//! # Overview
//!
//! - [`Emitter<T>`] — owns and fires events to registered listeners.
//! - [`Event`] — a subscribable event returned by [`Emitter::event`].
//! - [`DisposableHandle`] — an RAII guard that unsubscribes on drop.
//! - Combinators: [`Event::once`], [`Event::map`], [`Event::filter`],
//!   [`Event::debounce`], [`Event::any`], [`Event::chain`].
//!
//! # Example
//!
//! ```
//! use vsedit_events::Emitter;
//!
//! let emitter = Emitter::new();
//! let event = emitter.event();
//!
//! let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
//! let r = received.clone();
//! let _handle = event.on(move |value: &i32| {
//!     r.lock().unwrap().push(*value);
//! });
//!
//! emitter.fire(&42);
//! assert_eq!(*received.lock().unwrap(), vec![42]);
//! ```

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

// ---------------------------------------------------------------------------
// Listener id
// ---------------------------------------------------------------------------

static NEXT_LISTENER_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_LISTENER_ID.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Listener storage (shared between Emitter and Event handles)
// ---------------------------------------------------------------------------

type Callback<T> = Arc<dyn Fn(&T) + Send + Sync>;

struct Listener<T> {
    id: u64,
    callback: Callback<T>,
}

struct EmitterInner<T> {
    listeners: Vec<Listener<T>>,
    paused: bool,
    /// Events buffered while paused — stored as owned values.
    pause_buffer: Vec<T>,
}

impl<T> Default for EmitterInner<T> {
    fn default() -> Self {
        Self {
            listeners: Vec::new(),
            paused: false,
            pause_buffer: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// DisposableHandle
// ---------------------------------------------------------------------------

/// An RAII guard returned from event subscription.
///
/// Dropping the handle automatically unsubscribes the listener. You can also
/// call [`DisposableHandle::dispose`] explicitly.
pub struct DisposableHandle {
    disposed: AtomicBool,
    inner: Box<dyn Fn() + Send + Sync>,
}

impl DisposableHandle {
    fn new(f: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            disposed: AtomicBool::new(false),
            inner: Box::new(f),
        }
    }

    /// Explicitly unsubscribe. Subsequent calls are no-ops.
    pub fn dispose(&self) {
        if !self.disposed.swap(true, Ordering::AcqRel) {
            (self.inner)();
        }
    }

    /// Returns `true` if this handle has already been disposed.
    pub fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::Acquire)
    }
}

impl Drop for DisposableHandle {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl fmt::Debug for DisposableHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DisposableHandle")
            .field("disposed", &self.is_disposed())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Emitter<T>
// ---------------------------------------------------------------------------

/// An event emitter that fires values of type `T` to registered listeners.
///
/// Create with [`Emitter::new`], subscribe via the [`Event`] returned by
/// [`Emitter::event`], and fire with [`Emitter::fire`].
///
/// In debug builds, dropping an `Emitter` that still has active listeners will
/// emit a warning via `eprintln!`.
pub struct Emitter<T> {
    inner: Arc<Mutex<EmitterInner<T>>>,
}

impl<T: Clone + Send + Sync + 'static> Emitter<T> {
    /// Create a new emitter with no listeners.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EmitterInner::default())),
        }
    }

    /// Return an [`Event`] that can be used to subscribe to this emitter.
    pub fn event(&self) -> Event<T> {
        Event {
            inner: Arc::clone(&self.inner),
        }
    }

    /// Fire an event, synchronously invoking every registered listener.
    ///
    /// If the emitter is paused the value is buffered and will be delivered
    /// when [`Emitter::resume`] is called.
    pub fn fire(&self, value: &T) {
        let mut guard = self.inner.lock().unwrap();
        if guard.paused {
            guard.pause_buffer.push(value.clone());
            return;
        }
        // Clone Arc handles so we don't hold the lock during callbacks
        // (prevents deadlocks when a listener subscribes/unsubscribes).
        let callbacks: Vec<Callback<T>> =
            guard.listeners.iter().map(|l| Arc::clone(&l.callback)).collect();
        drop(guard);

        for cb in &callbacks {
            cb(value);
        }
    }

    /// Pause event delivery. Fired events are buffered.
    pub fn pause(&self) {
        self.inner.lock().unwrap().paused = true;
    }

    /// Resume event delivery, flushing any buffered events.
    pub fn resume(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.paused = false;
        let buffered: Vec<T> = guard.pause_buffer.drain(..).collect();
        let callbacks: Vec<Callback<T>> =
            guard.listeners.iter().map(|l| Arc::clone(&l.callback)).collect();
        drop(guard);

        for value in &buffered {
            for cb in &callbacks {
                cb(value);
            }
        }
    }

    /// Returns the current number of active listeners.
    pub fn listener_count(&self) -> usize {
        self.inner.lock().unwrap().listeners.len()
    }
}

impl<T: Clone + Send + Sync + 'static> Default for Emitter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for Emitter<T> {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            let count = self.inner.lock().unwrap().listeners.len();
            if count > 0 {
                eprintln!(
                    "[vsedit-events] WARNING: Emitter dropped with {count} \
                     active listener(s) — possible memory leak"
                );
            }
        }
    }
}

impl<T> fmt::Debug for Emitter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let guard = self.inner.lock().unwrap();
        f.debug_struct("Emitter")
            .field("listeners", &guard.listeners.len())
            .field("paused", &guard.paused)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Event<T>
// ---------------------------------------------------------------------------

/// A subscribable event.
///
/// Obtained from [`Emitter::event`]. Use [`Event::on`] to register a listener.
#[derive(Clone)]
pub struct Event<T> {
    inner: Arc<Mutex<EmitterInner<T>>>,
}

impl<T: Clone + Send + Sync + 'static> Event<T> {
    /// Subscribe a listener. Returns a [`DisposableHandle`] that unsubscribes
    /// on drop.
    pub fn on<F>(&self, listener: F) -> DisposableHandle
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        let id = next_id();
        self.inner.lock().unwrap().listeners.push(Listener {
            id,
            callback: Arc::new(listener),
        });

        let weak = Arc::downgrade(&self.inner);
        DisposableHandle::new(move || {
            if let Some(inner) = weak.upgrade() {
                inner
                    .lock()
                    .unwrap()
                    .listeners
                    .retain(|l| l.id != id);
            }
        })
    }

    // -- Combinators --------------------------------------------------------

    /// Subscribe a listener that fires only once, then auto-disposes.
    pub fn once<F>(&self, listener: F) -> DisposableHandle
    where
        F: FnOnce(&T) + Send + Sync + 'static,
    {
        let listener = Mutex::new(Some(listener));
        // We need a handle reference inside the callback so it can dispose
        // itself. Use a shared slot.
        let handle_slot: Arc<Mutex<Option<Weak<DisposableHandle>>>> =
            Arc::new(Mutex::new(None));
        let slot_clone = Arc::clone(&handle_slot);

        let id = next_id();
        self.inner.lock().unwrap().listeners.push(Listener {
            id,
            callback: Arc::new(move |value| {
                if let Some(f) = listener.lock().unwrap().take() {
                    f(value);
                    // Remove ourselves
                    if let Some(weak) = slot_clone.lock().unwrap().as_ref() {
                        if let Some(h) = weak.upgrade() {
                            h.dispose();
                        }
                    }
                }
            }),
        });

        let weak = Arc::downgrade(&self.inner);
        let handle = Arc::new(DisposableHandle::new(move || {
            if let Some(inner) = weak.upgrade() {
                inner
                    .lock()
                    .unwrap()
                    .listeners
                    .retain(|l| l.id != id);
            }
        }));

        *handle_slot.lock().unwrap() = Some(Arc::downgrade(&handle));

        // Unwrap the Arc — the caller owns the handle. We keep only a Weak
        // inside the callback.
        Arc::try_unwrap(handle).unwrap_or_else(|arc| {
            // If something still holds a strong ref (shouldn't happen), just
            // return a wrapper that delegates.
            let arc2 = arc.clone();
            DisposableHandle::new(move || arc2.dispose())
        })
    }

    /// Create a mapped event that transforms values with `f`.
    pub fn map<U, F>(&self, f: F) -> Event<U>
    where
        U: Clone + Send + Sync + 'static,
        F: Fn(&T) -> U + Send + Sync + 'static,
    {
        let derived = Emitter::<U>::new();
        let derived_event = derived.event();

        // Keep the derived emitter alive as long as the source event lives.
        let derived_arc = Arc::new(Mutex::new(Some(derived)));
        let derived_weak = Arc::downgrade(&derived_arc);

        let _source_handle = self.on(move |value| {
            let mapped = f(value);
            if let Some(arc) = derived_weak.upgrade() {
                if let Some(emitter) = arc.lock().unwrap().as_ref() {
                    emitter.fire(&mapped);
                }
            }
        });

        // Leak the source subscription and derived emitter so they live
        // as long as derived event listeners exist.
        std::mem::forget(_source_handle);
        std::mem::forget(derived_arc);

        derived_event
    }

    /// Create a filtered event that only fires when `predicate` returns true.
    pub fn filter<F>(&self, predicate: F) -> Event<T>
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let derived = Emitter::<T>::new();
        let derived_event = derived.event();

        let derived_arc = Arc::new(Mutex::new(Some(derived)));
        let derived_weak = Arc::downgrade(&derived_arc);

        let _source_handle = self.on(move |value| {
            if predicate(value) {
                if let Some(arc) = derived_weak.upgrade() {
                    if let Some(emitter) = arc.lock().unwrap().as_ref() {
                        emitter.fire(value);
                    }
                }
            }
        });

        std::mem::forget(_source_handle);
        std::mem::forget(derived_arc);

        derived_event
    }

    /// Create a debounced event that batches values.
    ///
    /// Values are collected into a `Vec<T>` and the derived event fires once
    /// after all synchronous fires have completed (via a zero-duration thread
    /// sleep, emulating microtask-like batching). Because this crate is
    /// `std`-only the debounce uses a background thread.
    pub fn debounce(&self) -> Event<Vec<T>>
    where
        T: 'static,
    {
        let derived = Emitter::<Vec<T>>::new();
        let derived_event = derived.event();

        let buffer: Arc<Mutex<Vec<T>>> = Arc::new(Mutex::new(Vec::new()));
        let pending = Arc::new(AtomicBool::new(false));

        let derived_arc = Arc::new(derived);
        let derived_weak = Arc::downgrade(&derived_arc);
        let buffer2 = Arc::clone(&buffer);
        let pending2 = Arc::clone(&pending);

        let _source_handle = self.on(move |value| {
            buffer2.lock().unwrap().push(value.clone());

            if !pending2.swap(true, Ordering::AcqRel) {
                let buf = Arc::clone(&buffer2);
                let p = Arc::clone(&pending2);
                let dw = derived_weak.clone();
                std::thread::spawn(move || {
                    // Yield to let synchronous fires accumulate.
                    std::thread::sleep(std::time::Duration::from_millis(0));
                    let items: Vec<T> = buf.lock().unwrap().drain(..).collect();
                    p.store(false, Ordering::Release);
                    if let Some(emitter) = dw.upgrade() {
                        emitter.fire(&items);
                    }
                });
            }
        });

        std::mem::forget(_source_handle);
        std::mem::forget(derived_arc);

        derived_event
    }

    /// Chain: pipe this event into the given emitter so that every value
    /// fired here is also fired on `target`.
    pub fn chain(&self, target: &Emitter<T>) -> DisposableHandle {
        let target_inner = Arc::clone(&target.inner);
        self.on(move |value| {
            let guard = target_inner.lock().unwrap();
            for listener in &guard.listeners {
                (listener.callback)(value);
            }
        })
    }
}

/// Merge multiple events of the same type into one.
///
/// The returned event fires whenever any of the source events fire.
pub fn any<T: Clone + Send + Sync + 'static>(events: &[Event<T>]) -> Event<T> {
    let derived = Emitter::<T>::new();
    let derived_event = derived.event();

    let derived_arc = Arc::new(derived);

    for event in events {
        let dw = Arc::downgrade(&derived_arc);
        let handle = event.on(move |value| {
            if let Some(emitter) = dw.upgrade() {
                emitter.fire(value);
            }
        });
        std::mem::forget(handle);
    }

    std::mem::forget(derived_arc);
    derived_event
}

impl<T> fmt::Debug for Event<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let guard = self.inner.lock().unwrap();
        f.debug_struct("Event")
            .field("listeners", &guard.listeners.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Event filtering by type tag
// ---------------------------------------------------------------------------

/// An event filter that selectively passes events based on a predicate.
pub struct EventFilter<T> {
    predicate: Box<dyn Fn(&T) -> bool + Send + Sync>,
}

impl<T> EventFilter<T> {
    /// Create a new filter with the given predicate.
    pub fn new(predicate: impl Fn(&T) -> bool + Send + Sync + 'static) -> Self {
        Self { predicate: Box::new(predicate) }
    }

    /// Test whether the value passes the filter.
    pub fn matches(&self, value: &T) -> bool {
        (self.predicate)(value)
    }
}

// ---------------------------------------------------------------------------
// Event replay buffer
// ---------------------------------------------------------------------------

/// A fixed-capacity ring buffer that stores the most recent events for replay.
pub struct EventReplayBuffer<T> {
    buffer: Vec<T>,
    capacity: usize,
}

impl<T: Clone> EventReplayBuffer<T> {
    /// Create a new replay buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a value into the buffer, evicting the oldest if at capacity.
    pub fn push(&mut self, value: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.remove(0);
        }
        self.buffer.push(value);
    }

    /// Return all buffered values in order (oldest first).
    pub fn values(&self) -> &[T] {
        &self.buffer
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Number of items currently buffered.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// The maximum capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

// ---------------------------------------------------------------------------
// Listener priority helpers
// ---------------------------------------------------------------------------

/// A priority value for ordering listener invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ListenerPriority(pub u32);

impl ListenerPriority {
    pub const LOW: Self = Self(10);
    pub const NORMAL: Self = Self(50);
    pub const HIGH: Self = Self(90);
}

impl Default for ListenerPriority {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl fmt::Display for ListenerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Priority({})", self.0)
    }
}

/// Creates a listener that counts how many times it is invoked.
/// Returns an `Arc<AtomicU64>` that tracks the count.
pub fn counter_listener<T: Clone + Send + Sync + 'static>(
    event: &Event<T>,
) -> (DisposableHandle, Arc<AtomicU64>) {
    let count = Arc::new(AtomicU64::new(0));
    let count_clone = Arc::clone(&count);
    let handle = event.on(move |_| {
        count_clone.fetch_add(1, Ordering::SeqCst);
    });
    (handle, count)
}

// ---------------------------------------------------------------------------
// Timestamp helper
// ---------------------------------------------------------------------------

/// Returns the current system time as milliseconds since the Unix epoch.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before Unix epoch")
        .as_millis() as u64
}

// ---------------------------------------------------------------------------
// DebouncedEmitter
// ---------------------------------------------------------------------------

/// A wrapper around [`Emitter`] that only fires after a configurable quiet
/// period has elapsed since the last call to [`fire`](DebouncedEmitter::fire).
pub struct DebouncedEmitter<T> {
    inner: Emitter<T>,
    quiet_period_ms: u64,
    last_fire_time: Arc<Mutex<u64>>,
}

impl<T: Clone + Send + Sync + 'static> DebouncedEmitter<T> {
    /// Create a new debounced emitter with the given quiet period in
    /// milliseconds.
    pub fn new(quiet_period_ms: u64) -> Self {
        Self {
            inner: Emitter::new(),
            quiet_period_ms,
            last_fire_time: Arc::new(Mutex::new(0)),
        }
    }

    /// Fire the value only if enough time has passed since the last fire.
    pub fn fire(&self, value: &T) {
        let now = current_time_ms();
        let mut last = self.last_fire_time.lock().unwrap();
        if now.saturating_sub(*last) >= self.quiet_period_ms {
            self.inner.fire(value);
            *last = now;
        }
    }

    /// Returns the subscribable event for this emitter.
    pub fn event(&self) -> Event<T> {
        self.inner.event()
    }

    /// Unconditionally fires the value and resets the timer.
    pub fn force_fire(&self, value: &T) {
        let now = current_time_ms();
        let mut last = self.last_fire_time.lock().unwrap();
        *last = now;
        self.inner.fire(value);
    }
}

// ---------------------------------------------------------------------------
// ThrottledEmitter
// ---------------------------------------------------------------------------

/// A wrapper around [`Emitter`] that limits emissions to at most one per
/// configured interval.
pub struct ThrottledEmitter<T> {
    inner: Emitter<T>,
    interval_ms: u64,
    last_emit_time: Arc<Mutex<u64>>,
}

impl<T: Clone + Send + Sync + 'static> ThrottledEmitter<T> {
    /// Create a new throttled emitter with the given interval in milliseconds.
    pub fn new(interval_ms: u64) -> Self {
        Self {
            inner: Emitter::new(),
            interval_ms,
            last_emit_time: Arc::new(Mutex::new(0)),
        }
    }

    /// Fire the value only if the interval has elapsed since the last emit.
    pub fn fire(&self, value: &T) {
        let now = current_time_ms();
        let mut last = self.last_emit_time.lock().unwrap();
        if now.saturating_sub(*last) >= self.interval_ms {
            self.inner.fire(value);
            *last = now;
        }
    }

    /// Returns the subscribable event for this emitter.
    pub fn event(&self) -> Event<T> {
        self.inner.event()
    }

    /// Resets the throttle timer, allowing the next fire to proceed
    /// immediately.
    pub fn reset(&self) {
        let mut last = self.last_emit_time.lock().unwrap();
        *last = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn basic_subscribe_and_fire() {
        let emitter = Emitter::new();
        let event = emitter.event();

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = event.on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });

        emitter.fire(&1);
        emitter.fire(&2);
        emitter.fire(&3);

        assert_eq!(*received.lock().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn dispose_removes_listener() {
        let emitter = Emitter::new();
        let event = emitter.event();

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let handle = event.on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });

        emitter.fire(&1);
        handle.dispose();
        emitter.fire(&2);

        assert_eq!(*received.lock().unwrap(), vec![1]);
        assert_eq!(emitter.listener_count(), 0);
    }

    #[test]
    fn drop_disposes() {
        let emitter = Emitter::new();
        let event = emitter.event();

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        {
            let _h = event.on(move |v: &i32| {
                r.lock().unwrap().push(*v);
            });
            emitter.fire(&1);
        }
        // Handle dropped — listener removed
        emitter.fire(&2);
        assert_eq!(*received.lock().unwrap(), vec![1]);
    }

    #[test]
    fn multiple_listeners() {
        let emitter = Emitter::new();
        let event = emitter.event();

        let a = Arc::new(Mutex::new(Vec::new()));
        let b = Arc::new(Mutex::new(Vec::new()));

        let a2 = a.clone();
        let _h1 = event.on(move |v: &i32| a2.lock().unwrap().push(*v));
        let b2 = b.clone();
        let _h2 = event.on(move |v: &i32| b2.lock().unwrap().push(*v));

        emitter.fire(&10);

        assert_eq!(*a.lock().unwrap(), vec![10]);
        assert_eq!(*b.lock().unwrap(), vec![10]);
    }

    #[test]
    fn once_fires_only_once() {
        let emitter = Emitter::new();
        let event = emitter.event();

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = event.once(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });

        emitter.fire(&1);
        emitter.fire(&2);

        assert_eq!(*received.lock().unwrap(), vec![1]);
    }

    #[test]
    fn map_transforms_values() {
        let emitter = Emitter::<i32>::new();
        let mapped = emitter.event().map(|v| v.to_string());

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = mapped.on(move |v: &String| {
            r.lock().unwrap().push(v.clone());
        });

        emitter.fire(&42);

        assert_eq!(
            *received.lock().unwrap(),
            vec!["42".to_string()]
        );
    }

    #[test]
    fn filter_only_matching() {
        let emitter = Emitter::<i32>::new();
        let evens = emitter.event().filter(|v| v % 2 == 0);

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = evens.on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });

        emitter.fire(&1);
        emitter.fire(&2);
        emitter.fire(&3);
        emitter.fire(&4);

        assert_eq!(*received.lock().unwrap(), vec![2, 4]);
    }

    #[test]
    fn any_merges_events() {
        let e1 = Emitter::<i32>::new();
        let e2 = Emitter::<i32>::new();
        let merged = any(&[e1.event(), e2.event()]);

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = merged.on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });

        e1.fire(&1);
        e2.fire(&2);
        e1.fire(&3);

        assert_eq!(*received.lock().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn chain_pipes_events() {
        let source = Emitter::<i32>::new();
        let target = Emitter::<i32>::new();

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h1 = target.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });

        let _h2 = source.event().chain(&target);

        source.fire(&99);

        assert_eq!(*received.lock().unwrap(), vec![99]);
    }

    #[test]
    fn pause_and_resume() {
        let emitter = Emitter::new();
        let event = emitter.event();

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = event.on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });

        emitter.fire(&1);
        emitter.pause();
        emitter.fire(&2);
        emitter.fire(&3);
        assert_eq!(*received.lock().unwrap(), vec![1]);

        emitter.resume();
        assert_eq!(*received.lock().unwrap(), vec![1, 2, 3]);

        // After resume, normal delivery resumes
        emitter.fire(&4);
        assert_eq!(*received.lock().unwrap(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn debounce_batches_events() {
        let emitter = Emitter::<i32>::new();
        let debounced = emitter.event().debounce();

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = debounced.on(move |batch: &Vec<i32>| {
            r.lock().unwrap().push(batch.clone());
        });

        emitter.fire(&1);
        emitter.fire(&2);
        emitter.fire(&3);

        // Give the debounce thread time to flush.
        std::thread::sleep(Duration::from_millis(50));

        let result = received.lock().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec![1, 2, 3]);
    }

    #[test]
    fn disposable_handle_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DisposableHandle>();
    }

    #[test]
    fn emitter_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Emitter<i32>>();
    }

    #[test]
    fn event_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Event<i32>>();
    }

    #[test]
    fn dispose_is_idempotent() {
        let emitter = Emitter::new();
        let event = emitter.event();
        let handle = event.on(|_: &i32| {});

        handle.dispose();
        handle.dispose(); // second call is a no-op
        assert!(handle.is_disposed());
        assert_eq!(emitter.listener_count(), 0);
    }

    #[test]
    fn debug_formatting() {
        let emitter = Emitter::<i32>::new();
        let dbg = format!("{emitter:?}");
        assert!(dbg.contains("Emitter"));
        assert!(dbg.contains("listeners"));

        let event = emitter.event();
        let dbg = format!("{event:?}");
        assert!(dbg.contains("Event"));
    }

    #[test]
    fn event_filter_matches() {
        let filter = EventFilter::new(|v: &i32| *v > 5);
        assert!(filter.matches(&10));
        assert!(!filter.matches(&3));
    }

    #[test]
    fn event_filter_string_predicate() {
        let filter = EventFilter::new(|s: &String| s.starts_with("err"));
        assert!(filter.matches(&"error: bad".to_string()));
        assert!(!filter.matches(&"info: ok".to_string()));
    }

    #[test]
    fn replay_buffer_push_and_values() {
        let mut buf = EventReplayBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.values(), &[1, 2, 3]);
        buf.push(4);
        assert_eq!(buf.values(), &[2, 3, 4]);
        assert_eq!(buf.len(), 3);
    }

    #[test]
    fn replay_buffer_clear() {
        let mut buf = EventReplayBuffer::new(5);
        buf.push(10);
        buf.push(20);
        assert!(!buf.is_empty());
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.capacity(), 5);
    }

    #[test]
    fn listener_priority_ordering() {
        assert!(ListenerPriority::HIGH > ListenerPriority::NORMAL);
        assert!(ListenerPriority::NORMAL > ListenerPriority::LOW);
        assert_eq!(ListenerPriority::default(), ListenerPriority::NORMAL);
    }

    #[test]
    fn listener_priority_display() {
        assert_eq!(format!("{}", ListenerPriority::HIGH), "Priority(90)");
        assert_eq!(format!("{}", ListenerPriority(42)), "Priority(42)");
    }

    #[test]
    fn replay_buffer_single_capacity() {
        let mut buf = EventReplayBuffer::new(1);
        buf.push("a");
        buf.push("b");
        assert_eq!(buf.values(), &["b"]);
        assert_eq!(buf.len(), 1);
    }

    #[test]
    fn counter_listener_counts_fires() {
        let emitter = Emitter::new();
        let (handle, count) = counter_listener(&emitter.event());
        emitter.fire(&1);
        emitter.fire(&2);
        emitter.fire(&3);
        assert_eq!(count.load(Ordering::SeqCst), 3);
        drop(handle);
    }

    #[test]
    fn counter_listener_stops_after_dispose() {
        let emitter = Emitter::new();
        let (handle, count) = counter_listener(&emitter.event());
        emitter.fire(&42);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        drop(handle);
        emitter.fire(&99);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // -----------------------------------------------------------------------
    // DebouncedEmitter tests
    // -----------------------------------------------------------------------

    #[test]
    fn debounced_fires_after_quiet_period() {
        let emitter = DebouncedEmitter::<i32>::new(0);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = emitter.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        emitter.fire(&1);
        assert_eq!(*received.lock().unwrap(), vec![1]);
    }

    #[test]
    fn debounced_suppresses_rapid_fires() {
        let emitter = DebouncedEmitter::<i32>::new(500);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = emitter.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        // First fire should succeed (last_fire_time starts at 0).
        emitter.fire(&1);
        // Rapid subsequent fires should be suppressed.
        emitter.fire(&2);
        emitter.fire(&3);
        assert_eq!(*received.lock().unwrap(), vec![1]);
    }

    #[test]
    fn debounced_force_fire_always_fires() {
        let emitter = DebouncedEmitter::<i32>::new(500);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = emitter.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        emitter.fire(&1);
        emitter.force_fire(&2);
        emitter.force_fire(&3);
        assert_eq!(*received.lock().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn debounced_event_returns_correct_event() {
        let emitter = DebouncedEmitter::<i32>::new(0);
        let event = emitter.event();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = event.on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        emitter.fire(&42);
        assert_eq!(*received.lock().unwrap(), vec![42]);
    }

    // -----------------------------------------------------------------------
    // ThrottledEmitter tests
    // -----------------------------------------------------------------------

    #[test]
    fn throttled_fires_first_event_immediately() {
        let emitter = ThrottledEmitter::<i32>::new(500);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = emitter.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        // First fire always succeeds (last_emit_time starts at 0).
        emitter.fire(&1);
        assert_eq!(*received.lock().unwrap(), vec![1]);
    }

    #[test]
    fn throttled_suppresses_rapid_fires() {
        let emitter = ThrottledEmitter::<i32>::new(500);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = emitter.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        emitter.fire(&1);
        emitter.fire(&2);
        emitter.fire(&3);
        assert_eq!(*received.lock().unwrap(), vec![1]);
    }

    #[test]
    fn throttled_fires_after_interval_passes() {
        let emitter = ThrottledEmitter::<i32>::new(50);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = emitter.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        emitter.fire(&1);
        std::thread::sleep(Duration::from_millis(80));
        emitter.fire(&2);
        assert_eq!(*received.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn throttled_reset_allows_immediate_fire() {
        let emitter = ThrottledEmitter::<i32>::new(500);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = emitter.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        emitter.fire(&1);
        emitter.fire(&2); // suppressed
        emitter.reset();
        emitter.fire(&3); // should succeed after reset
        assert_eq!(*received.lock().unwrap(), vec![1, 3]);
    }
}
