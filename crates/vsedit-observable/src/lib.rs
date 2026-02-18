//! Reactive observable system.
//!
//! Provides `IObservable<T>` and related types, equivalent to
//! VS Code's `vs/base/common/observable.ts`.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use vsedit_events::{DisposableHandle, Emitter};

// ---------------------------------------------------------------------------
// ObservableError
// ---------------------------------------------------------------------------

/// Errors that can occur in the observable system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservableError {
    /// A mutex was poisoned (a thread panicked while holding the lock).
    LockPoisoned,
    /// The observable has already been disposed.
    AlreadyDisposed,
}

impl fmt::Display for ObservableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObservableError::LockPoisoned => write!(f, "lock poisoned"),
            ObservableError::AlreadyDisposed => write!(f, "already disposed"),
        }
    }
}

impl std::error::Error for ObservableError {}

/// A reactive observable value that notifies subscribers on change.
pub struct ObservableValue<T: Clone + PartialEq + Send + Sync + 'static> {
    value: Arc<Mutex<T>>,
    on_change: Emitter<T>,
}

impl<T: Clone + PartialEq + Send + Sync + 'static> ObservableValue<T> {
    /// Create a new observable with an initial value.
    pub fn new(initial: T) -> Self {
        Self {
            value: Arc::new(Mutex::new(initial)),
            on_change: Emitter::new(),
        }
    }

    /// Get the current value.
    pub fn get(&self) -> T {
        self.value.lock().unwrap().clone()
    }

    /// Set the value. Fires the change event if the value changed.
    pub fn set(&self, new_value: T) {
        let changed = {
            let mut v = self.value.lock().unwrap();
            if *v != new_value {
                *v = new_value.clone();
                true
            } else {
                false
            }
        };
        if changed {
            self.on_change.fire(&new_value);
        }
    }

    /// Set the value without firing any change events.
    pub fn set_silent(&self, new_value: T) {
        let mut v = self.value.lock().unwrap();
        *v = new_value;
    }

    /// Atomically swap the value and return the old one.
    /// Fires the change event if the new value differs from the old.
    pub fn swap(&self, new_value: T) -> T {
        let old = {
            let mut v = self.value.lock().unwrap();
            let old = v.clone();
            *v = new_value.clone();
            old
        };
        if old != new_value {
            self.on_change.fire(&new_value);
        }
        old
    }

    /// Modify the value in-place via a closure that receives `&mut T`.
    /// Always fires the change event after the closure returns.
    pub fn modify(&self, f: impl FnOnce(&mut T)) {
        let snapshot = {
            let mut v = self.value.lock().unwrap();
            f(&mut v);
            v.clone()
        };
        self.on_change.fire(&snapshot);
    }

    /// Subscribe to value changes. Returns a handle that unsubscribes on drop.
    pub fn on_change(
        &self,
        listener: impl Fn(&T) + Send + Sync + 'static,
    ) -> DisposableHandle {
        self.on_change.event().on(listener)
    }

    /// Get the current value and subscribe in one call.
    /// Returns `(current_value, DisposableHandle)`.
    pub fn get_and_subscribe(
        &self,
        listener: impl Fn(&T) + Send + Sync + 'static,
    ) -> (T, DisposableHandle) {
        let current = self.get();
        let handle = self.on_change(listener);
        (current, handle)
    }

    /// Update the value using a function.
    pub fn update(&self, f: impl FnOnce(&T) -> T) {
        let new_value = {
            let v = self.value.lock().unwrap();
            f(&v)
        };
        self.set(new_value);
    }

    /// Map this observable through a function, creating a derived observable.
    pub fn map<U: Clone + PartialEq + Send + Sync + 'static>(
        &self,
        f: impl Fn(&T) -> U + Send + Sync + 'static,
    ) -> DerivedObservable<U> {
        let initial = f(&self.get());
        let derived = Arc::new(ObservableValue::new(initial));
        let derived_ref = derived.clone();
        let handle = self.on_change(move |val| {
            let new_val = f(val);
            derived_ref.set(new_val);
        });
        DerivedObservable {
            inner: derived,
            _subscription: handle,
        }
    }
}

impl<T: Clone + PartialEq + Send + Sync + fmt::Display + 'static> fmt::Display
    for ObservableValue<T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Observable({})", self.get())
    }
}

/// A derived observable that is computed from another observable.
pub struct DerivedObservable<T: Clone + PartialEq + Send + Sync + 'static> {
    inner: Arc<ObservableValue<T>>,
    _subscription: DisposableHandle,
}

impl<T: Clone + PartialEq + Send + Sync + 'static> DerivedObservable<T> {
    #[allow(dead_code)]
    fn new(initial: T) -> Self {
        // Create a no-op subscription for standalone derived observables
        let emitter = Emitter::<()>::new();
        let handle = emitter.event().on(|_| {});
        Self {
            inner: Arc::new(ObservableValue::new(initial)),
            _subscription: handle,
        }
    }

    /// Get the current derived value.
    pub fn get(&self) -> T {
        self.inner.get()
    }

    /// Subscribe to changes.
    pub fn on_change(
        &self,
        listener: impl Fn(&T) + Send + Sync + 'static,
    ) -> DisposableHandle {
        self.inner.on_change(listener)
    }
}

// ---------------------------------------------------------------------------
// ObservableList
// ---------------------------------------------------------------------------

/// An observable list that fires change events on mutations.
pub struct ObservableList<T: Clone + Send + Sync + 'static> {
    items: Arc<Mutex<Vec<T>>>,
    on_change: Emitter<Vec<T>>,
}

impl<T: Clone + Send + Sync + 'static> ObservableList<T> {
    /// Create a new empty observable list.
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
            on_change: Emitter::new(),
        }
    }

    /// Push an item and fire a change event.
    pub fn push(&self, item: T) {
        let snapshot = {
            let mut items = self.items.lock().unwrap();
            items.push(item);
            items.clone()
        };
        self.on_change.fire(&snapshot);
    }

    /// Remove the item at `index` and fire a change event.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn remove_at(&self, index: usize) -> T {
        let (removed, snapshot) = {
            let mut items = self.items.lock().unwrap();
            let removed = items.remove(index);
            (removed, items.clone())
        };
        self.on_change.fire(&snapshot);
        removed
    }

    /// Get a clone of the item at `index`, or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<T> {
        self.items.lock().unwrap().get(index).cloned()
    }

    /// Return the number of items in the list.
    pub fn len(&self) -> usize {
        self.items.lock().unwrap().len()
    }

    /// Return `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.lock().unwrap().is_empty()
    }

    /// Remove all items and fire a change event.
    pub fn clear(&self) {
        {
            self.items.lock().unwrap().clear();
        }
        self.on_change.fire(&Vec::new());
    }

    /// Subscribe to list changes. The listener receives the full list snapshot.
    pub fn on_change(
        &self,
        listener: impl Fn(&Vec<T>) + Send + Sync + 'static,
    ) -> DisposableHandle {
        self.on_change.event().on(listener)
    }

    /// Insert an item at a specific index and fire change event.
    pub fn insert_at(&self, index: usize, item: T) {
        let snapshot = {
            let mut items = self.items.lock().unwrap();
            items.insert(index, item);
            items.clone()
        };
        self.on_change.fire(&snapshot);
    }

    /// Replace the item at the given index, fire a change event, and return the old value.
    pub fn set(&self, index: usize, item: T) -> Option<T> {
        let (old, snapshot) = {
            let mut items = self.items.lock().unwrap();
            if index >= items.len() {
                return None;
            }
            let old = std::mem::replace(&mut items[index], item);
            (Some(old), items.clone())
        };
        self.on_change.fire(&snapshot);
        old
    }

    /// Return a clone of all items.
    pub fn to_vec(&self) -> Vec<T> {
        self.items.lock().unwrap().clone()
    }

    /// Retain only items matching the predicate. Fires change event.
    pub fn retain(&self, f: impl Fn(&T) -> bool) {
        let snapshot = {
            let mut items = self.items.lock().unwrap();
            items.retain(|item| f(item));
            items.clone()
        };
        self.on_change.fire(&snapshot);
    }
}

impl<T: Clone + Send + Sync + 'static> Default for ObservableList<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for observable operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservableStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ObservableStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ObservableStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ObservableStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ObservableStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ObservableStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

// ---------------------------------------------------------------------------
// ObservableValue – utility helpers & predicates
// ---------------------------------------------------------------------------

impl<T: Clone + PartialEq + Send + Sync + 'static> ObservableValue<T> {
    /// Replace the value only if `predicate` returns `true` for the current value.
    /// Returns `true` if the replacement occurred.
    pub fn set_if(&self, predicate: impl FnOnce(&T) -> bool, new_value: T) -> bool {
        let should_set = {
            let v = self.value.lock().unwrap();
            predicate(&v)
        };
        if should_set {
            self.set(new_value);
            true
        } else {
            false
        }
    }

    /// Take the current value and replace it with `replacement`, returning the old value.
    /// Always fires the change event if old != replacement.
    pub fn take(&self, replacement: T) -> T {
        self.swap(replacement)
    }

    /// Return a snapshot tuple of `(value, listener_handle)` where the listener
    /// records every change into the returned `Arc<Mutex<Vec<T>>>`.
    pub fn spy(&self) -> (T, Arc<Mutex<Vec<T>>>, DisposableHandle) {
        let log = Arc::new(Mutex::new(Vec::<T>::new()));
        let log2 = log.clone();
        let current = self.get();
        let handle = self.on_change(move |val| {
            log2.lock().unwrap().push(val.clone());
        });
        (current, log, handle)
    }
}

