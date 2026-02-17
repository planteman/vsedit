//! Reactive observable system.
//!
//! Provides `IObservable<T>` and related types, equivalent to
//! VS Code's `vs/base/common/observable.ts`.

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
    fn observable_validator_accepts_valid_name() {
        let v = ObservableValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn observable_validator_rejects_empty() {
        let v = ObservableValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn observable_validator_rejects_too_long() {
        let v = ObservableValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn observable_validator_forbidden_prefix() {
        let v = ObservableValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn observable_validator_allowed_chars() {
        let v = ObservableValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn observable_validator_range() {
        let v = ObservableValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn observable_sanitize_removes_control() {
        let result = ObservableValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn observable_truncate_short_string() {
        assert_eq!(ObservableValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn observable_truncate_long_string() {
        let result = ObservableValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn observable_is_ascii_printable() {
        assert!(ObservableValidator::is_ascii_printable("Hello World 123"));
        assert!(!ObservableValidator::is_ascii_printable("Hello\x00World"));
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
}
