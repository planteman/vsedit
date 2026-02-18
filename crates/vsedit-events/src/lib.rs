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

use std::collections::HashMap;
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

    /// Return a new filter whose predicate is the logical negation of this one.
    pub fn negate(self) -> EventFilter<T>
    where
        T: 'static,
    {
        let old = self.predicate;
        EventFilter {
            predicate: Box::new(move |v| !old(v)),
        }
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

    /// Whether the buffer has reached its maximum capacity.
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }

    /// The maximum capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Return the most recently pushed value, if any.
    pub fn last(&self) -> Option<&T> {
        self.buffer.last()
    }

    /// Iterate over buffered values from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }
}

impl<T: Clone + fmt::Debug> fmt::Display for EventReplayBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[buffer: {}/{} items]", self.buffer.len(), self.capacity)
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

// ---------------------------------------------------------------------------
// EventCounter
// ---------------------------------------------------------------------------

/// A simple counter that tracks how many events have occurred.
pub struct EventCounter<T> {
    count: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T> EventCounter<T> {
    /// Create a new counter starting at zero.
    pub fn new() -> Self {
        Self {
            count: 0,
            _marker: std::marker::PhantomData,
        }
    }

    /// Increment the counter by one.
    pub fn increment(&mut self) {
        self.count += 1;
    }

    /// Return the current count.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Reset the counter to zero.
    pub fn reset(&mut self) {
        self.count = 0;
    }
}

impl<T> Default for EventCounter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for EventCounter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventCounter")
            .field("count", &self.count)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EventBus — named event channels
// ---------------------------------------------------------------------------

/// A bus that manages named event channels.
///
/// Each channel is identified by a string name and broadcasts values of
/// a single type `T`.
pub struct EventBus<T: Clone + Send + Sync + 'static> {
    channels: Mutex<std::collections::HashMap<String, Arc<Emitter<T>>>>,
}

impl<T: Clone + Send + Sync + 'static> EventBus<T> {
    /// Create a new empty event bus.
    pub fn new() -> Self {
        Self {
            channels: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Get or create a channel by name.
    fn get_or_create(&self, name: &str) -> Arc<Emitter<T>> {
        let mut channels = self.channels.lock().unwrap();
        channels
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Emitter::new()))
            .clone()
    }

    /// Subscribe to a named channel.
    pub fn on<F>(&self, channel: &str, listener: F) -> DisposableHandle
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        let emitter = self.get_or_create(channel);
        emitter.event().on(listener)
    }

    /// Fire a value on a named channel.
    pub fn fire(&self, channel: &str, value: &T) {
        let emitter = self.get_or_create(channel);
        emitter.fire(value);
    }

    /// Return the number of registered channels.
    pub fn channel_count(&self) -> usize {
        self.channels.lock().unwrap().len()
    }

    /// Check if a channel exists.
    pub fn has_channel(&self, name: &str) -> bool {
        self.channels.lock().unwrap().contains_key(name)
    }

    /// List all channel names.
    pub fn channel_names(&self) -> Vec<String> {
        self.channels.lock().unwrap().keys().cloned().collect()
    }

    /// Remove a channel (new subscriptions will create a fresh one).
    pub fn remove_channel(&self, name: &str) {
        self.channels.lock().unwrap().remove(name);
    }
}

impl<T: Clone + Send + Sync + 'static> fmt::Debug for EventBus<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBus")
            .field("channels", &self.channel_count())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EventThrottle — max N fires per time window
// ---------------------------------------------------------------------------

/// Limits event firing to at most `max_fires` in a rolling time window.
pub struct EventThrottle<T: Clone + Send + Sync + 'static> {
    inner: Emitter<T>,
    max_fires: usize,
    window_ms: u64,
    fire_times: Mutex<Vec<u64>>,
}

impl<T: Clone + Send + Sync + 'static> EventThrottle<T> {
    /// Create a new throttle allowing `max_fires` in `window_ms` milliseconds.
    pub fn new(max_fires: usize, window_ms: u64) -> Self {
        Self {
            inner: Emitter::new(),
            max_fires,
            window_ms,
            fire_times: Mutex::new(Vec::new()),
        }
    }

    /// Attempt to fire a value. Returns `true` if the value was emitted,
    /// `false` if throttled.
    pub fn fire(&self, value: &T) -> bool {
        let now = current_time_ms();
        let mut times = self.fire_times.lock().unwrap();
        // Remove expired entries
        let cutoff = now.saturating_sub(self.window_ms);
        times.retain(|&t| t > cutoff);
        if times.len() >= self.max_fires {
            return false;
        }
        times.push(now);
        self.inner.fire(value);
        true
    }

    /// Returns the subscribable event.
    pub fn event(&self) -> Event<T> {
        self.inner.event()
    }

    /// Number of fires in the current window.
    pub fn fires_in_window(&self) -> usize {
        let now = current_time_ms();
        let cutoff = now.saturating_sub(self.window_ms);
        let times = self.fire_times.lock().unwrap();
        times.iter().filter(|&&t| t > cutoff).count()
    }

    /// Reset the throttle state.
    pub fn reset(&self) {
        self.fire_times.lock().unwrap().clear();
    }
}

// ---------------------------------------------------------------------------
// EventPipeline — chain of transforms
// ---------------------------------------------------------------------------

/// A pipeline that chains transformations on events.
///
/// Each stage transforms a value before passing it to the next stage.
pub struct EventPipeline<T: Clone + Send + Sync + 'static> {
    stages: Vec<Box<dyn Fn(T) -> T + Send + Sync>>,
    emitter: Emitter<T>,
}

impl<T: Clone + Send + Sync + 'static> EventPipeline<T> {
    /// Create a new empty pipeline.
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            emitter: Emitter::new(),
        }
    }

    /// Add a transformation stage.
    pub fn add_stage<F>(&mut self, f: F)
    where
        F: Fn(T) -> T + Send + Sync + 'static,
    {
        self.stages.push(Box::new(f));
    }

    /// Process a value through all stages and fire the result.
    pub fn process(&self, mut value: T) {
        for stage in &self.stages {
            value = stage(value);
        }
        self.emitter.fire(&value);
    }

    /// Returns the subscribable event for the pipeline output.
    pub fn event(&self) -> Event<T> {
        self.emitter.event()
    }

    /// Number of stages in the pipeline.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }
}

// ---------------------------------------------------------------------------
// EventStatistics — count events by tag
// ---------------------------------------------------------------------------

/// Tracks event statistics: total count, per-tag counts, and timestamps.
pub struct EventStatistics {
    total: usize,
    by_tag: std::collections::HashMap<String, usize>,
    first_event_time: Option<u64>,
    last_event_time: Option<u64>,
}