impl<T: Clone + PartialEq + Send + Sync + Default + 'static> ObservableValue<T> {
    /// Reset the value to `T::default()`, firing the change event if it differs.
    pub fn reset(&self) {
        self.set(T::default());
    }
}

// ---------------------------------------------------------------------------
// ObservableList – utility helpers
// ---------------------------------------------------------------------------

impl<T: Clone + Send + Sync + 'static> ObservableList<T> {
    /// Create an observable list from an existing vector.
    pub fn from_vec(items: Vec<T>) -> Self {
        Self {
            items: Arc::new(Mutex::new(items)),
            on_change: Emitter::new(),
        }
    }

    /// Return the first item, if any.
    pub fn first(&self) -> Option<T> {
        self.items.lock().unwrap().first().cloned()
    }

    /// Return the last item, if any.
    pub fn last(&self) -> Option<T> {
        self.items.lock().unwrap().last().cloned()
    }

    /// Apply a mapping function to every item in the list and return the results.
    pub fn map_items<R>(&self, f: impl Fn(&T) -> R) -> Vec<R> {
        self.items.lock().unwrap().iter().map(f).collect()
    }

    /// Extend the list with items from an iterator. Fires a single change event.
    pub fn extend(&self, iter: impl IntoIterator<Item = T>) {
        let snapshot = {
            let mut items = self.items.lock().unwrap();
            items.extend(iter);
            items.clone()
        };
        self.on_change.fire(&snapshot);
    }

    /// Remove and return the last item. Fires change event if an item was removed.
    pub fn pop(&self) -> Option<T> {
        let (popped, snapshot) = {
            let mut items = self.items.lock().unwrap();
            let popped = items.pop();
            (popped, items.clone())
        };
        if popped.is_some() {
            self.on_change.fire(&snapshot);
        }
        popped
    }
}

impl<T: Clone + PartialEq + Send + Sync + 'static> ObservableList<T> {
    /// Return the index of the first item equal to `value`, or `None`.
    pub fn index_of(&self, value: &T) -> Option<usize> {
        self.items.lock().unwrap().iter().position(|x| x == value)
    }

    /// Return `true` if the list contains the given value.
    pub fn contains(&self, value: &T) -> bool {
        self.items.lock().unwrap().contains(value)
    }
}

// ---------------------------------------------------------------------------
// ObservableMap – utility helpers
// ---------------------------------------------------------------------------

impl<K: Clone + Eq + std::hash::Hash + Send + Sync + 'static, V: Clone + Send + Sync + 'static>
    ObservableMap<K, V>
{
    /// Return all values (without keys).
    pub fn values(&self) -> Vec<V> {
        self.entries.lock().unwrap().values().cloned().collect()
    }

    /// Return all entries as a vector of `(key, value)` pairs.
    pub fn to_vec(&self) -> Vec<(K, V)> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get or insert a default value for the given key. Returns the value.
    pub fn get_or_insert(&self, key: K, default: V) -> V {
        {
            let map = self.entries.lock().unwrap();
            if let Some(v) = map.get(&key) {
                return v.clone();
            }
        }
        self.insert(key.clone(), default);
        self.entries.lock().unwrap().get(&key).unwrap().clone()
    }
}

// ---------------------------------------------------------------------------
// ObservableStats – additional analytics
// ---------------------------------------------------------------------------

impl ObservableStats {
    /// Return the number of successful operations.
    pub fn successes(&self) -> u64 {
        self.successful_operations
    }

    /// Return the number of failed operations.
    pub fn failures(&self) -> u64 {
        self.failed_operations
    }

    /// Return `true` if no failures have been recorded.
    pub fn is_all_success(&self) -> bool {
        self.failed_operations == 0
    }

    /// Return total elapsed time in nanoseconds.
    pub fn total_time_ns(&self) -> u64 {
        self.total_time_ns
    }
}

// ---------------------------------------------------------------------------
// ObservableHistory – additional utilities
// ---------------------------------------------------------------------------

impl<T: Clone> ObservableHistory<T> {
    /// Return the first recorded value, if any.
    pub fn first(&self) -> Option<&T> {
        self.entries.first().map(|e| &e.value)
    }

    /// Return a Vec of all recorded values (without sequence metadata).
    pub fn values(&self) -> Vec<&T> {
        self.entries.iter().map(|e| &e.value).collect()
    }

    /// Return the current next-sequence counter.
    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

impl<T: Clone + PartialEq> ObservableHistory<T> {
    /// Return `true` if the given value was ever recorded.
    pub fn contains(&self, value: &T) -> bool {
        self.entries.iter().any(|e| e.value == *value)
    }
}

// ---------------------------------------------------------------------------
// ObservableDebouncer – additional utilities
// ---------------------------------------------------------------------------

impl<T: Clone + PartialEq> ObservableDebouncer<T> {
    /// Create a debouncer with an initial committed value.
    pub fn with_initial(value: T) -> Self {
        Self {
            pending: None,
            committed: Some(value),
            change_count: 0,
        }
    }

    /// Stage and immediately commit. Returns `true` if the value changed.
    pub fn set(&mut self, value: T) -> bool {
        self.stage(value);
        self.commit()
    }

    /// Return `true` if a value has been committed at least once.
    pub fn has_committed(&self) -> bool {
        self.committed.is_some()
    }

    /// Reset the debouncer to its initial empty state.
    pub fn reset(&mut self) {
        self.pending = None;
        self.committed = None;
        self.change_count = 0;
    }
}

/// Validation utilities for observable.
#[derive(Debug, Clone)]
pub struct ObservableValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ObservableValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ObservableValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ObservableMap
// ---------------------------------------------------------------------------

/// A reactive key-value map that fires change events on mutations.
pub struct ObservableMap<K: Clone + Eq + std::hash::Hash + Send + Sync + 'static, V: Clone + Send + Sync + 'static> {
    entries: Arc<Mutex<std::collections::HashMap<K, V>>>,
    on_change: Emitter<Vec<(K, V)>>,
}

impl<K: Clone + Eq + std::hash::Hash + Send + Sync + 'static, V: Clone + Send + Sync + 'static> ObservableMap<K, V> {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(std::collections::HashMap::new())),
            on_change: Emitter::new(),
        }
    }

    /// Insert or update a key-value pair. Fires change event.
    pub fn insert(&self, key: K, value: V) {
        let snapshot = {
            let mut map = self.entries.lock().unwrap();
            map.insert(key, value);
            map.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>()
        };
        self.on_change.fire(&snapshot);
    }

    /// Remove a key. Fires change event. Returns the removed value.
    pub fn remove(&self, key: &K) -> Option<V> {
        let (removed, snapshot) = {
            let mut map = self.entries.lock().unwrap();
            let removed = map.remove(key);
            let snapshot = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>();
            (removed, snapshot)
        };
        if removed.is_some() {
            self.on_change.fire(&snapshot);
        }
        removed
    }

    /// Get a clone of the value for the given key.
    pub fn get(&self, key: &K) -> Option<V> {
        self.entries.lock().unwrap().get(key).cloned()
    }

    /// Check if the map contains the key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.lock().unwrap().contains_key(key)
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    /// Return true if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().is_empty()
    }

    /// Get all keys.
    pub fn keys(&self) -> Vec<K> {
        self.entries.lock().unwrap().keys().cloned().collect()
    }

    /// Clear the map. Fires change event.
    pub fn clear(&self) {
        {
            self.entries.lock().unwrap().clear();
        }
        self.on_change.fire(&Vec::new());
    }

    /// Subscribe to map changes. The listener receives a snapshot of all entries.
    pub fn on_change(
        &self,
        listener: impl Fn(&Vec<(K, V)>) + Send + Sync + 'static,
    ) -> DisposableHandle {
        self.on_change.event().on(listener)
    }
}

impl<K: Clone + Eq + std::hash::Hash + Send + Sync + 'static, V: Clone + Send + Sync + 'static> Default for ObservableMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// derived_from
// ---------------------------------------------------------------------------

/// Create a derived observable that maps a single observable value through a function.
/// This is equivalent to `ObservableValue::map` but as a free function.
pub fn derived_from<T, R>(
    source: &ObservableValue<T>,
    f: impl Fn(&T) -> R + Send + Sync + 'static,
) -> DerivedObservable<R>
where
    T: Clone + PartialEq + Send + Sync + 'static,
    R: Clone + PartialEq + Send + Sync + 'static,
{
    source.map(f)
}

// ---------------------------------------------------------------------------
// observable_combine
// ---------------------------------------------------------------------------

/// Compute a one-shot combined value from two observables.
pub fn observable_combine<A, B, C>(
    a: &ObservableValue<A>,
    b: &ObservableValue<B>,
    f: impl Fn(&A, &B) -> C,
) -> C
where
    A: Clone + PartialEq + Send + Sync + 'static,
    B: Clone + PartialEq + Send + Sync + 'static,
{
    let va = a.get();
    let vb = b.get();
    f(&va, &vb)
}

// ---------------------------------------------------------------------------
// ObservableTransaction
// ---------------------------------------------------------------------------

/// Batches multiple set operations and applies them atomically on commit.
pub struct ObservableTransaction {
    actions: Vec<Box<dyn FnOnce()>>,
}

