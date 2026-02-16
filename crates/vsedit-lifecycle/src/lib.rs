//! IDisposable pattern and resource cleanup.
//!
//! This crate provides the core disposable/lifecycle primitives for vsedit,
//! faithfully replicating the patterns from VS Code's `vs/base/common/lifecycle.ts`.
//!
//! # Key types
//!
//! - [`Disposable`] — trait for objects that perform cleanup on disposal.
//! - [`DisposableStore`] — collects disposables and disposes all on drop or explicit call.
//! - [`DisposableMap`] — keyed disposable storage; replacing a value disposes the old one.
//! - [`MutableDisposable`] — holds an optional disposable that can be swapped.
//! - [`RefCountedDisposable`] — reference-counted wrapper that disposes at zero.
//! - [`FnDisposable`] — wraps a closure into a [`Disposable`].
//!
//! In debug builds, leak tracking warns when a disposable is dropped without being disposed.

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Disposable trait
// ---------------------------------------------------------------------------

/// An object that performs a cleanup operation when [`dispose()`](Disposable::dispose) is called.
///
/// Implementations **must** be idempotent — calling `dispose()` more than once is safe and has
/// no additional effect.
pub trait Disposable {
    /// Perform cleanup. Subsequent calls are no-ops.
    fn dispose(&self);

    /// Returns `true` after [`dispose()`](Disposable::dispose) has been called.
    fn is_disposed(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Leak tracking (debug builds only)
// ---------------------------------------------------------------------------

#[cfg(debug_assertions)]
mod leak_tracker {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    static TRACKED: Mutex<Option<Vec<(usize, &'static str)>>> = Mutex::new(None);
    static ENABLED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    /// Enable or disable leak tracking. When enabled, undisposed resources that are dropped
    /// without being disposed will emit a warning to stderr.
    pub fn set_tracking_enabled(enabled: bool) {
        ENABLED.store(enabled, Ordering::SeqCst);
        let mut guard = TRACKED.lock().unwrap();
        if enabled && guard.is_none() {
            *guard = Some(Vec::new());
        }
    }

    pub fn is_tracking_enabled() -> bool {
        ENABLED.load(Ordering::SeqCst)
    }

    pub fn track(type_name: &'static str) -> usize {
        if !is_tracking_enabled() {
            return 0;
        }
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut guard) = TRACKED.lock() {
            if let Some(ref mut v) = *guard {
                v.push((id, type_name));
            }
        }
        id
    }

    pub fn mark_disposed(id: usize) {
        if !is_tracking_enabled() {
            return;
        }
        if let Ok(mut guard) = TRACKED.lock() {
            if let Some(ref mut v) = *guard {
                v.retain(|(i, _)| *i != id);
            }
        }
    }

    pub fn warn_if_leaked(id: usize, type_name: &str) {
        if !is_tracking_enabled() {
            return;
        }
        let is_tracked = TRACKED
            .lock()
            .ok()
            .and_then(|g| {
                g.as_ref().map(|v| v.iter().any(|(i, _)| *i == id))
            })
            .unwrap_or(false);
        if is_tracked {
            eprintln!(
                "[LEAK] {type_name} (id={id}) was dropped without being disposed"
            );
        }
    }

    /// Returns the number of currently tracked (undisposed) resources.
    pub fn tracked_count() -> usize {
        TRACKED
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|v| v.len()))
            .unwrap_or(0)
    }
}

#[cfg(debug_assertions)]
pub use leak_tracker::{set_tracking_enabled, tracked_count};

/// Enable or disable leak tracking (no-op in release builds).
#[cfg(not(debug_assertions))]
pub fn set_tracking_enabled(_enabled: bool) {}

/// Returns the number of currently tracked resources (always 0 in release builds).
#[cfg(not(debug_assertions))]
pub fn tracked_count() -> usize {
    0
}

// Helpers used by types below — these compile to nothing in release mode.
#[inline]
fn track_new(_type_name: &'static str) -> usize {
    #[cfg(debug_assertions)]
    {
        leak_tracker::track(_type_name)
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = _type_name;
        0
    }
}

#[inline]
fn mark_disposed(_id: usize) {
    #[cfg(debug_assertions)]
    leak_tracker::mark_disposed(_id);
}

#[inline]
fn warn_if_leaked(_id: usize, _type_name: &str) {
    #[cfg(debug_assertions)]
    leak_tracker::warn_if_leaked(_id, _type_name);
}

// ---------------------------------------------------------------------------
// FnDisposable — wraps a closure into a Disposable
// ---------------------------------------------------------------------------

/// A [`Disposable`] backed by a closure that runs exactly once on disposal.
///
/// Created via [`to_disposable`].
pub struct FnDisposable {
    inner: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    disposed: AtomicBool,
    #[cfg(debug_assertions)]
    track_id: usize,
}

impl FnDisposable {
    fn new(f: impl FnOnce() + Send + 'static) -> Self {
        Self {
            inner: Mutex::new(Some(Box::new(f))),
            disposed: AtomicBool::new(false),
            #[cfg(debug_assertions)]
            track_id: track_new("FnDisposable"),
        }
    }
}

impl Disposable for FnDisposable {
    fn dispose(&self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        mark_disposed(self.track_id());
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(f) = guard.take() {
                f();
            }
        }
    }

    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }
}

impl FnDisposable {
    #[inline]
    fn track_id(&self) -> usize {
        #[cfg(debug_assertions)]
        {
            self.track_id
        }
        #[cfg(not(debug_assertions))]
        0
    }
}

impl Drop for FnDisposable {
    fn drop(&mut self) {
        warn_if_leaked(self.track_id(), "FnDisposable");
    }
}

impl fmt::Debug for FnDisposable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FnDisposable")
            .field("disposed", &self.is_disposed())
            .finish()
    }
}