impl EventStatistics {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total: 0,
            by_tag: std::collections::HashMap::new(),
            first_event_time: None,
            last_event_time: None,
        }
    }

    /// Record an event with the given tag.
    pub fn record(&mut self, tag: &str) {
        let now = current_time_ms();
        self.total += 1;
        *self.by_tag.entry(tag.to_string()).or_insert(0) += 1;
        if self.first_event_time.is_none() {
            self.first_event_time = Some(now);
        }
        self.last_event_time = Some(now);
    }

    /// Total number of events recorded.
    pub fn total(&self) -> usize {
        self.total
    }

    /// Count for a specific tag. Returns 0 if the tag has never been recorded.
    pub fn count_for(&self, tag: &str) -> usize {
        self.by_tag.get(tag).copied().unwrap_or(0)
    }

    /// Return all recorded tags and their counts.
    pub fn all_counts(&self) -> &std::collections::HashMap<String, usize> {
        &self.by_tag
    }

    /// Number of distinct tags recorded.
    pub fn distinct_tags(&self) -> usize {
        self.by_tag.len()
    }

    /// Return the tag with the highest count, or `None` if empty.
    pub fn most_frequent(&self) -> Option<(&str, usize)> {
        self.by_tag
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(tag, count)| (tag.as_str(), *count))
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        self.total = 0;
        self.by_tag.clear();
        self.first_event_time = None;
        self.last_event_time = None;
    }

    /// Milliseconds between the first and last recorded event, or 0 if fewer
    /// than two events have been recorded.
    pub fn duration_ms(&self) -> u64 {
        match (self.first_event_time, self.last_event_time) {
            (Some(first), Some(last)) => last.saturating_sub(first),
            _ => 0,
        }
    }
}

impl Default for EventStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for EventStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventStatistics")
            .field("total", &self.total)
            .field("distinct_tags", &self.distinct_tags())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EventAggregator — batch events and flush
// ---------------------------------------------------------------------------

/// Collects events into a batch and fires them as a single `Vec<T>` when
/// the batch reaches a configured size or when manually flushed.
pub struct EventAggregator<T: Clone + Send + Sync + 'static> {
    batch: Mutex<Vec<T>>,
    batch_size: usize,
    emitter: Emitter<Vec<T>>,
}

impl<T: Clone + Send + Sync + 'static> EventAggregator<T> {
    /// Create a new aggregator that flushes every `batch_size` events.
    pub fn new(batch_size: usize) -> Self {
        assert!(batch_size > 0, "batch_size must be > 0");
        Self {
            batch: Mutex::new(Vec::with_capacity(batch_size)),
            batch_size,
            emitter: Emitter::new(),
        }
    }

    /// Add a value to the current batch. If the batch reaches `batch_size`,
    /// it is automatically flushed.
    pub fn push(&self, value: T) {
        let mut batch = self.batch.lock().unwrap();
        batch.push(value);
        if batch.len() >= self.batch_size {
            let items: Vec<T> = batch.drain(..).collect();
            drop(batch);
            self.emitter.fire(&items);
        }
    }

    /// Flush any pending events in the current batch regardless of size.
    /// No-op if the batch is empty.
    pub fn flush(&self) {
        let mut batch = self.batch.lock().unwrap();
        if batch.is_empty() {
            return;
        }
        let items: Vec<T> = batch.drain(..).collect();
        drop(batch);
        self.emitter.fire(&items);
    }

    /// Returns the subscribable event that fires batches.
    pub fn event(&self) -> Event<Vec<T>> {
        self.emitter.event()
    }

    /// Number of events waiting in the current batch.
    pub fn pending(&self) -> usize {
        self.batch.lock().unwrap().len()
    }

    /// The configured batch size.
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

// ---------------------------------------------------------------------------
// WildcardMatcher — glob-style pattern matching for event channel names
// ---------------------------------------------------------------------------

/// Matches event names against glob-style patterns.
///
/// Supports `*` (match any sequence of non-`.` chars) and `**` (match
/// any sequence including `.`).  For example, `"editor.*"` matches
/// `"editor.change"` but not `"editor.cursor.move"`, while
/// `"editor.**"` matches both.
pub struct WildcardMatcher {
    pattern: String,
}

impl WildcardMatcher {
    /// Create a new matcher from a pattern string.
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
        }
    }

    /// Return the pattern string.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Test whether `name` matches this pattern.
    pub fn matches(&self, name: &str) -> bool {
        Self::do_match(self.pattern.as_bytes(), name.as_bytes())
    }

    fn do_match(pat: &[u8], name: &[u8]) -> bool {
        // Handle "**" (matches everything including dots)
        if pat == b"**" {
            return true;
        }

        let mut pi = 0;
        let mut ni = 0;
        let mut star_pi: Option<usize> = None;
        let mut star_ni: Option<usize> = None;

        while ni < name.len() {
            if pi < pat.len() && (pat[pi] == name[ni] || pat[pi] == b'?') {
                pi += 1;
                ni += 1;
            } else if pi + 1 < pat.len() && pat[pi] == b'*' && pat[pi + 1] == b'*' {
                // "**" — match everything
                star_pi = Some(pi);
                star_ni = Some(ni);
                pi += 2;
                // skip trailing dot after **
                if pi < pat.len() && pat[pi] == b'.' {
                    pi += 1;
                }
            } else if pi < pat.len() && pat[pi] == b'*' {
                // single "*" — match non-dot chars
                star_pi = Some(pi);
                star_ni = Some(ni);
                pi += 1;
            } else if let (Some(sp), Some(sn)) = (star_pi, star_ni) {
                // Backtrack: check if the star was a double-star
                let is_double = sp + 1 < pat.len()
                    && pat.get(sp) == Some(&b'*')
                    && pat.get(sp + 1) == Some(&b'*');
                if !is_double && name[sn] == b'.' {
                    return false;
                }
                let new_sn = sn + 1;
                star_ni = Some(new_sn);
                ni = new_sn;
                pi = sp + if is_double { 2 } else { 1 };
                if is_double && pi < pat.len() && pat[pi] == b'.' {
                    pi += 1;
                }
            } else {
                return false;
            }
        }

        // Consume trailing stars in pattern
        while pi < pat.len() && pat[pi] == b'*' {
            pi += 1;
        }

        pi == pat.len()
    }
}

impl fmt::Debug for WildcardMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WildcardMatcher")
            .field("pattern", &self.pattern)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EventCorrelation — group related events by correlation id
// ---------------------------------------------------------------------------

/// Groups events by a string correlation ID.
///
/// Useful for tracking a sequence of related events (e.g. all events
/// belonging to a single user action or transaction).
pub struct EventCorrelation<T> {
    groups: std::collections::HashMap<String, Vec<T>>,
}

impl<T: Clone> EventCorrelation<T> {
    /// Create a new empty correlation tracker.
    pub fn new() -> Self {
        Self {
            groups: std::collections::HashMap::new(),
        }
    }

    /// Add an event to the correlation group identified by `id`.
    pub fn add(&mut self, id: &str, value: T) {
        self.groups
            .entry(id.to_string())
            .or_default()
            .push(value);
    }

    /// Get all events in a correlation group.
    pub fn get(&self, id: &str) -> Option<&[T]> {
        self.groups.get(id).map(|v| v.as_slice())
    }