impl ObservableTransaction {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Queue a deferred mutation.
    pub fn defer(&mut self, action: impl FnOnce() + 'static) {
        self.actions.push(Box::new(action));
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Execute all deferred actions in order.
    pub fn commit(self) {
        for action in self.actions {
            action();
        }
    }

    /// Drop all deferred actions without executing.
    pub fn rollback(self) {
        drop(self.actions);
    }
}

impl Default for ObservableTransaction {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ObservableHistory
// ---------------------------------------------------------------------------

/// A single recorded value change.
#[derive(Debug, Clone)]
pub struct HistoryEntry<T> {
    pub value: T,
    pub sequence: u64,
}

/// Records value changes with incrementing sequence numbers.
pub struct ObservableHistory<T> {
    pub entries: Vec<HistoryEntry<T>>,
    next_sequence: u64,
}

impl<T> ObservableHistory<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_sequence: 0,
        }
    }

    /// Record a value with an incrementing sequence number.
    pub fn record(&mut self, value: T) {
        self.entries.push(HistoryEntry {
            value,
            sequence: self.next_sequence,
        });
        self.next_sequence += 1;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the most recently recorded value, if any.
    pub fn latest(&self) -> Option<&T> {
        self.entries.last().map(|e| &e.value)
    }

    /// Return the value at the given index, if it exists.
    pub fn at(&self, index: usize) -> Option<&T> {
        self.entries.get(index).map(|e| &e.value)
    }

    /// Remove all recorded entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.next_sequence = 0;
    }
}

impl<T> Default for ObservableHistory<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ObservableSet
// ---------------------------------------------------------------------------

/// A reactive set that fires change events when elements are added or removed.
pub struct ObservableSet<T: Clone + Eq + std::hash::Hash + Send + Sync + 'static> {
    items: Arc<Mutex<std::collections::HashSet<T>>>,
    on_change: Emitter<Vec<T>>,
}

impl<T: Clone + Eq + std::hash::Hash + Send + Sync + 'static> ObservableSet<T> {
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(std::collections::HashSet::new())),
            on_change: Emitter::new(),
        }
    }

    /// Insert an element. Returns `true` if it was newly inserted.
    pub fn insert(&self, value: T) -> bool {
        let (inserted, snapshot) = {
            let mut set = self.items.lock().unwrap();
            let inserted = set.insert(value);
            let snapshot: Vec<T> = set.iter().cloned().collect();
            (inserted, snapshot)
        };
        if inserted {
            self.on_change.fire(&snapshot);
        }
        inserted
    }

    /// Remove an element. Returns `true` if it was present.
    pub fn remove(&self, value: &T) -> bool {
        let (removed, snapshot) = {
            let mut set = self.items.lock().unwrap();
            let removed = set.remove(value);
            let snapshot: Vec<T> = set.iter().cloned().collect();
            (removed, snapshot)
        };
        if removed {
            self.on_change.fire(&snapshot);
        }
        removed
    }

    /// Check if the set contains a value.
    pub fn contains(&self, value: &T) -> bool {
        self.items.lock().unwrap().contains(value)
    }

    /// Return the number of elements.
    pub fn len(&self) -> usize {
        self.items.lock().unwrap().len()
    }

    /// Return `true` if empty.
    pub fn is_empty(&self) -> bool {
        self.items.lock().unwrap().is_empty()
    }

    /// Return all items as a vector.
    pub fn to_vec(&self) -> Vec<T> {
        self.items.lock().unwrap().iter().cloned().collect()
    }

    /// Clear the set.
    pub fn clear(&self) {
        self.items.lock().unwrap().clear();
        self.on_change.fire(&Vec::new());
    }

    /// Subscribe to changes.
    pub fn on_change(
        &self,
        listener: impl Fn(&Vec<T>) + Send + Sync + 'static,
    ) -> DisposableHandle {
        self.on_change.event().on(listener)
    }
}

impl<T: Clone + Eq + std::hash::Hash + Send + Sync + 'static> Default for ObservableSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ObservableProjection
// ---------------------------------------------------------------------------

/// Projects/maps an observable value through a transformation function.
pub struct ObservableProjection<T, R>
where
    T: Clone + PartialEq + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
{
    source_value: Arc<Mutex<T>>,
    projected: Arc<Mutex<R>>,
    transform: Arc<dyn Fn(&T) -> R + Send + Sync>,
}

impl<T, R> ObservableProjection<T, R>
where
    T: Clone + PartialEq + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
{
    /// Create a new projection from an initial source value and a transform.
    pub fn new(initial: T, transform: impl Fn(&T) -> R + Send + Sync + 'static) -> Self {
        let projected = transform(&initial);
        Self {
            source_value: Arc::new(Mutex::new(initial)),
            projected: Arc::new(Mutex::new(projected)),
            transform: Arc::new(transform),
        }
    }

    /// Update the source value and recompute the projection.
    pub fn update(&self, value: T) {
        let new_projected = (self.transform)(&value);
        *self.source_value.lock().unwrap() = value;
        *self.projected.lock().unwrap() = new_projected;
    }

    /// Get the current projected value.
    pub fn get(&self) -> R {
        self.projected.lock().unwrap().clone()
    }

    /// Get the current source value.
    pub fn source(&self) -> T {
        self.source_value.lock().unwrap().clone()
    }
}

// ---------------------------------------------------------------------------
// ObservableHistory – branching & checkpoints
// ---------------------------------------------------------------------------

impl<T: Clone> ObservableHistory<T> {
    /// Create a named checkpoint at the current position.
    /// Returns the index that was checkpointed.
    pub fn checkpoint(&self) -> usize {
        self.entries.len()
    }

    /// Truncate history back to a checkpoint index, discarding later entries.
    pub fn restore_checkpoint(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.truncate(index);
            self.next_sequence = index as u64;
        }
    }

    /// Fork: clone entries up to `index` into a new history.
    pub fn fork(&self, up_to: usize) -> ObservableHistory<T> {
        let entries: Vec<HistoryEntry<T>> = self
            .entries
            .iter()
            .take(up_to)
            .map(|e| HistoryEntry {
                value: e.value.clone(),
                sequence: e.sequence,
            })
            .collect();
        let next_seq = entries.len() as u64;
        ObservableHistory {
            entries,
            next_sequence: next_seq,
        }
    }

    /// Return entries recorded since the given sequence number.
    pub fn since(&self, sequence: u64) -> Vec<&T> {
        self.entries
            .iter()
            .filter(|e| e.sequence >= sequence)
            .map(|e| &e.value)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ObservableDebouncer
// ---------------------------------------------------------------------------

/// Tracks a pending value and only applies it after a settle period (logical debounce).
pub struct ObservableDebouncer<T: Clone + PartialEq> {
    pending: Option<T>,
    committed: Option<T>,
    change_count: u64,
}

impl<T: Clone + PartialEq> ObservableDebouncer<T> {
    pub fn new() -> Self {
        Self {
            pending: None,
            committed: None,
            change_count: 0,
        }
    }

    /// Stage a new value (not yet committed).
    pub fn stage(&mut self, value: T) {
        self.pending = Some(value);
    }

    /// Commit the pending value if it differs from the last committed value.
    /// Returns `true` if a new value was committed.
    pub fn commit(&mut self) -> bool {
        if let Some(pending) = self.pending.take() {
            if self.committed.as_ref() != Some(&pending) {
                self.committed = Some(pending);
                self.change_count += 1;
                return true;
            }
        }
        false
    }

    /// Get the committed value.
    pub fn committed(&self) -> Option<&T> {
        self.committed.as_ref()
    }

    /// Get the pending value.
    pub fn pending(&self) -> Option<&T> {
        self.pending.as_ref()
    }

    /// How many times a value has been committed.
    pub fn change_count(&self) -> u64 {
        self.change_count
    }

    /// Returns `true` if there is a staged value waiting to be committed.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Discard any pending value without committing.
    pub fn discard(&mut self) {
        self.pending = None;
    }
}

impl<T: Clone + PartialEq> Default for ObservableDebouncer<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MapChange
// ---------------------------------------------------------------------------

/// Describes a single change to an `ObservableTrackedMap`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapChange {
    /// A key was inserted with the given value.
    Insert(String, String),
    /// A key was updated from the old value to the new value.
    Update(String, String, String),
    /// A key was removed; the removed value is included.
    Remove(String, String),
}

impl fmt::Display for MapChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapChange::Insert(k, v) => write!(f, "Insert({k}, {v})"),
            MapChange::Update(k, old, new) => write!(f, "Update({k}, {old} -> {new})"),
            MapChange::Remove(k, v) => write!(f, "Remove({k}, {v})"),
        }
    }
}

// ---------------------------------------------------------------------------
// ObservableTrackedMap
// ---------------------------------------------------------------------------

/// A key-value map that records per-key change history.
pub struct ObservableTrackedMap {
    entries: std::collections::HashMap<String, String>,
    change_log: Vec<MapChange>,
}

impl ObservableTrackedMap {
    /// Create a new empty tracked map.
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            change_log: Vec::new(),
        }
    }

    /// Insert or update a key-value pair, recording the change.
    pub fn insert(&mut self, key: String, value: String) {
        if let Some(old) = self.entries.insert(key.clone(), value.clone()) {
            self.change_log
                .push(MapChange::Update(key, old, value));
        } else {
            self.change_log
                .push(MapChange::Insert(key, value));
        }
    }

    /// Get a reference to the value for a key.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.entries.get(key)
    }

    /// Remove a key, recording the removal. Returns the removed value.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(old) = self.entries.remove(key) {
            self.change_log
                .push(MapChange::Remove(key.to_string(), old.clone()));
            Some(old)
        } else {
            None
        }
    }

    /// Check whether the map contains a key.
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// Return all keys.
    pub fn keys(&self) -> Vec<&String> {
        self.entries.keys().collect()
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the recorded change log.
    pub fn changes(&self) -> &[MapChange] {
        &self.change_log
    }
}

impl Default for ObservableTrackedMap {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ObservableTrackedMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ObservableTrackedMap(entries={}, changes={})",
            self.entries.len(),
            self.change_log.len()
        )
    }
}