/// Wrap a closure into a [`Disposable`]. The closure runs exactly once when
/// [`dispose()`](Disposable::dispose) is called.
pub fn to_disposable(f: impl FnOnce() + Send + 'static) -> FnDisposable {
    FnDisposable::new(f)
}

// ---------------------------------------------------------------------------
// DisposableStore
// ---------------------------------------------------------------------------

/// Manages a collection of disposable values.
///
/// This is the preferred way to manage multiple disposables. Items added after the store has
/// been disposed are disposed immediately and a warning is emitted to stderr.
///
/// Dropping a `DisposableStore` automatically disposes all contained items.
pub struct DisposableStore {
    inner: Mutex<DisposableStoreInner>,
    disposed: AtomicBool,
    #[cfg(debug_assertions)]
    track_id: usize,
}

struct DisposableStoreInner {
    items: Vec<Box<dyn Disposable + Send>>,
}

impl DisposableStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(DisposableStoreInner { items: Vec::new() }),
            disposed: AtomicBool::new(false),
            #[cfg(debug_assertions)]
            track_id: track_new("DisposableStore"),
        }
    }

    /// Add a disposable to this store.
    ///
    /// If the store has already been disposed, the item is disposed immediately.
    pub fn add<T: Disposable + Send + 'static>(&self, item: T) {
        if self.disposed.load(Ordering::SeqCst) {
            eprintln!(
                "Warning: adding a disposable to an already-disposed DisposableStore. \
                 The item will be disposed immediately."
            );
            item.dispose();
            return;
        }
        if let Ok(mut guard) = self.inner.lock() {
            guard.items.push(Box::new(item));
        }
    }

    /// Dispose all contained items but do **not** mark the store as disposed.
    ///
    /// New items can still be added afterwards.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            let items: Vec<_> = guard.items.drain(..).collect();
            // Drop lock before disposing to avoid potential deadlocks.
            drop(guard);
            for item in items {
                item.dispose();
            }
        }
    }

    /// Returns the number of items currently held.
    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.items.len()).unwrap_or(0)
    }

    /// Returns `true` if the store holds no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for DisposableStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Disposable for DisposableStore {
    fn dispose(&self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        mark_disposed(self.track_id());
        self.clear();
    }

    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }
}

impl DisposableStore {
    #[inline]
    fn track_id(&self) -> usize {
        #[cfg(debug_assertions)]
        {
            self.track_id
        }
        #[cfg(not(debug_assertions))]
        0
    }
}

impl Drop for DisposableStore {
    fn drop(&mut self) {
        // Ensure contained items are disposed when the store is dropped.
        if !self.disposed.load(Ordering::SeqCst) {
            self.dispose();
        }
        warn_if_leaked(self.track_id(), "DisposableStore");
    }
}

impl fmt::Debug for DisposableStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DisposableStore")
            .field("disposed", &self.is_disposed())
            .field("len", &self.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// DisposableMap
// ---------------------------------------------------------------------------

/// A map that manages the lifecycle of the values it stores.
///
/// Setting a key that already exists disposes the previous value. Dropping or disposing
/// the map disposes all contained values.
pub struct DisposableMap<K: Eq + Hash> {
    inner: Mutex<HashMap<K, Box<dyn Disposable + Send>>>,
    disposed: AtomicBool,
    #[cfg(debug_assertions)]
    track_id: usize,
}

impl<K: Eq + Hash> DisposableMap<K> {
    /// Create an empty map.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            disposed: AtomicBool::new(false),
            #[cfg(debug_assertions)]
            track_id: track_new("DisposableMap"),
        }
    }

    /// Insert a value. If the key already exists, the **old** value is disposed.
    ///
    /// If the map has already been disposed, the new value is disposed immediately.
    pub fn set<V: Disposable + Send + 'static>(&self, key: K, value: V) {
        if self.disposed.load(Ordering::SeqCst) {
            eprintln!(
                "Warning: adding a disposable to an already-disposed DisposableMap. \
                 The item will be disposed immediately."
            );
            value.dispose();
            return;
        }
        let old = self
            .inner
            .lock()
            .ok()
            .and_then(|mut g| g.insert(key, Box::new(value)));
        if let Some(old) = old {
            old.dispose();
        }
    }

    /// Returns `true` if the map contains the given key.
    pub fn has(&self, key: &K) -> bool {
        self.inner
            .lock()
            .map(|g| g.contains_key(key))
            .unwrap_or(false)
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove and dispose the value for `key`. No-op if the key doesn't exist.
    pub fn delete_and_dispose(&self, key: &K) {
        let old = self.inner.lock().ok().and_then(|mut g| g.remove(key));
        if let Some(old) = old {
            old.dispose();
        }
    }

    /// Dispose all values and clear the map without marking it as disposed.
    pub fn clear_and_dispose_all(&self) {
        let items: Vec<_> = self
            .inner
            .lock()
            .ok()
            .map(|mut g| g.drain().map(|(_, v)| v).collect())
            .unwrap_or_default();
        for item in items {
            item.dispose();
        }
    }
}

impl<K: Eq + Hash> Default for DisposableMap<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash> Disposable for DisposableMap<K> {
    fn dispose(&self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        mark_disposed(self.track_id());
        self.clear_and_dispose_all();
    }

    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }
}

impl<K: Eq + Hash> DisposableMap<K> {
    #[inline]
    fn track_id(&self) -> usize {
        #[cfg(debug_assertions)]
        {
            self.track_id
        }
        #[cfg(not(debug_assertions))]
        0
    }
}

impl<K: Eq + Hash> Drop for DisposableMap<K> {
    fn drop(&mut self) {
        if !self.disposed.load(Ordering::SeqCst) {
            self.dispose();
        }
        warn_if_leaked(self.track_id(), "DisposableMap");
    }
}