    /// Number of events in a specific group, or 0 if unknown.
    pub fn group_size(&self, id: &str) -> usize {
        self.groups.get(id).map_or(0, |v| v.len())
    }

    /// Number of active correlation groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Remove a completed correlation group and return its events.
    pub fn complete(&mut self, id: &str) -> Option<Vec<T>> {
        self.groups.remove(id)
    }

    /// Return all correlation IDs.
    pub fn ids(&self) -> Vec<&str> {
        self.groups.keys().map(|s| s.as_str()).collect()
    }

    /// Clear all groups.
    pub fn clear(&mut self) {
        self.groups.clear();
    }

    /// Total number of events across all groups.
    pub fn total_events(&self) -> usize {
        self.groups.values().map(|v| v.len()).sum()
    }
}

impl<T: Clone> Default for EventCorrelation<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> fmt::Debug for EventCorrelation<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventCorrelation")
            .field("groups", &self.groups.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EventRouter — route events to named handlers based on predicates
// ---------------------------------------------------------------------------

/// Routes events to one or more named handlers based on predicates.
///
/// Each route has a name, a predicate, and a handler. When an event is
/// dispatched, all matching routes fire their handlers.
pub struct EventRouter<T> {
    routes: Vec<Route<T>>,
}

struct Route<T> {
    name: String,
    predicate: Box<dyn Fn(&T) -> bool + Send + Sync>,
    handler: Arc<dyn Fn(&T) + Send + Sync>,
}

impl<T> EventRouter<T> {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Add a named route. Events matching `predicate` will be sent to
    /// `handler`.
    pub fn add_route<P, H>(&mut self, name: impl Into<String>, predicate: P, handler: H)
    where
        P: Fn(&T) -> bool + Send + Sync + 'static,
        H: Fn(&T) + Send + Sync + 'static,
    {
        self.routes.push(Route {
            name: name.into(),
            predicate: Box::new(predicate),
            handler: Arc::new(handler),
        });
    }

    /// Dispatch a value through the router. Returns the names of routes
    /// that matched.
    pub fn dispatch(&self, value: &T) -> Vec<&str> {
        let mut matched = Vec::new();
        for route in &self.routes {
            if (route.predicate)(value) {
                (route.handler)(value);
                matched.push(route.name.as_str());
            }
        }
        matched
    }

    /// Number of registered routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Remove all routes with the given name.
    pub fn remove_route(&mut self, name: &str) {
        self.routes.retain(|r| r.name != name);
    }

    /// List all route names.
    pub fn route_names(&self) -> Vec<&str> {
        self.routes.iter().map(|r| r.name.as_str()).collect()
    }
}

impl<T> Default for EventRouter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for EventRouter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventRouter")
            .field("routes", &self.route_count())
            .finish()
    }
}


// ---------------------------------------------------------------------------
// EventPriorityQueue — priority-based event delivery
// ---------------------------------------------------------------------------

/// Priority levels for event listeners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventPriority {
    /// Lowest priority — runs last.
    Low,
    /// Default priority.
    Normal,
    /// Higher priority — runs before normal.
    High,
    /// Highest priority — runs first.
    Critical,
}

impl EventPriority {
    /// Numeric weight (higher = more important).
    pub fn weight(&self) -> u32 {
        match self {
            Self::Low => 0,
            Self::Normal => 10,
            Self::High => 20,
            Self::Critical => 30,
        }
    }

    /// Parse from a string label (case-insensitive).
    pub fn from_label(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "normal" => Some(Self::Normal),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

impl Default for EventPriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl fmt::Display for EventPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        };
        f.write_str(label)
    }
}

/// A prioritised event entry waiting to be delivered.
#[derive(Debug, Clone)]
pub struct PriorityEntry<T> {
    pub value: T,
    pub priority: EventPriority,
    sequence: u64,
}

/// A queue that orders events by priority before delivery.
pub struct EventPriorityQueue<T> {
    entries: Vec<PriorityEntry<T>>,
    next_seq: u64,
}

impl<T: Clone + fmt::Debug> EventPriorityQueue<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_seq: 0,
        }
    }

    /// Enqueue an event with a given priority.
    pub fn push(&mut self, value: T, priority: EventPriority) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entries.push(PriorityEntry { value, priority, sequence: seq });
    }

    /// Number of queued events.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Sort entries by descending priority, then ascending sequence.
    fn sort(&mut self) {
        self.entries.sort_by(|a, b| {
            b.priority.weight().cmp(&a.priority.weight())
                .then_with(|| a.sequence.cmp(&b.sequence))
        });
    }

    /// Drain all entries in priority order.
    pub fn drain_sorted(&mut self) -> Vec<PriorityEntry<T>> {
        self.sort();
        self.entries.drain(..).collect()
    }

    /// Peek at the highest-priority entry without removing it.
    pub fn peek(&mut self) -> Option<&PriorityEntry<T>> {
        self.sort();
        self.entries.first()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Count entries at a specific priority level.
    pub fn count_at_priority(&self, priority: EventPriority) -> usize {
        self.entries.iter().filter(|e| e.priority == priority).count()
    }
}

impl<T: Clone + fmt::Debug> Default for EventPriorityQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for EventPriorityQueue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventPriorityQueue")
            .field("len", &self.entries.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EventReplaySession — record and replay events for debugging
// ---------------------------------------------------------------------------

/// A recorded event with metadata.
#[derive(Debug, Clone)]
pub struct RecordedEvent<T> {
    pub value: T,
    pub sequence: u64,
    pub label: String,
}

/// Records events and allows replaying them through an emitter.
pub struct EventReplaySession<T> {
    events: Vec<RecordedEvent<T>>,
    next_seq: u64,
}

impl<T: Clone + Send + Sync + 'static> EventReplaySession<T> {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            next_seq: 0,
        }
    }

    /// Record an event with a label.
    pub fn record(&mut self, value: T, label: impl Into<String>) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(RecordedEvent {
            value,
            sequence: seq,
            label: label.into(),
        });
    }

    /// Number of recorded events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether no events have been recorded.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get a reference to all recorded events.
    pub fn events(&self) -> &[RecordedEvent<T>] {
        &self.events
    }

    /// Replay all recorded events through the given emitter.
    pub fn replay(&self, emitter: &Emitter<T>) {
        for event in &self.events {
            emitter.fire(&event.value);
        }
    }

    /// Replay events from index `start` (inclusive) to `end` (exclusive).
    pub fn replay_range(&self, emitter: &Emitter<T>, start: usize, end: usize) {
        let end = end.min(self.events.len());
        for event in &self.events[start..end] {
            emitter.fire(&event.value);
        }
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
        self.next_seq = 0;
    }

    /// Get events matching a label prefix.
    pub fn events_with_prefix(&self, prefix: &str) -> Vec<&RecordedEvent<T>> {
        self.events.iter().filter(|e| e.label.starts_with(prefix)).collect()
    }
}