// ---------------------------------------------------------------------------
// CombinerKind / ObservableDerived
// ---------------------------------------------------------------------------

/// The combining strategy for an `ObservableDerived`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombinerKind {
    Sum,
    Product,
    Min,
    Max,
}

impl fmt::Display for CombinerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CombinerKind::Sum => write!(f, "Sum"),
            CombinerKind::Product => write!(f, "Product"),
            CombinerKind::Min => write!(f, "Min"),
            CombinerKind::Max => write!(f, "Max"),
        }
    }
}

/// A computed value derived from multiple `i64` sources using a combiner.
pub struct ObservableDerived {
    sources: Vec<i64>,
    combiner: CombinerKind,
}

impl ObservableDerived {
    /// Create a derived observable from source values and a combiner name.
    ///
    /// Recognised names: `"sum"`, `"product"`, `"min"`, `"max"`.
    /// Defaults to `Sum` for unrecognised names.
    pub fn from_values(values: Vec<i64>, combiner_name: &str) -> Self {
        let combiner = match combiner_name {
            "sum" => CombinerKind::Sum,
            "product" => CombinerKind::Product,
            "min" => CombinerKind::Min,
            "max" => CombinerKind::Max,
            _ => CombinerKind::Sum,
        };
        Self {
            sources: values,
            combiner,
        }
    }

    /// Compute and return the derived value.
    pub fn get(&self) -> i64 {
        if self.sources.is_empty() {
            return 0;
        }
        match self.combiner {
            CombinerKind::Sum => self.sources.iter().sum(),
            CombinerKind::Product => self.sources.iter().product(),
            CombinerKind::Min => self.sources.iter().copied().min().unwrap_or(0),
            CombinerKind::Max => self.sources.iter().copied().max().unwrap_or(0),
        }
    }

    /// Update a single source value by index.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn update_source(&mut self, index: usize, value: i64) {
        self.sources[index] = value;
    }

    /// Return the number of sources.
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
}

impl fmt::Display for ObservableDerived {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ObservableDerived({}, sources={}, value={})",
            self.combiner,
            self.sources.len(),
            self.get()
        )
    }
}

// ---------------------------------------------------------------------------
// BatchedChange / ObservableBatch
// ---------------------------------------------------------------------------

/// A single change recorded during a batch operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchedChange {
    pub key: String,
    pub value: String,
}

impl fmt::Display for BatchedChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Collects multiple changes and delivers them as a single batch.
pub struct ObservableBatch {
    batching: bool,
    pending: Vec<BatchedChange>,
}

impl ObservableBatch {
    /// Create a new batch collector.
    pub fn new() -> Self {
        Self {
            batching: false,
            pending: Vec::new(),
        }
    }

    /// Begin accumulating changes.
    pub fn begin_batch(&mut self) {
        self.batching = true;
        self.pending.clear();
    }

    /// Record a change while batching is active.
    pub fn add_change(&mut self, key: &str, value: &str) {
        if self.batching {
            self.pending.push(BatchedChange {
                key: key.to_string(),
                value: value.to_string(),
            });
        }
    }

    /// End the batch and return all accumulated changes.
    pub fn end_batch(&mut self) -> Vec<BatchedChange> {
        self.batching = false;
        std::mem::take(&mut self.pending)
    }

    /// Return `true` if currently inside a batch.
    pub fn is_batching(&self) -> bool {
        self.batching
    }

    /// Return the number of pending changes.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for ObservableBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ObservableBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ObservableBatch(batching={}, pending={})",
            self.batching,
            self.pending.len()
        )
    }
}

// ---------------------------------------------------------------------------
// ObservableReplay
// ---------------------------------------------------------------------------

/// A bounded buffer that records emitted values for replay to late subscribers.
pub struct ObservableReplay {
    capacity: usize,
    buffer: Vec<String>,
}

impl ObservableReplay {
    /// Create a replay buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: Vec::with_capacity(capacity),
        }
    }

    /// Emit a value into the buffer, evicting the oldest if full.
    pub fn emit(&mut self, value: String) {
        if self.buffer.len() >= self.capacity {
            self.buffer.remove(0);
        }
        self.buffer.push(value);
    }

    /// Replay all buffered values.
    pub fn replay(&self) -> Vec<&str> {
        self.buffer.iter().map(|s| s.as_str()).collect()
    }

    /// Return the most recently emitted value.
    pub fn latest(&self) -> Option<&str> {
        self.buffer.last().map(|s| s.as_str())
    }

    /// Return the number of buffered values.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Return `true` if the buffer has reached capacity.
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl fmt::Display for ObservableReplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ObservableReplay(len={}, capacity={})",
            self.buffer.len(),
            self.capacity
        )
    }
}


// ─── ObsC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for observable snapshots.
#[derive(Debug)]
pub struct ObsCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> ObsCLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for ObsCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObsCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── ObsF Formatter ───────────────────────────────────────

/// Formatting options for observable output.
#[derive(Debug, Clone)]
pub struct ObsFFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for ObsFFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl ObsFFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for observable data.
pub struct ObsFFmt {
    options: ObsFFmtOpts,
}