impl<K: Eq + Hash> fmt::Debug for DisposableMap<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DisposableMap")
            .field("disposed", &self.is_disposed())
            .field("len", &self.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// MutableDisposable
// ---------------------------------------------------------------------------

/// Manages the lifecycle of a disposable value that may be changed.
///
/// When a new value is set, the previously held value is disposed. Mirrors VS Code's
/// `MutableDisposable<T>`.
pub struct MutableDisposable {
    inner: Mutex<Option<Box<dyn Disposable + Send>>>,
    disposed: AtomicBool,
    #[cfg(debug_assertions)]
    track_id: usize,
}

impl MutableDisposable {
    /// Create an empty `MutableDisposable`.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            disposed: AtomicBool::new(false),
            #[cfg(debug_assertions)]
            track_id: track_new("MutableDisposable"),
        }
    }

    /// Replace the held value. The previous value (if any) is disposed.
    ///
    /// If this `MutableDisposable` has already been disposed, the new value is disposed
    /// immediately.
    pub fn set<T: Disposable + Send + 'static>(&self, value: T) {
        if self.disposed.load(Ordering::SeqCst) {
            value.dispose();
            return;
        }
        let old = self
            .inner
            .lock()
            .ok()
            .and_then(|mut g| g.replace(Box::new(value)));
        if let Some(old) = old {
            old.dispose();
        }
    }

    /// Clear the held value, disposing it. Equivalent to `set(None)` in VS Code.
    pub fn clear(&self) {
        let old = self.inner.lock().ok().and_then(|mut g| g.take());
        if let Some(old) = old {
            old.dispose();
        }
    }

    /// Clear the held value **without** disposing it, returning ownership to the caller.
    pub fn clear_and_leak(&self) -> Option<Box<dyn Disposable + Send>> {
        self.inner.lock().ok().and_then(|mut g| g.take())
    }

    /// Returns `true` if a value is currently held.
    pub fn has_value(&self) -> bool {
        self.inner
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }
}

impl Default for MutableDisposable {
    fn default() -> Self {
        Self::new()
    }
}

impl Disposable for MutableDisposable {
    fn dispose(&self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        mark_disposed(self.track_id());
        self.clear();
    }

    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }
}

impl MutableDisposable {
    #[inline]
    fn track_id(&self) -> usize {
        #[cfg(debug_assertions)]
        {
            self.track_id
        }
        #[cfg(not(debug_assertions))]
        0
    }
}

impl Drop for MutableDisposable {
    fn drop(&mut self) {
        if !self.disposed.load(Ordering::SeqCst) {
            self.dispose();
        }
        warn_if_leaked(self.track_id(), "MutableDisposable");
    }
}

impl fmt::Debug for MutableDisposable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MutableDisposable")
            .field("disposed", &self.is_disposed())
            .field("has_value", &self.has_value())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// RefCountedDisposable
// ---------------------------------------------------------------------------

/// A reference-counted wrapper around a [`Disposable`].
///
/// Starts with a reference count of 1. Each call to [`acquire`](RefCountedDisposable::acquire)
/// increments the count. Each call to [`release`](RefCountedDisposable::release) decrements it.
/// The inner disposable is disposed when the count reaches zero.
///
/// This type is cheaply cloneable (backed by `Arc`).
pub struct RefCountedDisposable {
    inner: Arc<RefCountedInner>,
}

struct RefCountedInner {
    state: Mutex<RefCountedState>,
}

struct RefCountedState {
    disposable: Option<Box<dyn Disposable + Send>>,
    count: usize,
}

impl RefCountedDisposable {
    /// Wrap a disposable with an initial reference count of 1.
    pub fn new(disposable: impl Disposable + Send + 'static) -> Self {
        Self {
            inner: Arc::new(RefCountedInner {
                state: Mutex::new(RefCountedState {
                    disposable: Some(Box::new(disposable)),
                    count: 1,
                }),
            }),
        }
    }

    /// Increment the reference count and return a clone of this handle.
    pub fn acquire(&self) -> Self {
        if let Ok(mut state) = self.inner.state.lock() {
            state.count = state.count.saturating_add(1);
        }
        self.clone()
    }

    /// Decrement the reference count. If it reaches zero the inner disposable is disposed.
    pub fn release(&self) {
        let should_dispose = self.inner.state.lock().ok().map(|mut state| {
            state.count = state.count.saturating_sub(1);
            state.count == 0
        });
        if should_dispose == Some(true) {
            if let Ok(mut state) = self.inner.state.lock() {
                if let Some(d) = state.disposable.take() {
                    // Drop lock before disposing to avoid potential deadlocks.
                    drop(state);
                    d.dispose();
                }
            }
        }
    }
}

impl Clone for RefCountedDisposable {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Disposable for RefCountedDisposable {
    /// Equivalent to [`release`](RefCountedDisposable::release).
    fn dispose(&self) {
        self.release();
    }

    fn is_disposed(&self) -> bool {
        self.inner
            .state
            .lock()
            .map(|s| s.disposable.is_none())
            .unwrap_or(true)
    }
}

impl fmt::Debug for RefCountedDisposable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (disposed, count) = self
            .inner
            .state
            .lock()
            .map(|s| (s.disposable.is_none(), s.count))
            .unwrap_or((true, 0));
        f.debug_struct("RefCountedDisposable")
            .field("disposed", &disposed)
            .field("ref_count", &count)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Convenience helpers
// ---------------------------------------------------------------------------

/// Dispose all items in a slice. Errors are not propagated; each item is disposed
/// independently.
pub fn dispose_all(items: &[&dyn Disposable]) {
    for item in items {
        item.dispose();
    }
}

/// A [`Disposable`] that does nothing. Equivalent to VS Code's `Disposable.None`.
pub static DISPOSABLE_NONE: DisposableNone = DisposableNone;

/// Zero-sized disposable that does nothing.
#[derive(Debug, Clone, Copy)]
pub struct DisposableNone;

impl Disposable for DisposableNone {
    fn dispose(&self) {}