impl<T: Clone + Send + Sync + 'static> Default for EventReplaySession<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for EventReplaySession<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventReplaySession")
            .field("events", &self.events.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EventBatchAggregator — aggregate multiple events into batches
// ---------------------------------------------------------------------------

/// Collects events and flushes them as a batch.
pub struct EventBatchAggregator<T> {
    buffer: Vec<T>,
    max_size: usize,
}

impl<T: Clone + Send + Sync + 'static> EventBatchAggregator<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_size: if max_size == 0 { 1 } else { max_size },
        }
    }

    /// Add an event to the batch buffer.
    /// Returns `true` if the batch is now full and should be flushed.
    pub fn add(&mut self, value: T) -> bool {
        self.buffer.push(value);
        self.buffer.len() >= self.max_size
    }

    /// Whether the buffer is full.
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.max_size
    }

    /// Current number of buffered events.
    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }

    /// Drain the buffer, returning all buffered events.
    pub fn flush(&mut self) -> Vec<T> {
        self.buffer.drain(..).collect()
    }

    /// Flush and fire all buffered events through an emitter.
    pub fn flush_to_emitter(&mut self, emitter: &Emitter<T>) {
        let events = self.flush();
        for event in &events {
            emitter.fire(event);
        }
    }

    /// Clear the buffer without returning events.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// The configured maximum batch size.
    pub fn max_size(&self) -> usize {
        self.max_size
    }
}

impl<T: Clone + Send + Sync + 'static> Default for EventBatchAggregator<T> {
    fn default() -> Self {
        Self::new(10)
    }
}

impl<T> fmt::Debug for EventBatchAggregator<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBatchAggregator")
            .field("buffered", &self.buffer.len())
            .field("max_size", &self.max_size)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EventThrottleWindow — rate-limit event delivery
// ---------------------------------------------------------------------------

/// Tracks event delivery rate within a time window.
/// Uses a simple counter-based approach (time-independent for testability).
#[derive(Debug, Clone)]
pub struct EventThrottleWindow {
    /// Maximum events allowed in the window.
    max_events: usize,
    /// Events delivered in the current window.
    current_count: usize,
    /// Total events that were throttled (dropped).
    throttled_total: u64,
    /// Total events that were allowed.
    allowed_total: u64,
}

impl EventThrottleWindow {
    pub fn new(max_events: usize) -> Self {
        Self {
            max_events: if max_events == 0 { 1 } else { max_events },
            current_count: 0,
            throttled_total: 0,
            allowed_total: 0,
        }
    }

    /// Try to allow an event. Returns `true` if the event should proceed,
    /// `false` if it should be throttled.
    pub fn try_allow(&mut self) -> bool {
        if self.current_count < self.max_events {
            self.current_count += 1;
            self.allowed_total += 1;
            true
        } else {
            self.throttled_total += 1;
            false
        }
    }

    /// Reset the window counter (call this at the start of each time window).
    pub fn reset_window(&mut self) {
        self.current_count = 0;
    }

    /// Current count in this window.
    pub fn current_count(&self) -> usize {
        self.current_count
    }

    /// Total events throttled across all windows.
    pub fn throttled_total(&self) -> u64 {
        self.throttled_total
    }

    /// Total events allowed across all windows.
    pub fn allowed_total(&self) -> u64 {
        self.allowed_total
    }

    /// Whether the window is currently full.
    pub fn is_throttled(&self) -> bool {
        self.current_count >= self.max_events
    }

    /// Remaining capacity in the current window.
    pub fn remaining(&self) -> usize {
        self.max_events.saturating_sub(self.current_count)
    }

    /// The throttle rate as a percentage (0.0 – 100.0).
    pub fn throttle_rate(&self) -> f64 {
        let total = self.allowed_total + self.throttled_total;
        if total == 0 {
            return 0.0;
        }
        (self.throttled_total as f64 / total as f64) * 100.0
    }

    /// The configured maximum events per window.
    pub fn max_events(&self) -> usize {
        self.max_events
    }
}

impl Default for EventThrottleWindow {
    fn default() -> Self {
        Self::new(100)
    }
}

impl fmt::Display for EventThrottleWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "throttle({}/{}, allowed={}, throttled={})",
            self.current_count, self.max_events,
            self.allowed_total, self.throttled_total,
        )
    }
}


/// Configuration manager for events functionality.
pub struct EventsConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl EventsConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &EventsConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for events operations.
pub struct EventsRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl EventsRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for events.
pub struct EventsValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl EventsValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &EventsValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Event emitter and listener management — extended utilities (qq)
// ---------------------------------------------------------------------------

