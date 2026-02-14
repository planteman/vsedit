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
        let before = tracked_count();

        let d = to_disposable(|| {});
        assert_eq!(tracked_count(), before + 1);

        d.dispose();
        assert_eq!(tracked_count(), before);

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
}