impl ObsFFmt {
    pub fn new(options: ObsFFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: ObsFFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


/// Configuration manager for observable functionality.
pub struct ObservableConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ObservableConfig {
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

    pub fn merge(&mut self, other: &ObservableConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for observable operations.
pub struct ObservableRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ObservableRateTracker {
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

/// Validation result collector for observable.
pub struct ObservableValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ObservableValidationCollector {
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

    pub fn merge(&mut self, other: &ObservableValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Reactive observable value streams — extended utilities (zy)
// ---------------------------------------------------------------------------

/// Metric accumulator for observable operations.
#[derive(Debug, Clone)]
pub struct ZyMetrics {
    samples: Vec<f64>,
    label: String,
}

impl ZyMetrics {
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

/// Sliding-window rate counter for observable.
#[derive(Debug, Clone)]
pub struct ZyRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl ZyRateWindow {
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

/// A small LRU-style cache for observable lookups.
#[derive(Debug, Clone)]
pub struct ZyLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZyLruCache {
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
// xa_ extended helpers for observable
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaObservableRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaObservableRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaObservableCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaObservableCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaObservableCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 131
// ---------------------------------------------------------------------------

/// Generic object pool `Xc131Pool<T>`.
pub struct Xc131Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc131Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc131PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc131Pool<T> {
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
    pub fn stats(&self) -> Xc131PoolStats {
        Xc131PoolStats {
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

impl<T> Default for Xc131Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc131Scheduler`.
pub struct Xc131Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc131Scheduler {
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

impl Default for Xc131Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_131 hash for the given byte slice.
pub fn xc_131_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_131 convention.
pub fn xc_131_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_44 deepening: state machine + event bus ---

/// States for the Xd44 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd44State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd44State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd44Transition {
    pub from: Xd44State,
    pub to: Xd44State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd44StateMachine {
    current: Xd44State,
    history: Vec<Xd44Transition>,
    step_counter: usize,
}

impl Xd44StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd44State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd44State {
        self.current
    }

    pub fn history(&self) -> &[Xd44Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd44State) -> Result<Xd44State, String> {
        let allowed = match (self.current, target) {
            (Xd44State::Idle, Xd44State::Running) => true,
            (Xd44State::Running, Xd44State::Paused) => true,
            (Xd44State::Running, Xd44State::Done) => true,
            (Xd44State::Paused, Xd44State::Running) => true,
            (Xd44State::Paused, Xd44State::Done) => true,
            (Xd44State::Done, Xd44State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_44: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd44Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd44SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd44State> {
        let prefix = "Xd44SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd44State::Idle),
            "Running" => Some(Xd44State::Running),
            "Paused" => Some(Xd44State::Paused),
            "Done" => Some(Xd44State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd44State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd44 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd44Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd44Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd44HandlerFn = Box<dyn Fn(&Xd44Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd44EventBus {
    handlers: Vec<(usize, Option<String>, Xd44HandlerFn)>,
    next_id: usize,
    published: Vec<Xd44Event>,
}

impl Xd44EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd44Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd44Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd44Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd44Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #42
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf42Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf42TrieNode {
    children: std::collections::HashMap<char, Xf42TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf42Trie {
    root: Xf42TrieNode,
    count: usize,
}

impl Xf42Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf42TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf42TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf42TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf42BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf42BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 130).
pub struct Xh130SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh130SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 172 as u64,
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

/// A compact bit set supporting boolean operations (variant 130).
pub struct Xh130BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh130BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 130).
pub struct Xi130Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi130Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi130Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi130Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 130).
pub struct Xi130IntervalTree {
    xi_intervals: Vec<Xi130Interval>,
}

impl Xi130IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi130Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi130Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi130Interval) -> Vec<&Xi130Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi130Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi130Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi130Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi130Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi130Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi130Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observable_get_set() {
        let obs = ObservableValue::new(42);
        assert_eq!(obs.get(), 42);
        obs.set(100);
        assert_eq!(obs.get(), 100);
    }

    #[test]
    fn observable_fires_on_change() {
        let obs = ObservableValue::new(0);
        let received = Arc::new(Mutex::new(Vec::new()));
        let received2 = received.clone();
        let _handle = obs.on_change(move |val| {
            received2.lock().unwrap().push(*val);
        });
        obs.set(1);
        obs.set(2);
        obs.set(2); // same value, no fire
        obs.set(3);
        assert_eq!(*received.lock().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn observable_update() {
        let obs = ObservableValue::new(10);
        obs.update(|v| v + 5);
        assert_eq!(obs.get(), 15);
    }

    #[test]
    fn observable_map() {
        let obs = ObservableValue::new(5);
        let doubled = obs.map(|v| v * 2);
        assert_eq!(doubled.get(), 10);
        obs.set(10);
        assert_eq!(doubled.get(), 20);
    }

    #[test]
    fn get_and_subscribe_returns_current_and_subscribes() {
        let obs = ObservableValue::new(42);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let (current, _handle) = obs.get_and_subscribe(move |val| {
            r.lock().unwrap().push(*val);
        });
        assert_eq!(current, 42);
        obs.set(100);
        assert_eq!(*received.lock().unwrap(), vec![100]);
    }

    #[test]
    fn set_silent_does_not_fire() {
        let obs = ObservableValue::new(0);
        let fired = Arc::new(Mutex::new(false));
        let f = fired.clone();
        let _handle = obs.on_change(move |_| {
            *f.lock().unwrap() = true;
        });
        obs.set_silent(99);
        assert_eq!(obs.get(), 99);
        assert!(!*fired.lock().unwrap());
    }

    #[test]
    fn swap_returns_old_value() {
        let obs = ObservableValue::new(10);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = obs.on_change(move |val| {
            r.lock().unwrap().push(*val);
        });
        let old = obs.swap(20);
        assert_eq!(old, 10);
        assert_eq!(obs.get(), 20);
        assert_eq!(*received.lock().unwrap(), vec![20]);
    }

    #[test]
    fn swap_same_value_no_fire() {
        let obs = ObservableValue::new(5);
        let fired = Arc::new(Mutex::new(false));
        let f = fired.clone();
        let _handle = obs.on_change(move |_| {
            *f.lock().unwrap() = true;
        });
        let old = obs.swap(5);
        assert_eq!(old, 5);
        assert!(!*fired.lock().unwrap());
    }

    #[test]
    fn modify_mutates_in_place() {
        let obs = ObservableValue::new(vec![1, 2, 3]);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = obs.on_change(move |val| {
            r.lock().unwrap().push(val.clone());
        });
        obs.modify(|v| v.push(4));
        assert_eq!(obs.get(), vec![1, 2, 3, 4]);
        assert_eq!(*received.lock().unwrap(), vec![vec![1, 2, 3, 4]]);
    }

    #[test]
    fn display_impl() {
        let obs = ObservableValue::new(42);
        assert_eq!(format!("{obs}"), "Observable(42)");
        obs.set(99);
        assert_eq!(format!("{obs}"), "Observable(99)");
    }

    #[test]
    fn observable_list_push_and_events() {
        let list = ObservableList::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = list.on_change(move |snapshot| {
            r.lock().unwrap().push(snapshot.clone());
        });
        list.push(1);
        list.push(2);
        assert_eq!(list.len(), 2);
        assert_eq!(list.get(0), Some(1));
        assert_eq!(list.get(1), Some(2));
        let snapshots = received.lock().unwrap();
        assert_eq!(snapshots[0], vec![1]);
        assert_eq!(snapshots[1], vec![1, 2]);
    }

    #[test]
    fn observable_list_remove_and_clear() {
        let list = ObservableList::new();
        list.push(10);
        list.push(20);
        list.push(30);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = list.on_change(move |snapshot| {
            r.lock().unwrap().push(snapshot.clone());
        });
        let removed = list.remove_at(1);
        assert_eq!(removed, 20);
        assert_eq!(list.len(), 2);
        list.clear();
        assert!(list.is_empty());
        let snapshots = received.lock().unwrap();
        assert_eq!(snapshots[0], vec![10, 30]);
        assert_eq!(snapshots[1], Vec::<i32>::new());
    }

    #[test]
    fn error_display() {
        assert_eq!(
            format!("{}", ObservableError::LockPoisoned),
            "lock poisoned"
        );
        assert_eq!(
            format!("{}", ObservableError::AlreadyDisposed),
            "already disposed"
        );
    }

    #[test]
    fn eq_observableerror_same() {
        assert_eq!(ObservableError::LockPoisoned, ObservableError::LockPoisoned);
    }

    #[test]
    fn ne_observableerror_diff() {
        assert_ne!(ObservableError::LockPoisoned, ObservableError::AlreadyDisposed);
    }

    #[test]
    fn display_observableerror_variants() {
        assert!(!ObservableError::LockPoisoned.to_string().is_empty());
        assert!(!ObservableError::AlreadyDisposed.to_string().is_empty());
    }

    #[test]
    fn observable_map_insert_and_get() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        map.insert("a".to_string(), 1);
        map.insert("b".to_string(), 2);
        assert_eq!(map.get(&"a".to_string()), Some(1));
        assert_eq!(map.get(&"b".to_string()), Some(2));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn observable_map_remove() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        map.insert("a".to_string(), 1);
        let removed = map.remove(&"a".to_string());
        assert_eq!(removed, Some(1));
        assert!(map.is_empty());
    }

    #[test]
    fn observable_map_contains_key() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        map.insert("x".to_string(), 42);
        assert!(map.contains_key(&"x".to_string()));
        assert!(!map.contains_key(&"y".to_string()));
    }

    #[test]
    fn observable_map_clear() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        map.insert("a".to_string(), 1);
        map.insert("b".to_string(), 2);
        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn observable_map_keys() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        map.insert("a".to_string(), 1);
        map.insert("b".to_string(), 2);
        let mut keys = map.keys();
        keys.sort();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn observable_map_overwrite() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        map.insert("a".to_string(), 1);
        map.insert("a".to_string(), 99);
        assert_eq!(map.get(&"a".to_string()), Some(99));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn observable_map_remove_nonexistent() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        assert!(map.remove(&"nope".to_string()).is_none());
    }

    #[test]
    fn observable_list_insert_at() {
        let list: ObservableList<String> = ObservableList::new();
        list.push("a".to_string());
        list.push("c".to_string());
        list.insert_at(1, "b".to_string());
        assert_eq!(list.get(0), Some("a".to_string()));
        assert_eq!(list.get(1), Some("b".to_string()));
        assert_eq!(list.get(2), Some("c".to_string()));
    }

    #[test]
    fn observable_list_set() {
        let list: ObservableList<i32> = ObservableList::new();
        list.push(10);
        list.push(20);
        let old = list.set(1, 99);
        assert_eq!(old, Some(20));
        assert_eq!(list.get(1), Some(99));
    }

    #[test]
    fn observable_list_set_out_of_bounds() {
        let list: ObservableList<i32> = ObservableList::new();
        list.push(10);
        assert!(list.set(5, 99).is_none());
    }

    #[test]
    fn observable_list_to_vec() {
        let list: ObservableList<i32> = ObservableList::new();
        list.push(1);
        list.push(2);
        list.push(3);
        assert_eq!(list.to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn observable_list_retain() {
        let list: ObservableList<i32> = ObservableList::new();
        list.push(1);
        list.push(2);
        list.push(3);
        list.push(4);
        list.retain(|x| x % 2 == 0);
        assert_eq!(list.to_vec(), vec![2, 4]);
    }

    #[test]
    fn derived_from_maps_value() {
        let source = ObservableValue::new(5_i32);
        let derived = derived_from(&source, |v| v * 2);
        assert_eq!(derived.get(), 10);
    }

    #[test]
    fn derived_from_updates_on_change() {
        let source = ObservableValue::new(3_i32);
        let derived = derived_from(&source, |v| format!("val={v}"));
        assert_eq!(derived.get(), "val=3");
        source.set(7);
        assert_eq!(derived.get(), "val=7");
    }

    #[test]
    fn observable_map_on_change_fires() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        let fired = Arc::new(Mutex::new(0_u32));
        let f = fired.clone();
        let _handle = map.on_change(move |_| {
            *f.lock().unwrap() += 1;
        });
        map.insert("a".to_string(), 1);
        map.insert("b".to_string(), 2);
        map.remove(&"a".to_string());
        assert_eq!(*fired.lock().unwrap(), 3);
    }

    #[test]
    fn observable_map_default() {
        let map: ObservableMap<String, i32> = ObservableMap::default();
        assert!(map.is_empty());
    }

    #[test]
    fn observable_stats_new_defaults() {
        let stats = ObservableStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn observable_stats_record_success() {
        let mut stats = ObservableStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn observable_stats_record_failure() {
        let mut stats = ObservableStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn observable_stats_reset() {
        let mut stats = ObservableStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn observable_stats_merge() {
        let mut a = ObservableStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ObservableStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn observable_stats_display() {
        let mut stats = ObservableStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn observable_stats_default() {
        let stats = ObservableStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn observable_validator_accepts_and_rejects() {
        let mut v = ObservableValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn observable_validator_warnings() {
        let mut v = ObservableValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn observable_validator_clear_and_merge() {
        let mut v = ObservableValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ObservableValidationCollector::new();
        a.add_error("a_err");
        let mut b = ObservableValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    #[test]
    fn test_observable_combine_basic() {
        let a = ObservableValue::new(3);
        let b = ObservableValue::new(4);
        let result = observable_combine(&a, &b, |x, y| x + y);
        assert_eq!(result, 7);

        a.set(10);
        let result2 = observable_combine(&a, &b, |x, y| x * y);
        assert_eq!(result2, 40);
    }

    #[test]
    fn test_observable_transaction_commit() {
        let val1 = Arc::new(Mutex::new(1));
        let val2 = Arc::new(Mutex::new(2));

        let mut tx = ObservableTransaction::new();
        let v1 = val1.clone();
        let v2 = val2.clone();
        tx.defer(move || *v1.lock().unwrap() = 10);
        tx.defer(move || *v2.lock().unwrap() = 20);
        assert_eq!(tx.len(), 2);

        tx.commit();
        assert_eq!(*val1.lock().unwrap(), 10);
        assert_eq!(*val2.lock().unwrap(), 20);
    }

    #[test]
    fn test_observable_transaction_rollback() {
        let val = Arc::new(Mutex::new(1));
        let mut tx = ObservableTransaction::new();
        let v = val.clone();
        tx.defer(move || *v.lock().unwrap() = 99);
        tx.rollback();
        assert_eq!(*val.lock().unwrap(), 1);
    }

    #[test]
    fn test_observable_transaction_empty() {
        let tx = ObservableTransaction::new();
        assert!(tx.is_empty());
        assert_eq!(tx.len(), 0);
        tx.commit(); // should not panic
    }

    #[test]
    fn test_observable_history_record() {
        let mut history = ObservableHistory::new();
        history.record(10);
        history.record(20);
        history.record(30);
        assert_eq!(history.len(), 3);
        assert_eq!(history.at(0), Some(&10));
        assert_eq!(history.at(1), Some(&20));
        assert_eq!(history.at(2), Some(&30));
        assert_eq!(history.entries[0].sequence, 0);
        assert_eq!(history.entries[1].sequence, 1);
        assert_eq!(history.entries[2].sequence, 2);
    }

    #[test]
    fn test_observable_history_latest() {
        let mut history: ObservableHistory<i32> = ObservableHistory::new();
        assert_eq!(history.latest(), None);
        history.record(42);
        assert_eq!(history.latest(), Some(&42));
        history.record(99);
        assert_eq!(history.latest(), Some(&99));
    }

    #[test]
    fn test_observable_history_clear() {
        let mut history = ObservableHistory::new();
        history.record(1);
        history.record(2);
        assert_eq!(history.len(), 2);
        history.clear();
        assert_eq!(history.len(), 0);
        assert!(history.is_empty());
        assert_eq!(history.latest(), None);
    }

    // -- ObservableSet --

    #[test]
    fn observable_set_insert_contains_remove() {
        let set: ObservableSet<i32> = ObservableSet::new();
        assert!(set.is_empty());
        assert!(set.insert(1));
        assert!(set.insert(2));
        assert!(!set.insert(1)); // duplicate
        assert_eq!(set.len(), 2);
        assert!(set.contains(&1));
        assert!(set.remove(&1));
        assert!(!set.contains(&1));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn observable_set_clear_and_to_vec() {
        let set: ObservableSet<String> = ObservableSet::new();
        set.insert("a".into());
        set.insert("b".into());
        assert_eq!(set.to_vec().len(), 2);
        set.clear();
        assert!(set.is_empty());
    }

    // -- ObservableProjection --

    #[test]
    fn projection_maps_values() {
        let proj = ObservableProjection::new(10_i32, |v| v * 2);
        assert_eq!(proj.get(), 20);
        assert_eq!(proj.source(), 10);
        proj.update(5);
        assert_eq!(proj.get(), 10);
        assert_eq!(proj.source(), 5);
    }

    #[test]
    fn projection_string_transform() {
        let proj = ObservableProjection::new("hello".to_string(), |s| s.len());
        assert_eq!(proj.get(), 5);
        proj.update("hi".to_string());
        assert_eq!(proj.get(), 2);
    }

    // -- ObservableHistory branching --

    #[test]
    fn history_checkpoint_and_restore() {
        let mut h: ObservableHistory<i32> = ObservableHistory::new();
        h.record(10);
        h.record(20);
        let cp = h.checkpoint();
        assert_eq!(cp, 2);
        h.record(30);
        assert_eq!(h.len(), 3);
        h.restore_checkpoint(cp);
        assert_eq!(h.len(), 2);
        assert_eq!(h.latest(), Some(&20));
    }

    #[test]
    fn history_fork() {
        let mut h: ObservableHistory<&str> = ObservableHistory::new();
        h.record("a");
        h.record("b");
        h.record("c");
        let fork = h.fork(2);
        assert_eq!(fork.len(), 2);
        assert_eq!(fork.latest(), Some(&"b"));
        assert_eq!(h.len(), 3); // original unchanged
    }

    #[test]
    fn history_since() {
        let mut h: ObservableHistory<i32> = ObservableHistory::new();
        h.record(1);
        h.record(2);
        h.record(3);
        let since = h.since(1);
        assert_eq!(since, vec![&2, &3]);
    }

    // -- ObservableDebouncer --

    #[test]
    fn debouncer_stage_and_commit() {
        let mut d: ObservableDebouncer<i32> = ObservableDebouncer::new();
        assert!(!d.has_pending());
        assert_eq!(d.committed(), None);

        d.stage(42);
        assert!(d.has_pending());
        assert_eq!(d.pending(), Some(&42));
        assert!(d.commit());
        assert_eq!(d.committed(), Some(&42));
        assert_eq!(d.change_count(), 1);
    }

    #[test]
    fn debouncer_duplicate_commit_ignored() {
        let mut d: ObservableDebouncer<i32> = ObservableDebouncer::new();
        d.stage(5);
        assert!(d.commit());
        d.stage(5); // same value
        assert!(!d.commit()); // no new commit
        assert_eq!(d.change_count(), 1);
    }

    #[test]
    fn debouncer_discard() {
        let mut d: ObservableDebouncer<i32> = ObservableDebouncer::new();
        d.stage(10);
        d.discard();
        assert!(!d.has_pending());
        assert!(!d.commit());
        assert_eq!(d.committed(), None);
    }

    // -----------------------------------------------------------------------
    // New tests for added functionality
    // -----------------------------------------------------------------------

    #[test]
    fn observable_value_set_if_true() {
        let obs = ObservableValue::new(10);
        let changed = obs.set_if(|v| *v < 20, 15);
        assert!(changed);
        assert_eq!(obs.get(), 15);
    }

    #[test]
    fn observable_value_set_if_false() {
        let obs = ObservableValue::new(10);
        let changed = obs.set_if(|v| *v > 20, 15);
        assert!(!changed);
        assert_eq!(obs.get(), 10);
    }

    #[test]
    fn observable_value_take() {
        let obs = ObservableValue::new(42);
        let old = obs.take(99);
        assert_eq!(old, 42);
        assert_eq!(obs.get(), 99);
    }

    #[test]
    fn observable_value_spy() {
        let obs = ObservableValue::new(0);
        let (current, log, _handle) = obs.spy();
        assert_eq!(current, 0);
        obs.set(1);
        obs.set(2);
        obs.set(3);
        assert_eq!(*log.lock().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn observable_value_reset_to_default() {
        let obs = ObservableValue::new(42_i32);
        obs.reset();
        assert_eq!(obs.get(), 0);
    }

    #[test]
    fn observable_list_from_vec() {
        let list = ObservableList::from_vec(vec![1, 2, 3]);
        assert_eq!(list.len(), 3);
        assert_eq!(list.to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn observable_list_first_last() {
        let list = ObservableList::from_vec(vec![10, 20, 30]);
        assert_eq!(list.first(), Some(10));
        assert_eq!(list.last(), Some(30));

        let empty: ObservableList<i32> = ObservableList::new();
        assert_eq!(empty.first(), None);
        assert_eq!(empty.last(), None);
    }

    #[test]
    fn observable_list_map_items() {
        let list = ObservableList::from_vec(vec![1, 2, 3]);
        let doubled = list.map_items(|x| x * 2);
        assert_eq!(doubled, vec![2, 4, 6]);
    }

    #[test]
    fn observable_list_extend() {
        let list = ObservableList::from_vec(vec![1]);
        let fired = Arc::new(Mutex::new(0_u32));
        let f = fired.clone();
        let _handle = list.on_change(move |_| {
            *f.lock().unwrap() += 1;
        });
        list.extend(vec![2, 3, 4]);
        assert_eq!(list.to_vec(), vec![1, 2, 3, 4]);
        assert_eq!(*fired.lock().unwrap(), 1); // single event
    }

    #[test]
    fn observable_list_pop() {
        let list = ObservableList::from_vec(vec![1, 2, 3]);
        assert_eq!(list.pop(), Some(3));
        assert_eq!(list.len(), 2);

        let empty: ObservableList<i32> = ObservableList::new();
        assert_eq!(empty.pop(), None);
    }

    #[test]
    fn observable_list_index_of_and_contains() {
        let list = ObservableList::from_vec(vec![10, 20, 30]);
        assert_eq!(list.index_of(&20), Some(1));
        assert_eq!(list.index_of(&99), None);
        assert!(list.contains(&10));
        assert!(!list.contains(&99));
    }

    #[test]
    fn observable_map_values_and_to_vec() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        map.insert("a".to_string(), 1);
        map.insert("b".to_string(), 2);
        let mut values = map.values();
        values.sort();
        assert_eq!(values, vec![1, 2]);

        let mut entries = map.to_vec();
        entries.sort_by_key(|(k, _)| k.clone());
        assert_eq!(
            entries,
            vec![("a".to_string(), 1), ("b".to_string(), 2)]
        );
    }

    #[test]
    fn observable_map_get_or_insert() {
        let map: ObservableMap<String, i32> = ObservableMap::new();
        let v = map.get_or_insert("key".to_string(), 42);
        assert_eq!(v, 42);
        // Existing key should not overwrite.
        let v2 = map.get_or_insert("key".to_string(), 99);
        assert_eq!(v2, 42);
    }

    #[test]
    fn observable_stats_successes_failures_helpers() {
        let mut stats = ObservableStats::new();
        stats.record_success(10);
        stats.record_success(20);
        stats.record_failure(30);
        assert_eq!(stats.successes(), 2);
        assert_eq!(stats.failures(), 1);
        assert!(!stats.is_all_success());
        assert!(stats.total_time_ns() > 0);

        let mut clean = ObservableStats::new();
        clean.record_success(5);
        assert!(clean.is_all_success());
    }

    #[test]
    fn history_first_and_values() {
        let mut h: ObservableHistory<&str> = ObservableHistory::new();
        assert_eq!(h.first(), None);
        h.record("a");
        h.record("b");
        h.record("c");
        assert_eq!(h.first(), Some(&"a"));
        assert_eq!(h.values(), vec![&"a", &"b", &"c"]);
    }

    #[test]
    fn history_next_sequence() {
        let mut h: ObservableHistory<i32> = ObservableHistory::new();
        assert_eq!(h.next_sequence(), 0);
        h.record(1);
        assert_eq!(h.next_sequence(), 1);
        h.record(2);
        assert_eq!(h.next_sequence(), 2);
    }

    #[test]
    fn history_contains() {
        let mut h: ObservableHistory<i32> = ObservableHistory::new();
        h.record(10);
        h.record(20);
        assert!(h.contains(&10));
        assert!(!h.contains(&99));
    }

    #[test]
    fn debouncer_with_initial() {
        let d = ObservableDebouncer::with_initial(42);
        assert_eq!(d.committed(), Some(&42));
        assert!(d.has_committed());
        assert_eq!(d.change_count(), 0);
    }

    #[test]
    fn debouncer_set_shorthand() {
        let mut d: ObservableDebouncer<i32> = ObservableDebouncer::new();
        assert!(d.set(10));
        assert_eq!(d.committed(), Some(&10));
        assert!(!d.set(10)); // same value
        assert_eq!(d.change_count(), 1);
        assert!(d.set(20));
        assert_eq!(d.change_count(), 2);
    }

    #[test]
    fn debouncer_reset() {
        let mut d: ObservableDebouncer<i32> = ObservableDebouncer::new();
        d.set(42);
        d.stage(99);
        d.reset();
        assert!(!d.has_committed());
        assert!(!d.has_pending());
        assert_eq!(d.change_count(), 0);
    }

    #[test]
    fn observable_value_set_if_fires_event() {
        let obs = ObservableValue::new(5);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = obs.on_change(move |val| {
            r.lock().unwrap().push(*val);
        });
        obs.set_if(|v| *v == 5, 10);
        obs.set_if(|v| *v == 999, 20); // predicate false, no fire
        assert_eq!(*received.lock().unwrap(), vec![10]);
    }

    #[test]
    fn tracked_map_insert_and_get() {
        let mut m = ObservableTrackedMap::new();
        m.insert("a".into(), "1".into());
        assert_eq!(m.get("a"), Some(&"1".to_string()));
        assert!(m.contains_key("a"));
        assert_eq!(m.len(), 1);
        assert!(!m.is_empty());
    }

    #[test]
    fn tracked_map_update_records_change() {
        let mut m = ObservableTrackedMap::new();
        m.insert("k".into(), "old".into());
        m.insert("k".into(), "new".into());
        assert_eq!(m.changes().len(), 2);
        assert_eq!(
            m.changes()[1],
            MapChange::Update("k".into(), "old".into(), "new".into())
        );
    }

    #[test]
    fn tracked_map_remove() {
        let mut m = ObservableTrackedMap::new();
        m.insert("x".into(), "42".into());
        let removed = m.remove("x");
        assert_eq!(removed, Some("42".to_string()));
        assert!(!m.contains_key("x"));
        assert_eq!(
            m.changes().last().unwrap(),
            &MapChange::Remove("x".into(), "42".into())
        );
    }

    #[test]
    fn tracked_map_display() {
        let m = ObservableTrackedMap::new();
        assert_eq!(format!("{m}"), "ObservableTrackedMap(entries=0, changes=0)");
    }

    #[test]
    fn derived_sum() {
        let d = ObservableDerived::from_values(vec![1, 2, 3], "sum");
        assert_eq!(d.get(), 6);
        assert_eq!(d.source_count(), 3);
    }

    #[test]
    fn derived_product() {
        let d = ObservableDerived::from_values(vec![2, 3, 4], "product");
        assert_eq!(d.get(), 24);
    }

    #[test]
    fn derived_min_max() {
        let d_min = ObservableDerived::from_values(vec![5, 1, 9], "min");
        assert_eq!(d_min.get(), 1);
        let d_max = ObservableDerived::from_values(vec![5, 1, 9], "max");
        assert_eq!(d_max.get(), 9);
    }

    #[test]
    fn derived_update_source() {
        let mut d = ObservableDerived::from_values(vec![10, 20], "sum");
        d.update_source(0, 100);
        assert_eq!(d.get(), 120);
    }

    #[test]
    fn batch_lifecycle() {
        let mut b = ObservableBatch::new();
        assert!(!b.is_batching());
        b.begin_batch();
        assert!(b.is_batching());
        b.add_change("a", "1");
        b.add_change("b", "2");
        assert_eq!(b.pending_count(), 2);
        let changes = b.end_batch();
        assert!(!b.is_batching());
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].key, "a");
        assert_eq!(changes[1].value, "2");
    }

    #[test]
    fn batch_ignores_outside_batch() {
        let mut b = ObservableBatch::new();
        b.add_change("ignored", "value");
        assert_eq!(b.pending_count(), 0);
    }

    #[test]
    fn replay_buffer() {
        let mut r = ObservableReplay::new(3);
        r.emit("a".into());
        r.emit("b".into());
        r.emit("c".into());
        assert!(r.is_full());
        assert_eq!(r.len(), 3);
        assert_eq!(r.latest(), Some("c"));
        assert_eq!(r.replay(), vec!["a", "b", "c"]);
        r.emit("d".into());
        assert_eq!(r.replay(), vec!["b", "c", "d"]);
    }

    #[test]
    fn replay_clear() {
        let mut r = ObservableReplay::new(5);
        r.emit("x".into());
        r.clear();
        assert_eq!(r.len(), 0);
        assert_eq!(r.latest(), None);
        assert!(!r.is_full());
    }

    #[test]
    fn observable_list_pop_fires_event() {
        let list = ObservableList::from_vec(vec![1, 2, 3]);
        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _handle = list.on_change(move |snapshot| {
            r.lock().unwrap().push(snapshot.clone());
        });
        list.pop();
        assert_eq!(received.lock().unwrap()[0], vec![1, 2]);
    }

    #[test]
    fn obsc_lru_insert_get() {
        let mut c = ObsCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn obsc_lru_eviction() {
        let mut c = ObsCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn obsc_lru_hit_ratio() {
        let mut c = ObsCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn obsc_lru_clear() {
        let mut c = ObsCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn obsc_lru_remove() {
        let mut c = ObsCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn obsc_lru_peek() {
        let mut c = ObsCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn obsf_fmt_list() {
        let f = ObsFFmt::new(ObsFFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn obsf_fmt_kv() {
        let f = ObsFFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn obsf_fmt_section() {
        let f = ObsFFmt::new(ObsFFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn obsf_fmt_truncate() {
        let f = ObsFFmt::new(ObsFFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn obsf_fmt_opts_defaults() {
        let o = ObsFFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn observable_config_new() {
        let cfg = ObservableConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn observable_config_set_get() {
        let mut cfg = ObservableConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn observable_config_remove() {
        let mut cfg = ObservableConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn observable_config_keys_sorted() {
        let mut cfg = ObservableConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn observable_config_bump_version() {
        let mut cfg = ObservableConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn observable_config_clear() {
        let mut cfg = ObservableConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn observable_config_merge() {
        let mut cfg1 = ObservableConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ObservableConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn observable_config_disable() {
        let mut cfg = ObservableConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn observable_rate_tracker_empty() {
        let rt = ObservableRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn observable_rate_tracker_record() {
        let mut rt = ObservableRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn observable_rate_tracker_prune() {
        let mut rt = ObservableRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn observable_validator_valid() {
        let v = ObservableValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn observable_validator_errors() {
        let mut v = ObservableValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn observable_validator_clear() {
        let mut v = ObservableValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn observable_validator_merge() {
        let mut v1 = ObservableValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ObservableValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn observable_rate_tracker_clear() {
        let mut rt = ObservableRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zy_metrics_empty() {
        let m = ZyMetrics::new("observable");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zy_metrics_record_and_mean() {
        let mut m = ZyMetrics::new("observable");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zy_metrics_min_max() {
        let mut m = ZyMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zy_metrics_variance_and_std() {
        let mut m = ZyMetrics::new("v");
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
    fn zy_metrics_percentile() {
        let mut m = ZyMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn zy_metrics_merge() {
        let mut a = ZyMetrics::new("a");
        a.record(1.0);
        let mut b = ZyMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn zy_metrics_reset() {
        let mut m = ZyMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn zy_rate_window_empty() {
        let rw = ZyRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn zy_rate_window_tick_and_rate() {
        let mut rw = ZyRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn zy_lru_cache_basic() {
        let mut c = ZyLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn zy_lru_cache_contains_and_keys() {
        let mut c = ZyLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn zy_lru_cache_remove() {
        let mut c = ZyLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn zy_metrics_sum() {
        let mut m = ZyMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zy_metrics_label() {
        let m = ZyMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn zy_lru_cache_clear() {
        let mut c = ZyLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for observable
    #[test]
    fn xa_observable_ring_new() {
        let rb = super::XaObservableRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_observable_ring_push_len() {
        let mut rb = super::XaObservableRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_observable_ring_wrap() {
        let mut rb = super::XaObservableRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_observable_ring_mean_empty() {
        let rb = super::XaObservableRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_observable_ring_mean_values() {
        let mut rb = super::XaObservableRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_observable_ring_min_max() {
        let mut rb = super::XaObservableRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_observable_ring_iter() {
        let mut rb = super::XaObservableRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_observable_counter_new() {
        let c = super::XaObservableCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_observable_counter_inc() {
        let mut c = super::XaObservableCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_observable_counter_inc_by() {
        let mut c = super::XaObservableCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_observable_counter_reset() {
        let mut c = super::XaObservableCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_observable_counter_clear() {
        let mut c = super::XaObservableCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_observable_counter_default() {
        let c = super::XaObservableCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 131 ----

    #[test]
    fn xc_131_pool_new_empty() {
        let pool: super::Xc131Pool<i32> = super::Xc131Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_131_pool_release_acquire() {
        let mut pool = super::Xc131Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_131_pool_acquire_empty() {
        let mut pool: super::Xc131Pool<i32> = super::Xc131Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_131_pool_full() {
        let mut pool = super::Xc131Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_131_pool_drain() {
        let mut pool = super::Xc131Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_131_pool_stats() {
        let mut pool = super::Xc131Pool::new(8);
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
    fn xc_131_pool_clear() {
        let mut pool = super::Xc131Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_131_pool_shrink() {
        let mut pool = super::Xc131Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_131_pool_default() {
        let pool: super::Xc131Pool<String> = super::Xc131Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_131_pool_extend() {
        let mut pool = super::Xc131Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_131_pool_retain() {
        let mut pool = super::Xc131Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_131_scheduler_round_robin() {
        let mut sched = super::Xc131Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_131_scheduler_empty() {
        let mut sched = super::Xc131Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_131_scheduler_reset() {
        let mut sched = super::Xc131Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_131_scheduler_add_remove() {
        let mut sched = super::Xc131Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_131_scheduler_targets() {
        let sched = super::Xc131Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_131_hash_empty() {
        assert_eq!(super::xc_131_hash(b""), 5381);
    }

    #[test]
    fn xc_131_hash_data() {
        let h = super::xc_131_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_131_hash(b"hello"), h);
    }

    #[test]
    fn xc_131_reverse_str() {
        assert_eq!(super::xc_131_reverse("abc"), "cba");
        assert_eq!(super::xc_131_reverse(""), "");
    }


    // --- xd_44 deepening tests ---

    #[test]
    fn xd_44_sm_initial_state() {
        let sm = Xd44StateMachine::new();
        assert_eq!(sm.current_state(), Xd44State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_44_sm_valid_idle_to_running() {
        let mut sm = Xd44StateMachine::new();
        assert!(sm.transition(Xd44State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd44State::Running);
    }

    #[test]
    fn xd_44_sm_valid_running_to_paused() {
        let mut sm = Xd44StateMachine::new();
        sm.transition(Xd44State::Running).unwrap();
        assert!(sm.transition(Xd44State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd44State::Paused);
    }

    #[test]
    fn xd_44_sm_valid_running_to_done() {
        let mut sm = Xd44StateMachine::new();
        sm.transition(Xd44State::Running).unwrap();
        assert!(sm.transition(Xd44State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd44State::Done);
    }

    #[test]
    fn xd_44_sm_valid_paused_to_running() {
        let mut sm = Xd44StateMachine::new();
        sm.transition(Xd44State::Running).unwrap();
        sm.transition(Xd44State::Paused).unwrap();
        assert!(sm.transition(Xd44State::Running).is_ok());
    }

    #[test]
    fn xd_44_sm_valid_done_to_idle() {
        let mut sm = Xd44StateMachine::new();
        sm.transition(Xd44State::Running).unwrap();
        sm.transition(Xd44State::Done).unwrap();
        assert!(sm.transition(Xd44State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd44State::Idle);
    }

    #[test]
    fn xd_44_sm_invalid_idle_to_done() {
        let mut sm = Xd44StateMachine::new();
        assert!(sm.transition(Xd44State::Done).is_err());
    }

    #[test]
    fn xd_44_sm_invalid_idle_to_paused() {
        let mut sm = Xd44StateMachine::new();
        assert!(sm.transition(Xd44State::Paused).is_err());
    }

    #[test]
    fn xd_44_sm_history_tracking() {
        let mut sm = Xd44StateMachine::new();
        sm.transition(Xd44State::Running).unwrap();
        sm.transition(Xd44State::Paused).unwrap();
        sm.transition(Xd44State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd44State::Idle);
        assert_eq!(sm.history()[0].to, Xd44State::Running);
        assert_eq!(sm.history()[1].from, Xd44State::Running);
        assert_eq!(sm.history()[2].to, Xd44State::Done);
    }

    #[test]
    fn xd_44_sm_serialize_deserialize() {
        let mut sm = Xd44StateMachine::new();
        sm.transition(Xd44State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd44StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd44State::Running));
    }

    #[test]
    fn xd_44_sm_deserialize_invalid() {
        assert_eq!(Xd44StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_44_sm_reset() {
        let mut sm = Xd44StateMachine::new();
        sm.transition(Xd44State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd44State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_44_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd44EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd44Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_44_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd44EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd44Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd44Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_44_bus_unsubscribe() {
        let mut bus = Xd44EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_44_event_kind_and_payload() {
        let e = Xd44Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd44Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_44_bus_clear_history() {
        let mut bus = Xd44EventBus::new();
        bus.publish(Xd44Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_44_sm_step_counter_increments() {
        let mut sm = Xd44StateMachine::new();
        sm.transition(Xd44State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd44State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #42 --

    #[test]
    fn xf42_trie_insert_search() {
        let mut t = Xf42Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf42_trie_starts_with() {
        let mut t = Xf42Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf42_trie_remove() {
        let mut t = Xf42Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf42_trie_word_count() {
        let mut t = Xf42Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf42_trie_longest_prefix() {
        let mut t = Xf42Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf42_trie_all_words() {
        let mut t = Xf42Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf42_trie_autocomplete() {
        let mut t = Xf42Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf42_trie_empty_search() {
        let t = Xf42Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf42_bloom_add_contains() {
        let mut bf = Xf42BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf42_bloom_probably_absent() {
        let bf = Xf42BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf42_bloom_false_positive_rate() {
        let mut bf = Xf42BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf42_bloom_clear() {
        let mut bf = Xf42BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf42_bloom_union() {
        let mut a = Xf42BloomFilter::xf_new(512, 2);
        let mut b = Xf42BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf42_bloom_intersection_estimate() {
        let mut a = Xf42BloomFilter::xf_new(512, 2);
        let mut b = Xf42BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf42_bloom_union_size_mismatch() {
        let a = Xf42BloomFilter::xf_new(256, 2);
        let b = Xf42BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh130_skip_insert_contains() {
        let mut sl = super::Xh130SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh130_skip_remove() {
        let mut sl = super::Xh130SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh130_skip_len() {
        let mut sl = super::Xh130SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh130_skip_range_query() {
        let mut sl = super::Xh130SkipList::xh_new(4);
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
    fn xh130_skip_floor_ceiling() {
        let mut sl = super::Xh130SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh130_skip_rank() {
        let mut sl = super::Xh130SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh130_skip_empty() {
        let sl = super::Xh130SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh130_skip_duplicates() {
        let mut sl = super::Xh130SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh130_bitset_set_test() {
        let mut bs = super::Xh130BitSet::xh_new(256);
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
    fn xh130_bitset_clear_count() {
        let mut bs = super::Xh130BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh130_bitset_and_or_xor() {
        let mut a = super::Xh130BitSet::xh_new(128);
        let mut b = super::Xh130BitSet::xh_new(128);
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
    fn xh130_bitset_iter_ones() {
        let mut bs = super::Xh130BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh130_bitset_first_last() {
        let mut bs = super::Xh130BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh130_bitset_empty() {
        let bs = super::Xh130BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi130_deque_push_pop_back() {
        let mut dq = super::Xi130Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi130_deque_push_pop_front() {
        let mut dq = super::Xi130Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi130_deque_mixed_ops() {
        let mut dq = super::Xi130Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi130_deque_get_and_split() {
        let mut dq = super::Xi130Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi130_deque_rotate_left() {
        let mut dq = super::Xi130Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi130_deque_rotate_right() {
        let mut dq = super::Xi130Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi130_deque_grow() {
        let mut dq = super::Xi130Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi130_deque_empty() {
        let dq = super::Xi130Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi130_interval_tree_insert_query() {
        let mut tree = super::Xi130IntervalTree::xi_new();
        tree.xi_insert(super::Xi130Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi130Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi130Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi130_interval_tree_overlap() {
        let mut tree = super::Xi130IntervalTree::xi_new();
        tree.xi_insert(super::Xi130Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi130Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi130Interval::xi_new(12, 20));
        let q = super::Xi130Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi130_interval_tree_remove() {
        let mut tree = super::Xi130IntervalTree::xi_new();
        tree.xi_insert(super::Xi130Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi130Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi130_interval_tree_gaps() {
        let mut tree = super::Xi130IntervalTree::xi_new();
        tree.xi_insert(super::Xi130Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi130Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi130Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi130Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi130Interval::xi_new(8, 10));
    }

    #[test]
    fn xi130_interval_tree_merge() {
        let mut tree = super::Xi130IntervalTree::xi_new();
        tree.xi_insert(super::Xi130Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi130Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi130Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi130Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi130Interval::xi_new(10, 15));
    }

    #[test]
    fn xi130_interval_tree_all() {
        let mut tree = super::Xi130IntervalTree::xi_new();
        tree.xi_insert(super::Xi130Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi130Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi130_interval_tree_empty() {
        let tree = super::Xi130IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi130_interval_tree_contains_point() {
        let iv = super::Xi130Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}