/// Metric accumulator for events operations.
#[derive(Debug, Clone)]
pub struct QqMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QqMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for events.
#[derive(Debug, Clone)]
pub struct QqRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QqRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for events lookups.
#[derive(Debug, Clone)]
pub struct QqLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QqLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 4
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer4 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer4 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_4(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_4<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_4<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_4(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_4(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 46
// ---------------------------------------------------------------------------

/// Generic object pool `Xc46Pool<T>`.
pub struct Xc46Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc46Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc46PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc46Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc46PoolStats {
        Xc46PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc46Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc46Scheduler`.
pub struct Xc46Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc46Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc46Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_46 hash for the given byte slice.
pub fn xc_46_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_46 convention.
pub fn xc_46_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe7 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe7Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe7PipelineError {
    pub stage: Xe7Stage,
    pub message: String,
}

impl std::fmt::Display for Xe7PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe7Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe7Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError>>>,
    stage_names: Vec<Xe7Stage>,
}

impl Xe7Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe7Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe7Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe7Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe7Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe7Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe7CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe7CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe7Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe7CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe7CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe7Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe7CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_7_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe7CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_7_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe7CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_7_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> {
    Ok(data)
}

pub fn xe_7_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_7_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_7_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_7_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe7PipelineError> {
    Err(Xe7PipelineError {
        stage: Xe7Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #70
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf70Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf70TrieNode {
    children: std::collections::HashMap<char, Xf70TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf70Trie {
    root: Xf70TrieNode,
    count: usize,
}

impl Xf70Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf70TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf70TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf70TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf70BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf70BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 45).
pub struct Xh45SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh45SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 87 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 45).
pub struct Xh45BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh45BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
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

    // -----------------------------------------------------------------------
    // New functionality tests
    // -----------------------------------------------------------------------

    #[test]
    fn replay_buffer_is_full() {
        let mut buf = EventReplayBuffer::new(2);
        assert!(!buf.is_full());
        buf.push(1);
        assert!(!buf.is_full());
        buf.push(2);
        assert!(buf.is_full());
        buf.push(3);
        assert!(buf.is_full());
    }

    #[test]
    fn replay_buffer_last() {
        let mut buf = EventReplayBuffer::<i32>::new(3);
        assert_eq!(buf.last(), None);
        buf.push(10);
        assert_eq!(buf.last(), Some(&10));
        buf.push(20);
        assert_eq!(buf.last(), Some(&20));
        buf.push(30);
        buf.push(40);
        assert_eq!(buf.last(), Some(&40));
    }

    #[test]
    fn replay_buffer_iter() {
        let mut buf = EventReplayBuffer::new(4);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        let collected: Vec<&i32> = buf.iter().collect();
        assert_eq!(collected, vec![&1, &2, &3]);
    }

    #[test]
    fn replay_buffer_display() {
        let mut buf = EventReplayBuffer::new(5);
        buf.push(1);
        buf.push(2);
        assert_eq!(format!("{buf}"), "[buffer: 2/5 items]");
        buf.push(3);
        buf.push(4);
        buf.push(5);
        assert_eq!(format!("{buf}"), "[buffer: 5/5 items]");
    }

    #[test]
    fn event_filter_negate() {
        let filter = EventFilter::new(|v: &i32| *v > 5);
        assert!(filter.matches(&10));
        let negated = filter.negate();
        assert!(!negated.matches(&10));
        assert!(negated.matches(&3));
    }

    #[test]
    fn event_counter_basic() {
        let mut counter = EventCounter::<i32>::new();
        assert_eq!(counter.count(), 0);
        counter.increment();
        counter.increment();
        counter.increment();
        assert_eq!(counter.count(), 3);
    }

    #[test]
    fn event_counter_reset() {
        let mut counter = EventCounter::<String>::new();
        counter.increment();
        counter.increment();
        assert_eq!(counter.count(), 2);
        counter.reset();
        assert_eq!(counter.count(), 0);
        counter.increment();
        assert_eq!(counter.count(), 1);
    }

    // -- EventBus tests ------------------------------------------------------

    #[test]
    fn event_bus_named_channels() {
        let bus = EventBus::<i32>::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = bus.on("numbers", move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        bus.fire("numbers", &42);
        bus.fire("other", &99);
        assert_eq!(*received.lock().unwrap(), vec![42]);
        assert!(bus.has_channel("numbers"));
        assert_eq!(bus.channel_count(), 2);
    }

    #[test]
    fn event_bus_remove_channel() {
        let bus = EventBus::<String>::new();
        let _handle = bus.on("ch1", |_| {});
        assert!(bus.has_channel("ch1"));
        bus.remove_channel("ch1");
        assert!(!bus.has_channel("ch1"));
    }

    // -- EventThrottle tests -------------------------------------------------

    #[test]
    fn event_throttle_limits_fires() {
        let throttle = EventThrottle::<i32>::new(2, 10_000);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = throttle.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        assert!(throttle.fire(&1));
        assert!(throttle.fire(&2));
        assert!(!throttle.fire(&3)); // throttled
        assert_eq!(received.lock().unwrap().len(), 2);
    }

    #[test]
    fn event_throttle_reset() {
        let throttle = EventThrottle::<i32>::new(1, 10_000);
        assert!(throttle.fire(&1));
        assert!(!throttle.fire(&2));
        throttle.reset();
        assert!(throttle.fire(&3));
    }

    // -- EventPipeline tests -------------------------------------------------

    #[test]
    fn event_pipeline_processes_stages() {
        let mut pipeline = EventPipeline::<i32>::new();
        pipeline.add_stage(|v| v + 10);
        pipeline.add_stage(|v| v * 2);
        assert_eq!(pipeline.stage_count(), 2);

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = pipeline.event().on(move |v: &i32| {
            r.lock().unwrap().push(*v);
        });
        pipeline.process(5); // (5 + 10) * 2 = 30
        assert_eq!(*received.lock().unwrap(), vec![30]);
    }

    #[test]
    fn event_pipeline_empty_passes_through() {
        let pipeline = EventPipeline::<String>::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = pipeline.event().on(move |v: &String| {
            r.lock().unwrap().push(v.clone());
        });
        pipeline.process("hello".to_string());
        assert_eq!(received.lock().unwrap()[0], "hello");
    }

    // -- EventStatistics tests -----------------------------------------------

    #[test]
    fn event_statistics_record_and_count() {
        let mut stats = EventStatistics::new();
        stats.record("click");
        stats.record("click");
        stats.record("scroll");
        assert_eq!(stats.total(), 3);
        assert_eq!(stats.count_for("click"), 2);
        assert_eq!(stats.count_for("scroll"), 1);
        assert_eq!(stats.count_for("keypress"), 0);
        assert_eq!(stats.distinct_tags(), 2);
    }

    #[test]
    fn event_statistics_most_frequent() {
        let mut stats = EventStatistics::new();
        assert!(stats.most_frequent().is_none());
        stats.record("a");
        stats.record("b");
        stats.record("b");
        stats.record("b");
        stats.record("a");
        let (tag, count) = stats.most_frequent().unwrap();
        assert_eq!(tag, "b");
        assert_eq!(count, 3);
    }

    #[test]
    fn event_statistics_reset() {
        let mut stats = EventStatistics::new();
        stats.record("x");
        stats.record("y");
        assert_eq!(stats.total(), 2);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.distinct_tags(), 0);
        assert_eq!(stats.duration_ms(), 0);
    }

    // -- EventAggregator tests -----------------------------------------------

    #[test]
    fn aggregator_auto_flushes_at_batch_size() {
        let agg = EventAggregator::<i32>::new(3);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = agg.event().on(move |batch: &Vec<i32>| {
            r.lock().unwrap().push(batch.clone());
        });
        agg.push(1);
        agg.push(2);
        assert_eq!(agg.pending(), 2);
        assert!(received.lock().unwrap().is_empty());
        agg.push(3); // triggers auto-flush
        assert_eq!(agg.pending(), 0);
        let batches = received.lock().unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0], vec![1, 2, 3]);
    }

    #[test]
    fn aggregator_manual_flush() {
        let agg = EventAggregator::<String>::new(100);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = agg.event().on(move |batch: &Vec<String>| {
            r.lock().unwrap().push(batch.clone());
        });
        agg.push("a".into());
        agg.push("b".into());
        assert_eq!(agg.pending(), 2);
        agg.flush();
        assert_eq!(agg.pending(), 0);
        let batches = received.lock().unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0], vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn aggregator_flush_empty_is_noop() {
        let agg = EventAggregator::<i32>::new(5);
        let received = Arc::new(Mutex::new(Vec::<Vec<i32>>::new()));
        let r = received.clone();
        let _h = agg.event().on(move |batch: &Vec<i32>| {
            r.lock().unwrap().push(batch.clone());
        });
        agg.flush(); // no-op
        assert!(received.lock().unwrap().is_empty());
    }

    // -- WildcardMatcher tests -----------------------------------------------

    #[test]
    fn wildcard_exact_match() {
        let m = WildcardMatcher::new("editor.change");
        assert!(m.matches("editor.change"));
        assert!(!m.matches("editor.scroll"));
        assert!(!m.matches("editor"));
    }

    #[test]
    fn wildcard_single_star_matches_segment() {
        let m = WildcardMatcher::new("editor.*");
        assert!(m.matches("editor.change"));
        assert!(m.matches("editor.scroll"));
        // single star should NOT cross dot boundaries
        assert!(!m.matches("editor.cursor.move"));
    }

    #[test]
    fn wildcard_double_star_matches_all() {
        let m = WildcardMatcher::new("editor.**");
        assert!(m.matches("editor.change"));
        assert!(m.matches("editor.cursor.move"));
        assert!(m.matches("editor.cursor.selection.expand"));
        assert!(!m.matches("window.resize"));
    }

    #[test]
    fn wildcard_bare_double_star() {
        let m = WildcardMatcher::new("**");
        assert!(m.matches("anything"));
        assert!(m.matches("a.b.c.d"));
        assert!(m.matches(""));
    }

    #[test]
    fn wildcard_pattern_accessor() {
        let m = WildcardMatcher::new("foo.*");
        assert_eq!(m.pattern(), "foo.*");
    }

    // -- EventCorrelation tests ----------------------------------------------

    #[test]
    fn correlation_add_and_get() {
        let mut corr = EventCorrelation::<String>::new();
        corr.add("tx-1", "start".into());
        corr.add("tx-1", "step-a".into());
        corr.add("tx-2", "start".into());

        assert_eq!(corr.group_count(), 2);
        assert_eq!(corr.group_size("tx-1"), 2);
        assert_eq!(corr.group_size("tx-2"), 1);
        assert_eq!(corr.group_size("tx-3"), 0);
        assert_eq!(corr.total_events(), 3);

        let events = corr.get("tx-1").unwrap();
        assert_eq!(events, &["start", "step-a"]);
    }

    #[test]
    fn correlation_complete_removes_group() {
        let mut corr = EventCorrelation::<i32>::new();
        corr.add("g1", 10);
        corr.add("g1", 20);
        let completed = corr.complete("g1").unwrap();
        assert_eq!(completed, vec![10, 20]);
        assert_eq!(corr.group_count(), 0);
        assert!(corr.complete("g1").is_none());
    }

    #[test]
    fn correlation_clear() {
        let mut corr = EventCorrelation::<i32>::new();
        corr.add("a", 1);
        corr.add("b", 2);
        assert_eq!(corr.group_count(), 2);
        corr.clear();
        assert_eq!(corr.group_count(), 0);
        assert_eq!(corr.total_events(), 0);
    }

    // -- EventRouter tests ---------------------------------------------------

    #[test]
    fn router_dispatches_to_matching_routes() {
        let mut router = EventRouter::<i32>::new();
        let evens = Arc::new(Mutex::new(Vec::new()));
        let odds = Arc::new(Mutex::new(Vec::new()));

        let e = evens.clone();
        router.add_route("evens", |v| v % 2 == 0, move |v| {
            e.lock().unwrap().push(*v);
        });
        let o = odds.clone();
        router.add_route("odds", |v| v % 2 != 0, move |v| {
            o.lock().unwrap().push(*v);
        });

        assert_eq!(router.route_count(), 2);

        let matched = router.dispatch(&4);
        assert_eq!(matched, vec!["evens"]);
        let matched = router.dispatch(&7);
        assert_eq!(matched, vec!["odds"]);

        assert_eq!(*evens.lock().unwrap(), vec![4]);
        assert_eq!(*odds.lock().unwrap(), vec![7]);
    }

    #[test]
    fn router_remove_route() {
        let mut router = EventRouter::<i32>::new();
        router.add_route("a", |_| true, |_| {});
        router.add_route("b", |_| true, |_| {});
        assert_eq!(router.route_count(), 2);
        router.remove_route("a");
        assert_eq!(router.route_count(), 1);
        assert_eq!(router.route_names(), vec!["b"]);
    }

    #[test]
    fn router_multiple_routes_match_same_event() {
        let mut router = EventRouter::<i32>::new();
        let log = Arc::new(Mutex::new(Vec::new()));

        let l1 = log.clone();
        router.add_route("positive", |v| *v > 0, move |v| {
            l1.lock().unwrap().push(format!("pos:{v}"));
        });
        let l2 = log.clone();
        router.add_route("small", |v| *v < 10, move |v| {
            l2.lock().unwrap().push(format!("small:{v}"));
        });

        let matched = router.dispatch(&5);
        assert_eq!(matched, vec!["positive", "small"]);
        assert_eq!(
            *log.lock().unwrap(),
            vec!["pos:5".to_string(), "small:5".to_string()]
        );
    }

    // --- EventPriority tests ------------------------------------------------

    #[test]
    fn priority_ordering() {
        assert!(EventPriority::Low < EventPriority::Normal);
        assert!(EventPriority::Normal < EventPriority::High);
        assert!(EventPriority::High < EventPriority::Critical);
    }

    #[test]
    fn priority_from_label() {
        assert_eq!(EventPriority::from_label("high"), Some(EventPriority::High));
        assert_eq!(EventPriority::from_label("LOW"), Some(EventPriority::Low));
        assert_eq!(EventPriority::from_label("unknown"), None);
    }

    #[test]
    fn priority_display() {
        assert_eq!(EventPriority::Normal.to_string(), "normal");
        assert_eq!(EventPriority::Critical.to_string(), "critical");
    }

    #[test]
    fn priority_default() {
        assert_eq!(EventPriority::default(), EventPriority::Normal);
    }

    // --- EventPriorityQueue tests -------------------------------------------

    #[test]
    fn priority_queue_basic() {
        let mut q = EventPriorityQueue::<i32>::new();
        q.push(1, EventPriority::Low);
        q.push(2, EventPriority::Critical);
        q.push(3, EventPriority::Normal);
        assert_eq!(q.len(), 3);
        let drained = q.drain_sorted();
        assert_eq!(drained[0].value, 2); // critical first
        assert_eq!(drained[1].value, 3); // normal second
        assert_eq!(drained[2].value, 1); // low last
    }

    #[test]
    fn priority_queue_empty() {
        let q = EventPriorityQueue::<String>::new();
        assert!(q.is_empty());
    }

    #[test]
    fn priority_queue_peek() {
        let mut q = EventPriorityQueue::<i32>::new();
        q.push(10, EventPriority::Normal);
        q.push(20, EventPriority::High);
        let peeked = q.peek().unwrap();
        assert_eq!(peeked.value, 20);
        assert_eq!(q.len(), 2); // peek doesn't remove
    }

    #[test]
    fn priority_queue_count_at_priority() {
        let mut q = EventPriorityQueue::<i32>::new();
        q.push(1, EventPriority::High);
        q.push(2, EventPriority::High);
        q.push(3, EventPriority::Low);
        assert_eq!(q.count_at_priority(EventPriority::High), 2);
        assert_eq!(q.count_at_priority(EventPriority::Low), 1);
        assert_eq!(q.count_at_priority(EventPriority::Normal), 0);
    }

    #[test]
    fn priority_queue_clear() {
        let mut q = EventPriorityQueue::<i32>::new();
        q.push(1, EventPriority::Normal);
        q.clear();
        assert!(q.is_empty());
    }

    #[test]
    fn priority_queue_fifo_within_same_priority() {
        let mut q = EventPriorityQueue::<&str>::new();
        q.push("first", EventPriority::Normal);
        q.push("second", EventPriority::Normal);
        q.push("third", EventPriority::Normal);
        let drained = q.drain_sorted();
        assert_eq!(drained[0].value, "first");
        assert_eq!(drained[1].value, "second");
        assert_eq!(drained[2].value, "third");
    }

    // --- EventReplaySession tests -------------------------------------------

    #[test]
    fn replay_session_record() {
        let mut session = EventReplaySession::<i32>::new();
        session.record(42, "first");
        session.record(99, "second");
        assert_eq!(session.len(), 2);
        assert!(!session.is_empty());
    }

    #[test]
    fn replay_session_replay() {
        let mut session = EventReplaySession::<i32>::new();
        session.record(1, "a");
        session.record(2, "b");

        let emitter = Emitter::new();
        let event = emitter.event();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = event.on(move |v: &i32| { r.lock().unwrap().push(*v); });

        session.replay(&emitter);
        assert_eq!(*received.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn replay_session_replay_range() {
        let mut session = EventReplaySession::<i32>::new();
        for i in 0..5 {
            session.record(i, format!("e{i}"));
        }

        let emitter = Emitter::new();
        let event = emitter.event();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = event.on(move |v: &i32| { r.lock().unwrap().push(*v); });

        session.replay_range(&emitter, 1, 3);
        assert_eq!(*received.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn replay_session_events_with_prefix() {
        let mut session = EventReplaySession::<i32>::new();
        session.record(1, "mouse.click");
        session.record(2, "key.press");
        session.record(3, "mouse.move");
        let mouse = session.events_with_prefix("mouse");
        assert_eq!(mouse.len(), 2);
    }

    #[test]
    fn replay_session_clear() {
        let mut session = EventReplaySession::<i32>::new();
        session.record(1, "x");
        session.clear();
        assert!(session.is_empty());
    }

    // --- EventBatchAggregator tests -----------------------------------------

    #[test]
    fn batch_aggregator_add() {
        let mut agg = EventBatchAggregator::<i32>::new(3);
        assert!(!agg.add(1));
        assert!(!agg.add(2));
        assert!(agg.add(3)); // full
        assert!(agg.is_full());
    }

    #[test]
    fn batch_aggregator_flush() {
        let mut agg = EventBatchAggregator::<i32>::new(10);
        agg.add(1);
        agg.add(2);
        let batch = agg.flush();
        assert_eq!(batch, vec![1, 2]);
        assert_eq!(agg.buffered_count(), 0);
    }

    #[test]
    fn batch_aggregator_flush_to_emitter() {
        let mut agg = EventBatchAggregator::<i32>::new(10);
        agg.add(10);
        agg.add(20);

        let emitter = Emitter::new();
        let event = emitter.event();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = event.on(move |v: &i32| { r.lock().unwrap().push(*v); });

        agg.flush_to_emitter(&emitter);
        assert_eq!(*received.lock().unwrap(), vec![10, 20]);
        assert_eq!(agg.buffered_count(), 0);
    }

    #[test]
    fn batch_aggregator_clear() {
        let mut agg = EventBatchAggregator::<i32>::new(5);
        agg.add(1);
        agg.clear();
        assert_eq!(agg.buffered_count(), 0);
    }

    #[test]
    fn batch_aggregator_max_size() {
        let agg = EventBatchAggregator::<i32>::new(7);
        assert_eq!(agg.max_size(), 7);
    }

    // --- EventThrottleWindow tests ------------------------------------------

    #[test]
    fn throttle_window_basic() {
        let mut tw = EventThrottleWindow::new(3);
        assert!(tw.try_allow());
        assert!(tw.try_allow());
        assert!(tw.try_allow());
        assert!(!tw.try_allow()); // throttled
        assert!(tw.is_throttled());
    }

    #[test]
    fn throttle_window_reset() {
        let mut tw = EventThrottleWindow::new(2);
        tw.try_allow();
        tw.try_allow();
        tw.try_allow(); // throttled
        tw.reset_window();
        assert!(!tw.is_throttled());
        assert!(tw.try_allow());
    }

    #[test]
    fn throttle_window_remaining() {
        let mut tw = EventThrottleWindow::new(5);
        tw.try_allow();
        assert_eq!(tw.remaining(), 4);
    }

    #[test]
    fn throttle_window_stats() {
        let mut tw = EventThrottleWindow::new(2);
        tw.try_allow();
        tw.try_allow();
        tw.try_allow(); // throttled
        assert_eq!(tw.allowed_total(), 2);
        assert_eq!(tw.throttled_total(), 1);
    }

    #[test]
    fn throttle_window_rate() {
        let mut tw = EventThrottleWindow::new(1);
        tw.try_allow();  // allowed
        tw.try_allow();  // throttled
        tw.try_allow();  // throttled
        // 1 allowed, 2 throttled => 66.67%
        assert!(tw.throttle_rate() > 60.0);
    }

    #[test]
    fn throttle_window_empty_rate() {
        let tw = EventThrottleWindow::new(10);
        assert!((tw.throttle_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn throttle_window_display() {
        let tw = EventThrottleWindow::new(100);
        let s = tw.to_string();
        assert!(s.contains("0/100"));
    }

    #[test]
    fn throttle_window_default() {
        let tw = EventThrottleWindow::default();
        assert_eq!(tw.max_events(), 100);
    }


    #[test]
    fn events_config_new() {
        let cfg = EventsConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn events_config_set_get() {
        let mut cfg = EventsConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn events_config_remove() {
        let mut cfg = EventsConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn events_config_keys_sorted() {
        let mut cfg = EventsConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn events_config_bump_version() {
        let mut cfg = EventsConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn events_config_clear() {
        let mut cfg = EventsConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn events_config_merge() {
        let mut cfg1 = EventsConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = EventsConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn events_config_disable() {
        let mut cfg = EventsConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn events_rate_tracker_empty() {
        let rt = EventsRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn events_rate_tracker_record() {
        let mut rt = EventsRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn events_rate_tracker_prune() {
        let mut rt = EventsRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn events_validator_valid() {
        let v = EventsValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn events_validator_errors() {
        let mut v = EventsValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn events_validator_clear() {
        let mut v = EventsValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn events_validator_merge() {
        let mut v1 = EventsValidator::new();
        v1.add_error("e1");
        let mut v2 = EventsValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn events_rate_tracker_clear() {
        let mut rt = EventsRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn qq_metrics_empty() {
        let m = QqMetrics::new("events");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qq_metrics_record_and_mean() {
        let mut m = QqMetrics::new("events");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qq_metrics_min_max() {
        let mut m = QqMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qq_metrics_variance_and_std() {
        let mut m = QqMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qq_metrics_percentile() {
        let mut m = QqMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qq_metrics_merge() {
        let mut a = QqMetrics::new("a");
        a.record(1.0);
        let mut b = QqMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qq_metrics_reset() {
        let mut m = QqMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qq_rate_window_empty() {
        let rw = QqRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qq_rate_window_tick_and_rate() {
        let mut rw = QqRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qq_lru_cache_basic() {
        let mut c = QqLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qq_lru_cache_contains_and_keys() {
        let mut c = QqLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qq_lru_cache_remove() {
        let mut c = QqLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qq_metrics_sum() {
        let mut m = QqMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qq_metrics_label() {
        let m = QqMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qq_lru_cache_clear() {
        let mut c = QqLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_4_push_and_len() {
        let mut rb = super::XbRingBuffer4::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_4_overwrite() {
        let mut rb = super::XbRingBuffer4::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_4_get_out_of_bounds() {
        let rb = super::XbRingBuffer4::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_4_drain_all() {
        let mut rb = super::XbRingBuffer4::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_4_peek_front_back() {
        let mut rb = super::XbRingBuffer4::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_4_clear() {
        let mut rb = super::XbRingBuffer4::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_4_capacity() {
        let rb = super::XbRingBuffer4::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_4_basic() {
        let h = super::xb_fnv1a_4(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_4(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_4_different_inputs() {
        let h1 = super::xb_fnv1a_4(b"abc");
        let h2 = super::xb_fnv1a_4(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_4_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_4(&data);
        let dec = super::xb_rle_decode_4(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_4_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_4(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_4(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_4_values() {
        assert!((super::xb_clamp_4(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_4(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_4(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_4_values() {
        assert!((super::xb_lerp_4(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_4(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_4(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_4_wrap_around_twice() {
        let mut rb = super::XbRingBuffer4::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 46 ----

    #[test]
    fn xc_46_pool_new_empty() {
        let pool: super::Xc46Pool<i32> = super::Xc46Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_46_pool_release_acquire() {
        let mut pool = super::Xc46Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_46_pool_acquire_empty() {
        let mut pool: super::Xc46Pool<i32> = super::Xc46Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_46_pool_full() {
        let mut pool = super::Xc46Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_46_pool_drain() {
        let mut pool = super::Xc46Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_46_pool_stats() {
        let mut pool = super::Xc46Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_46_pool_clear() {
        let mut pool = super::Xc46Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_46_pool_shrink() {
        let mut pool = super::Xc46Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_46_pool_default() {
        let pool: super::Xc46Pool<String> = super::Xc46Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_46_pool_extend() {
        let mut pool = super::Xc46Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_46_pool_retain() {
        let mut pool = super::Xc46Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_46_scheduler_round_robin() {
        let mut sched = super::Xc46Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_46_scheduler_empty() {
        let mut sched = super::Xc46Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_46_scheduler_reset() {
        let mut sched = super::Xc46Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_46_scheduler_add_remove() {
        let mut sched = super::Xc46Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_46_scheduler_targets() {
        let sched = super::Xc46Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_46_hash_empty() {
        assert_eq!(super::xc_46_hash(b""), 5381);
    }

    #[test]
    fn xc_46_hash_data() {
        let h = super::xc_46_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_46_hash(b"hello"), h);
    }

    #[test]
    fn xc_46_reverse_str() {
        assert_eq!(super::xc_46_reverse("abc"), "cba");
        assert_eq!(super::xc_46_reverse(""), "");
    }


    #[test]
    fn xe_7_pipeline_empty() {
        let p = super::Xe7Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_7_pipeline_parse_stage() {
        let p = super::Xe7Pipeline::new()
            .add_parse(super::xe_7_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_7_pipeline_transform_double() {
        let p = super::Xe7Pipeline::new()
            .add_transform(super::xe_7_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_7_pipeline_validate_reverse() {
        let p = super::Xe7Pipeline::new()
            .add_validate(super::xe_7_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_7_pipeline_emit_filter() {
        let p = super::Xe7Pipeline::new()
            .add_emit(super::xe_7_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_7_pipeline_multi_stage() {
        let p = super::Xe7Pipeline::new()
            .add_parse(super::xe_7_pipeline_identity)
            .add_transform(super::xe_7_pipeline_double)
            .add_validate(super::xe_7_pipeline_reverse)
            .add_emit(super::xe_7_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_7_pipeline_error_propagation() {
        let p = super::Xe7Pipeline::new()
            .add_parse(super::xe_7_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe7Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_7_pipeline_compose() {
        let p1 = super::Xe7Pipeline::new()
            .add_parse(super::xe_7_pipeline_identity);
        let p2 = super::Xe7Pipeline::new()
            .add_transform(super::xe_7_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_7_pipeline_error_display() {
        let e = super::Xe7PipelineError {
            stage: super::Xe7Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_7_cache_put_get() {
        let mut c = super::Xe7Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_7_cache_miss() {
        let mut c: super::Xe7Cache<&str, i32> = super::Xe7Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_7_cache_ttl_expiry() {
        let mut c = super::Xe7Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_7_cache_evict() {
        let mut c = super::Xe7Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_7_cache_capacity() {
        let mut c = super::Xe7Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_7_cache_stats() {
        let mut c = super::Xe7Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_7_cache_clear() {
        let mut c = super::Xe7Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #70 --

    #[test]
    fn xf70_trie_insert_search() {
        let mut t = Xf70Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf70_trie_starts_with() {
        let mut t = Xf70Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf70_trie_remove() {
        let mut t = Xf70Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf70_trie_word_count() {
        let mut t = Xf70Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf70_trie_longest_prefix() {
        let mut t = Xf70Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf70_trie_all_words() {
        let mut t = Xf70Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf70_trie_autocomplete() {
        let mut t = Xf70Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf70_trie_empty_search() {
        let t = Xf70Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf70_bloom_add_contains() {
        let mut bf = Xf70BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf70_bloom_probably_absent() {
        let bf = Xf70BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf70_bloom_false_positive_rate() {
        let mut bf = Xf70BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf70_bloom_clear() {
        let mut bf = Xf70BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf70_bloom_union() {
        let mut a = Xf70BloomFilter::xf_new(512, 2);
        let mut b = Xf70BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf70_bloom_intersection_estimate() {
        let mut a = Xf70BloomFilter::xf_new(512, 2);
        let mut b = Xf70BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf70_bloom_union_size_mismatch() {
        let a = Xf70BloomFilter::xf_new(256, 2);
        let b = Xf70BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh45_skip_insert_contains() {
        let mut sl = super::Xh45SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh45_skip_remove() {
        let mut sl = super::Xh45SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh45_skip_len() {
        let mut sl = super::Xh45SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh45_skip_range_query() {
        let mut sl = super::Xh45SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh45_skip_floor_ceiling() {
        let mut sl = super::Xh45SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh45_skip_rank() {
        let mut sl = super::Xh45SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh45_skip_empty() {
        let sl = super::Xh45SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh45_skip_duplicates() {
        let mut sl = super::Xh45SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh45_bitset_set_test() {
        let mut bs = super::Xh45BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh45_bitset_clear_count() {
        let mut bs = super::Xh45BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh45_bitset_and_or_xor() {
        let mut a = super::Xh45BitSet::xh_new(128);
        let mut b = super::Xh45BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh45_bitset_iter_ones() {
        let mut bs = super::Xh45BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh45_bitset_first_last() {
        let mut bs = super::Xh45BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh45_bitset_empty() {
        let bs = super::Xh45BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