    fn is_disposed(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// DisposableBatch
// ---------------------------------------------------------------------------

/// A batch container for bulk disposal management.
pub struct DisposableBatch {
    items: Vec<Box<dyn Disposable>>,
    label: String,
}

impl DisposableBatch {
    /// Create a new batch with the given label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            items: Vec::new(),
            label: label.into(),
        }
    }

    /// Add a disposable item to the batch.
    pub fn add(&mut self, item: impl Disposable + 'static) {
        self.items.push(Box::new(item));
    }

    /// Returns the number of items in the batch.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the batch contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Dispose all items and return the count disposed.
    pub fn dispose_all(&mut self) -> usize {
        let count = self.items.len();
        for item in self.items.drain(..) {
            item.dispose();
        }
        count
    }

    /// Dispose the first `n` items and return the actual count disposed.
    pub fn dispose_first_n(&mut self, n: usize) -> usize {
        let actual = n.min(self.items.len());
        for item in self.items.drain(..actual) {
            item.dispose();
        }
        actual
    }
}

// ---------------------------------------------------------------------------
// lifecycle_phase_name
// ---------------------------------------------------------------------------

/// Returns a human-readable name for a lifecycle phase number.
pub fn lifecycle_phase_name(phase: u8) -> &'static str {
    match phase {
        0 => "None",
        1 => "Starting",
        2 => "Ready",
        3 => "ShuttingDown",
        4 => "Disposed",
        _ => "Unknown",
    }
}

// ---------------------------------------------------------------------------
// DisposableGuard
// ---------------------------------------------------------------------------

/// An RAII guard that disposes a [`DisposableStore`] when dropped.
pub struct DisposableGuard {
    store: DisposableStore,
}

impl DisposableGuard {
    /// Create a new guard wrapping the given store.
    pub fn new(store: DisposableStore) -> Self {
        Self { store }
    }

    /// Returns a shared reference to the inner store.
    pub fn store(&self) -> &DisposableStore {
        &self.store
    }

    /// Returns a mutable reference to the inner store.
    pub fn store_mut(&mut self) -> &mut DisposableStore {
        &mut self.store
    }
}

impl Drop for DisposableGuard {
    fn drop(&mut self) {
        self.store.dispose();
    }
}

// ---------------------------------------------------------------------------
// LifecyclePhase – ordered startup/shutdown phases
// ---------------------------------------------------------------------------

/// Ordered lifecycle phases for the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LifecyclePhase {
    Starting,
    Ready,
    Restored,
    Eventually,
    ShuttingDown,
}

impl std::fmt::Display for LifecyclePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Starting => "Starting",
            Self::Ready => "Ready",
            Self::Restored => "Restored",
            Self::Eventually => "Eventually",
            Self::ShuttingDown => "ShuttingDown",
        };
        f.write_str(s)
    }
}

// ---------------------------------------------------------------------------
// LifecycleHook – phase-transition callbacks
// ---------------------------------------------------------------------------

/// A registered callback for a lifecycle phase transition.
pub struct LifecycleHook {
    phase: LifecyclePhase,
    callback: Box<dyn FnOnce() + Send>,
}

impl LifecycleHook {
    pub fn new(phase: LifecyclePhase, callback: impl FnOnce() + Send + 'static) -> Self {
        Self {
            phase,
            callback: Box::new(callback),
        }
    }

    pub fn phase(&self) -> LifecyclePhase {
        self.phase
    }

    /// Consume the hook and run its callback.
    pub fn invoke(self) {
        (self.callback)();
    }
}

impl std::fmt::Debug for LifecycleHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LifecycleHook")
            .field("phase", &self.phase)
            .finish()
    }
}

/// Registry that collects hooks and fires them when phases transition.
pub struct LifecycleHookRegistry {
    hooks: Vec<LifecycleHook>,
    current_phase: LifecyclePhase,
}

impl LifecycleHookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            current_phase: LifecyclePhase::Starting,
        }
    }

    pub fn current_phase(&self) -> LifecyclePhase {
        self.current_phase
    }

    /// Register a hook for a given phase.
    /// If that phase has already passed, the hook fires immediately.
    pub fn register(&mut self, hook: LifecycleHook) {
        if hook.phase() <= self.current_phase {
            hook.invoke();
        } else {
            self.hooks.push(hook);
        }
    }

    /// Advance to the given phase, firing all matching hooks.
    pub fn advance_to(&mut self, phase: LifecyclePhase) {
        if phase <= self.current_phase {
            return;
        }
        self.current_phase = phase;
        let mut remaining = Vec::new();
        for hook in self.hooks.drain(..) {
            if hook.phase() <= phase {
                hook.invoke();
            } else {
                remaining.push(hook);
            }
        }
        self.hooks = remaining;
    }

    /// Number of pending (not-yet-fired) hooks.
    pub fn pending_count(&self) -> usize {
        self.hooks.len()
    }
}

impl Default for LifecycleHookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LifecyclePhaseTiming – duration tracking per phase
// ---------------------------------------------------------------------------

/// Tracks wall-clock duration of each lifecycle phase.
#[derive(Debug, Clone)]
pub struct LifecyclePhaseTiming {
    entries: Vec<(LifecyclePhase, std::time::Instant)>,
}

impl LifecyclePhaseTiming {
    pub fn new() -> Self {
        Self {
            entries: vec![(LifecyclePhase::Starting, std::time::Instant::now())],
        }
    }

    /// Record transition to a new phase.
    pub fn mark(&mut self, phase: LifecyclePhase) {
        self.entries.push((phase, std::time::Instant::now()));
    }

    /// Duration between two adjacent phase entries.
    pub fn duration_of(&self, phase: LifecyclePhase) -> Option<std::time::Duration> {
        let idx = self.entries.iter().position(|(p, _)| *p == phase)?;
        if idx + 1 < self.entries.len() {
            Some(self.entries[idx + 1].1.duration_since(self.entries[idx].1))
        } else {
            // Last entry – elapsed since that mark
            Some(self.entries[idx].1.elapsed())
        }
    }

    /// Total elapsed since first entry.
    pub fn total_elapsed(&self) -> std::time::Duration {
        if let Some(first) = self.entries.first() {
            first.1.elapsed()
        } else {
            std::time::Duration::ZERO
        }
    }

    /// Number of recorded phase transitions.
    pub fn phase_count(&self) -> usize {
        self.entries.len()
    }

    /// The phases in order.
    pub fn phases(&self) -> Vec<LifecyclePhase> {
        self.entries.iter().map(|(p, _)| *p).collect()
    }
}

impl Default for LifecyclePhaseTiming {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HealthCheck – lifecycle health aggregation
// ---------------------------------------------------------------------------

/// Result of a single health check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded(msg) => write!(f, "degraded: {msg}"),
            Self::Unhealthy(msg) => write!(f, "unhealthy: {msg}"),
        }
    }
}

/// A named health check entry.
#[derive(Debug, Clone)]
pub struct HealthCheckEntry {
    pub name: String,
    pub status: HealthStatus,
}

/// Aggregates multiple health checks into a summary.
#[derive(Debug, Clone)]
pub struct HealthCheckAggregator {
    entries: Vec<HealthCheckEntry>,
}

impl HealthCheckAggregator {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, name: impl Into<String>, status: HealthStatus) {
        self.entries.push(HealthCheckEntry {
            name: name.into(),
            status,
        });
    }

    /// Overall status: unhealthy if any unhealthy, degraded if any degraded, else healthy.
    pub fn overall(&self) -> HealthStatus {
        let mut worst = HealthStatus::Healthy;
        for e in &self.entries {
            match &e.status {
                HealthStatus::Unhealthy(msg) => return HealthStatus::Unhealthy(msg.clone()),
                HealthStatus::Degraded(msg) => worst = HealthStatus::Degraded(msg.clone()),
                HealthStatus::Healthy => {}
            }
        }
        worst
    }

    pub fn is_healthy(&self) -> bool {
        self.overall() == HealthStatus::Healthy
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn healthy_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status == HealthStatus::Healthy)
            .count()
    }

    pub fn unhealthy_entries(&self) -> Vec<&HealthCheckEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e.status, HealthStatus::Unhealthy(_)))
            .collect()
    }

    pub fn entries(&self) -> &[HealthCheckEntry] {
        &self.entries
    }
}

impl Default for HealthCheckAggregator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DisposableTimeout – deferred disposal after a duration
// ---------------------------------------------------------------------------

/// Wraps a disposable with a minimum time before it may be disposed.
/// Useful for scheduling cleanup that should not happen immediately.
pub struct DisposableTimeout {
    inner: Option<Box<dyn Disposable + Send>>,
    created_at: std::time::Instant,
    min_age: std::time::Duration,
    disposed: AtomicBool,
}

impl DisposableTimeout {
    /// Create a new timeout wrapper. The inner disposable will only be
    /// disposed once `min_age` has elapsed since creation.
    pub fn new(inner: impl Disposable + Send + 'static, min_age: std::time::Duration) -> Self {
        Self {
            inner: Some(Box::new(inner)),
            created_at: std::time::Instant::now(),
            min_age,
            disposed: AtomicBool::new(false),
        }
    }

    /// Whether enough time has elapsed for disposal.
    pub fn is_ready(&self) -> bool {
        self.created_at.elapsed() >= self.min_age
    }

    /// Try to dispose. Returns `true` if disposal happened, `false` if
    /// it's too early or already disposed.
    pub fn try_dispose(&mut self) -> bool {
        if self.disposed.load(Ordering::Acquire) {
            return false;
        }
        if !self.is_ready() {
            return false;
        }
        if let Some(inner) = self.inner.take() {
            inner.dispose();
        }
        self.disposed.store(true, Ordering::Release);
        true
    }

    /// Elapsed time since creation.
    pub fn age(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }

    /// The minimum age configured for this timeout.
    pub fn min_age(&self) -> std::time::Duration {
        self.min_age
    }
}

impl Disposable for DisposableTimeout {
    fn dispose(&self) {
        // Force-dispose regardless of timing.
        self.disposed.store(true, Ordering::Release);
        // Note: cannot take inner here since &self is immutable.
        // The inner will be dropped when this struct is dropped.
    }

    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::Acquire)
    }
}

// ---------------------------------------------------------------------------
// ShutdownCoordinator – orders shutdown of multiple subsystems
// ---------------------------------------------------------------------------

/// Coordinates an orderly shutdown across named subsystems.
#[derive(Debug)]
pub struct ShutdownCoordinator {
    subsystems: Vec<String>,
    shutdown_complete: Vec<String>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        Self {
            subsystems: Vec::new(),
            shutdown_complete: Vec::new(),
        }
    }

    /// Register a subsystem that needs to be shut down.
    pub fn register(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.subsystems.contains(&name) {
            self.subsystems.push(name);
        }
    }

    /// Mark a subsystem as having completed shutdown.
    pub fn mark_complete(&mut self, name: &str) {
        if self.subsystems.contains(&name.to_string())
            && !self.shutdown_complete.contains(&name.to_string())
        {
            self.shutdown_complete.push(name.to_string());
        }
    }

    /// Whether all registered subsystems have completed shutdown.
    pub fn is_complete(&self) -> bool {
        !self.subsystems.is_empty()
            && self.subsystems.iter().all(|s| self.shutdown_complete.contains(s))
    }

    /// Number of subsystems still pending shutdown.
    pub fn pending_count(&self) -> usize {
        self.subsystems
            .iter()
            .filter(|s| !self.shutdown_complete.contains(s))
            .count()
    }

    /// Total registered subsystems.
    pub fn total(&self) -> usize {
        self.subsystems.len()
    }

    /// Names of subsystems that have not yet completed shutdown.
    pub fn pending_subsystems(&self) -> Vec<&str> {
        self.subsystems
            .iter()
            .filter(|s| !self.shutdown_complete.contains(s))
            .map(|s| s.as_str())
            .collect()
    }

    /// Completion ratio (0.0 – 1.0).
    pub fn progress(&self) -> f64 {
        if self.subsystems.is_empty() {
            return 1.0;
        }
        self.shutdown_complete.len() as f64 / self.subsystems.len() as f64
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ShutdownCoordinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ShutdownCoordinator({}/{} complete)",
            self.shutdown_complete.len(),
            self.subsystems.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Helper: returns a disposable and a counter that tracks how many times dispose ran.
    fn counted_disposable() -> (FnDisposable, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        let d = to_disposable(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        (d, count)
    }

    // -- FnDisposable -------------------------------------------------------

    #[test]
    fn fn_disposable_runs_once() {
        let (d, count) = counted_disposable();
        assert!(!d.is_disposed());
        d.dispose();
        assert!(d.is_disposed());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn fn_disposable_idempotent() {
        let (d, count) = counted_disposable();
        d.dispose();
        d.dispose();
        d.dispose();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // -- DisposableStore ----------------------------------------------------

    #[test]
    fn store_disposes_all_on_dispose() {
        let c1 = Arc::new(AtomicUsize::new(0));
        let c2 = Arc::new(AtomicUsize::new(0));

        let store = DisposableStore::new();
        let cc1 = Arc::clone(&c1);
        store.add(to_disposable(move || {
            cc1.fetch_add(1, Ordering::SeqCst);
        }));
        let cc2 = Arc::clone(&c2);
        store.add(to_disposable(move || {
            cc2.fetch_add(1, Ordering::SeqCst);
        }));

        assert_eq!(store.len(), 2);
        assert!(!store.is_disposed());

        store.dispose();
        assert!(store.is_disposed());
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 1);
        assert!(store.is_empty());
    }

    #[test]
    fn store_dispose_idempotent() {
        let (d, count) = counted_disposable();
        let store = DisposableStore::new();
        store.add(d);
        store.dispose();
        store.dispose();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn store_clear_does_not_mark_disposed() {
        let (d, count) = counted_disposable();
        let store = DisposableStore::new();
        store.add(d);
        store.clear();
        assert!(!store.is_disposed());
        assert_eq!(count.load(Ordering::SeqCst), 1);

        // Can still add after clear.
        let (d2, count2) = counted_disposable();
        store.add(d2);
        store.dispose();
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn store_add_after_dispose_disposes_immediately() {
        let store = DisposableStore::new();
        store.dispose();

        let (d, count) = counted_disposable();
        store.add(d);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn store_drop_disposes_items() {
        let count = Arc::new(AtomicUsize::new(0));
        {
            let store = DisposableStore::new();
            let c = Arc::clone(&count);
            store.add(to_disposable(move || {
                c.fetch_add(1, Ordering::SeqCst);
            }));
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // -- DisposableMap ------------------------------------------------------

    #[test]
    fn map_set_and_dispose() {
        let (d, count) = counted_disposable();
        let map: DisposableMap<&str> = DisposableMap::new();
        map.set("a", d);
        assert!(map.has(&"a"));
        assert_eq!(map.len(), 1);

        map.dispose();
        assert!(map.is_disposed());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn map_replacing_key_disposes_old() {
        let (d1, c1) = counted_disposable();
        let (d2, c2) = counted_disposable();
        let map: DisposableMap<&str> = DisposableMap::new();

        map.set("k", d1);
        map.set("k", d2);

        // Old value disposed.
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        // New value still alive.
        assert_eq!(c2.load(Ordering::SeqCst), 0);

        map.dispose();
        assert_eq!(c2.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn map_delete_and_dispose() {
        let (d, count) = counted_disposable();
        let map: DisposableMap<&str> = DisposableMap::new();
        map.set("x", d);

        map.delete_and_dispose(&"x");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(!map.has(&"x"));
    }

    #[test]
    fn map_clear_and_dispose_all() {
        let (d1, c1) = counted_disposable();
        let (d2, c2) = counted_disposable();
        let map: DisposableMap<i32> = DisposableMap::new();
        map.set(1, d1);
        map.set(2, d2);

        map.clear_and_dispose_all();
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 1);
        assert!(!map.is_disposed());
    }

    #[test]
    fn map_add_after_dispose_disposes_immediately() {
        let map: DisposableMap<&str> = DisposableMap::new();
        map.dispose();

        let (d, count) = counted_disposable();
        map.set("late", d);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // -- MutableDisposable --------------------------------------------------

    #[test]
    fn mutable_set_disposes_old() {
        let (d1, c1) = counted_disposable();
        let (d2, c2) = counted_disposable();
        let m = MutableDisposable::new();

        m.set(d1);
        assert!(m.has_value());
        m.set(d2);
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn mutable_clear() {
        let (d, count) = counted_disposable();
        let m = MutableDisposable::new();
        m.set(d);
        m.clear();
        assert!(!m.has_value());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn mutable_clear_and_leak() {
        let (d, count) = counted_disposable();
        let m = MutableDisposable::new();
        m.set(d);

        let leaked = m.clear_and_leak();
        assert!(leaked.is_some());
        assert_eq!(count.load(Ordering::SeqCst), 0);

        // Manually dispose the leaked value.
        leaked.unwrap().dispose();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn mutable_dispose_idempotent() {
        let (d, count) = counted_disposable();
        let m = MutableDisposable::new();
        m.set(d);
        m.dispose();
        m.dispose();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(m.is_disposed());
    }

    #[test]
    fn mutable_set_after_dispose() {
        let m = MutableDisposable::new();
        m.dispose();

        let (d, count) = counted_disposable();
        m.set(d);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // -- RefCountedDisposable -----------------------------------------------

    #[test]
    fn ref_counted_basic() {
        let (d, count) = counted_disposable();
        let rc = RefCountedDisposable::new(d);
        assert!(!rc.is_disposed());

        rc.release();
        assert!(rc.is_disposed());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ref_counted_acquire_release() {
        let (d, count) = counted_disposable();
        let rc = RefCountedDisposable::new(d);

        let rc2 = rc.acquire();
        let rc3 = rc.acquire();

        // count is now 3
        rc.release();
        assert!(!rc2.is_disposed());
        assert_eq!(count.load(Ordering::SeqCst), 0);

        rc2.release();
        assert!(!rc3.is_disposed());
        assert_eq!(count.load(Ordering::SeqCst), 0);

        rc3.release();
        assert!(rc.is_disposed());
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ref_counted_dispose_alias() {
        let (d, count) = counted_disposable();
        let rc = RefCountedDisposable::new(d);

        // dispose() is an alias for release()
        rc.dispose();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ref_counted_extra_release_is_safe() {
        let (d, count) = counted_disposable();
        let rc = RefCountedDisposable::new(d);

        rc.release();
        rc.release(); // already at zero — should not panic
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // -- DisposableNone -----------------------------------------------------

    #[test]
    fn disposable_none_is_noop() {
        DISPOSABLE_NONE.dispose();
        assert!(!DISPOSABLE_NONE.is_disposed());
    }

    // -- Leak tracking (debug only) -----------------------------------------

    #[cfg(debug_assertions)]
    #[test]
    fn leak_tracking_counts() {
        set_tracking_enabled(true);

        let d = to_disposable(|| {});
        let count_with = tracked_count();
        assert!(count_with > 0);

        d.dispose();
        // After dispose, count should decrease
        assert!(tracked_count() < count_with);

        set_tracking_enabled(false);
    }

    // -- Thread safety ------------------------------------------------------

    #[test]
    fn store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DisposableStore>();
    }

    #[test]
    fn map_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DisposableMap<String>>();
    }

    #[test]
    fn mutable_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MutableDisposable>();
    }

    #[test]
    fn ref_counted_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RefCountedDisposable>();
    }

    #[test]
    fn concurrent_store_usage() {
        let store = Arc::new(DisposableStore::new());
        let count = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let s = Arc::clone(&store);
            let c = Arc::clone(&count);
            handles.push(std::thread::spawn(move || {
                s.add(to_disposable(move || {
                    c.fetch_add(1, Ordering::SeqCst);
                }));
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(store.len(), 10);
        store.dispose();
        assert_eq!(count.load(Ordering::SeqCst), 10);
    }

    // -- DisposableBatch ----------------------------------------------------

    #[test]
    fn test_disposable_batch_add_and_len() {
        let mut batch = DisposableBatch::new("test");
        assert_eq!(batch.len(), 0);
        let (d1, _) = counted_disposable();
        let (d2, _) = counted_disposable();
        batch.add(d1);
        batch.add(d2);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn test_disposable_batch_dispose_all() {
        let mut batch = DisposableBatch::new("test");
        let (d1, c1) = counted_disposable();
        let (d2, c2) = counted_disposable();
        batch.add(d1);
        batch.add(d2);
        let count = batch.dispose_all();
        assert_eq!(count, 2);
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 1);
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn test_disposable_batch_dispose_first_n() {
        let mut batch = DisposableBatch::new("test");
        let (d1, c1) = counted_disposable();
        let (d2, c2) = counted_disposable();
        let (d3, c3) = counted_disposable();
        batch.add(d1);
        batch.add(d2);
        batch.add(d3);
        let count = batch.dispose_first_n(2);
        assert_eq!(count, 2);
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 1);
        assert_eq!(c3.load(Ordering::SeqCst), 0);
        assert_eq!(batch.len(), 1);
        // Request more than available
        let count = batch.dispose_first_n(5);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_disposable_batch_empty() {
        let batch = DisposableBatch::new("empty");
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
    }

    // -- lifecycle_phase_name -----------------------------------------------

    #[test]
    fn test_lifecycle_phase_name_known() {
        assert_eq!(lifecycle_phase_name(0), "None");
        assert_eq!(lifecycle_phase_name(1), "Starting");
        assert_eq!(lifecycle_phase_name(2), "Ready");
        assert_eq!(lifecycle_phase_name(3), "ShuttingDown");
        assert_eq!(lifecycle_phase_name(4), "Disposed");
    }

    #[test]
    fn test_lifecycle_phase_name_unknown() {
        assert_eq!(lifecycle_phase_name(5), "Unknown");
        assert_eq!(lifecycle_phase_name(255), "Unknown");
    }

    // -- DisposableGuard ----------------------------------------------------

    #[test]
    fn test_disposable_guard_drop() {
        let store = DisposableStore::new();
        let (d, count) = counted_disposable();
        store.add(d);
        {
            let _guard = DisposableGuard::new(store);
            assert_eq!(count.load(Ordering::SeqCst), 0);
        }
        // After guard is dropped, the store should be disposed.
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // --- New tests for LifecycleHook, PhaseTiming, HealthCheck ---

    #[test]
    fn lifecycle_hook_registry_advance() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&counter);
        let mut reg = LifecycleHookRegistry::new();
        reg.register(LifecycleHook::new(LifecyclePhase::Ready, move || {
            c.fetch_add(1, Ordering::SeqCst);
        }));
        assert_eq!(reg.pending_count(), 1);
        reg.advance_to(LifecyclePhase::Ready);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(reg.pending_count(), 0);
    }

    #[test]
    fn lifecycle_hook_fires_immediately_if_past() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&counter);
        let mut reg = LifecycleHookRegistry::new();
        reg.advance_to(LifecyclePhase::Restored);
        reg.register(LifecycleHook::new(LifecyclePhase::Ready, move || {
            c.fetch_add(1, Ordering::SeqCst);
        }));
        // Should have fired immediately since Ready < Restored
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(reg.pending_count(), 0);
    }

    #[test]
    fn phase_timing_basic() {
        let mut timing = LifecyclePhaseTiming::new();
        timing.mark(LifecyclePhase::Ready);
        assert_eq!(timing.phase_count(), 2); // Starting + Ready
        assert!(timing.duration_of(LifecyclePhase::Starting).is_some());
        assert!(timing.total_elapsed() >= std::time::Duration::ZERO);
        assert_eq!(timing.phases(), vec![LifecyclePhase::Starting, LifecyclePhase::Ready]);
    }

    #[test]
    fn health_check_aggregator_healthy() {
        let mut agg = HealthCheckAggregator::new();
        agg.add("db", HealthStatus::Healthy);
        agg.add("cache", HealthStatus::Healthy);
        assert!(agg.is_healthy());
        assert_eq!(agg.healthy_count(), 2);
        assert_eq!(agg.entry_count(), 2);
    }

    #[test]
    fn health_check_aggregator_degraded() {
        let mut agg = HealthCheckAggregator::new();
        agg.add("db", HealthStatus::Healthy);
        agg.add("cache", HealthStatus::Degraded("slow".into()));
        assert!(!agg.is_healthy());
        assert!(matches!(agg.overall(), HealthStatus::Degraded(_)));
    }

    #[test]
    fn health_check_aggregator_unhealthy_wins() {
        let mut agg = HealthCheckAggregator::new();
        agg.add("db", HealthStatus::Unhealthy("down".into()));
        agg.add("cache", HealthStatus::Degraded("slow".into()));
        assert!(matches!(agg.overall(), HealthStatus::Unhealthy(_)));
        assert_eq!(agg.unhealthy_entries().len(), 1);
    }

    #[test]
    fn lifecycle_phase_ordering() {
        assert!(LifecyclePhase::Starting < LifecyclePhase::Ready);
        assert!(LifecyclePhase::Ready < LifecyclePhase::Restored);
        assert!(LifecyclePhase::Restored < LifecyclePhase::Eventually);
        assert!(LifecyclePhase::Eventually < LifecyclePhase::ShuttingDown);
    }

    #[test]
    fn lifecycle_phase_display() {
        assert_eq!(format!("{}", LifecyclePhase::Starting), "Starting");
        assert_eq!(format!("{}", LifecyclePhase::ShuttingDown), "ShuttingDown");
    }

    #[test]
    fn shutdown_coordinator_basic_flow() {
        let mut coord = ShutdownCoordinator::new();
        coord.register("editor");
        coord.register("extensions");
        coord.register("terminal");
        assert_eq!(coord.total(), 3);
        assert_eq!(coord.pending_count(), 3);
        assert!(!coord.is_complete());

        coord.mark_complete("editor");
        assert_eq!(coord.pending_count(), 2);
        assert!(!coord.is_complete());

        coord.mark_complete("extensions");
        coord.mark_complete("terminal");
        assert!(coord.is_complete());
        assert_eq!(coord.pending_count(), 0);
    }

    #[test]
    fn shutdown_coordinator_progress() {
        let mut coord = ShutdownCoordinator::new();
        coord.register("a");
        coord.register("b");
        assert!((coord.progress() - 0.0).abs() < f64::EPSILON);
        coord.mark_complete("a");
        assert!((coord.progress() - 0.5).abs() < f64::EPSILON);
        coord.mark_complete("b");
        assert!((coord.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn shutdown_coordinator_pending_subsystems() {
        let mut coord = ShutdownCoordinator::new();
        coord.register("db");
        coord.register("cache");
        coord.mark_complete("db");
        let pending = coord.pending_subsystems();
        assert_eq!(pending, vec!["cache"]);
    }

    #[test]
    fn shutdown_coordinator_display() {
        let mut coord = ShutdownCoordinator::new();
        coord.register("x");
        let display = format!("{coord}");
        assert!(display.contains("0/1"));
        coord.mark_complete("x");
        let display2 = format!("{coord}");
        assert!(display2.contains("1/1"));
    }

    #[test]
    fn shutdown_coordinator_duplicate_register() {
        let mut coord = ShutdownCoordinator::new();
        coord.register("svc");
        coord.register("svc"); // duplicate
        assert_eq!(coord.total(), 1);
        // duplicate mark_complete is safe
        coord.mark_complete("svc");
        coord.mark_complete("svc");
        assert!(coord.is_complete());
    }

    #[test]
    fn disposable_timeout_is_ready() {
        let inner = to_disposable(|| {});
        let timeout = DisposableTimeout::new(inner, std::time::Duration::from_secs(0));
        assert!(timeout.is_ready());
        assert!(!timeout.is_disposed());
    }

    #[test]
    fn disposable_timeout_force_dispose() {
        let flag = Arc::new(AtomicUsize::new(0));
        let f = flag.clone();
        let inner = to_disposable(move || { f.fetch_add(1, Ordering::SeqCst); });
        let timeout = DisposableTimeout::new(inner, std::time::Duration::from_secs(3600));
        assert!(!timeout.is_ready());
        timeout.dispose(); // force dispose
        assert!(timeout.is_disposed());
    }
}
