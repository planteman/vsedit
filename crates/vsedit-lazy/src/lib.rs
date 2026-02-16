//! Lazy evaluation and caching utilities.
//!
//! Provides `Lazy<T>` for deferred computation, equivalent to VS Code's
//! `vs/base/common/lazy.ts`.

use std::fmt;
use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Errors related to lazy evaluation and caching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LazyError {
    /// The value has already been initialized.
    AlreadyInitialized,
    /// The value has not been initialized yet.
    NotInitialized,
    /// The computation failed.
    ComputationFailed(String),
}

impl std::fmt::Display for LazyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LazyError::AlreadyInitialized => write!(f, "value has already been initialized"),
            LazyError::NotInitialized => write!(f, "value has not been initialized"),
            LazyError::ComputationFailed(msg) => write!(f, "computation failed: {msg}"),
        }
    }
}

impl std::error::Error for LazyError {}

impl LazyError {
    /// Returns `true` if this is the `NotInitialized` variant.
    pub fn is_not_initialized(&self) -> bool {
        matches!(self, LazyError::NotInitialized)
    }

    /// Returns `true` if this is the `AlreadyInitialized` variant.
    pub fn is_already_initialized(&self) -> bool {
        matches!(self, LazyError::AlreadyInitialized)
    }
}

/// A lazily initialized value computed from a closure.
///
/// The closure runs at most once, on first access.
pub struct Lazy<T> {
    cell: OnceCell<T>,
    init: Option<Box<dyn FnOnce() -> T>>,
}

impl<T> Lazy<T> {
    /// Create a new lazy value with the given initializer.
    pub fn new(init: impl FnOnce() -> T + 'static) -> Self {
        Self {
            cell: OnceCell::new(),
            init: Some(Box::new(init)),
        }
    }

    /// Get the value, initializing it if necessary.
    pub fn get(&mut self) -> &T {
        if self.cell.get().is_none() {
            if let Some(init) = self.init.take() {
                let _ = self.cell.set(init());
            }
        }
        self.cell.get().expect("lazy value must be initialized")
    }

    /// Try to get the value without initializing it.
    /// Returns `None` if not yet initialized.
    pub fn try_get(&self) -> Option<&T> {
        self.cell.get()
    }

    /// Consume the `Lazy` and return the inner value if initialized.
    pub fn into_inner(self) -> Option<T> {
        self.cell.into_inner()
    }

    /// Check if the value has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.cell.get().is_some()
    }
}

/// A thread-safe lazily initialized value.
pub struct SyncLazy<T> {
    cell: OnceLock<T>,
    init: std::sync::Mutex<Option<Box<dyn FnOnce() -> T + Send>>>,
}

impl<T> SyncLazy<T> {
    /// Create a new thread-safe lazy value.
    pub fn new(init: impl FnOnce() -> T + Send + 'static) -> Self {
        Self {
            cell: OnceLock::new(),
            init: std::sync::Mutex::new(Some(Box::new(init))),
        }
    }

    /// Get the value, initializing it if necessary.
    pub fn get(&self) -> &T {
        self.cell.get_or_init(|| {
            let init = self
                .init
                .lock()
                .expect("lock poisoned")
                .take()
                .expect("init already consumed");
            init()
        })
    }

    /// Check if the value has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.cell.get().is_some()
    }
}

// Safety: SyncLazy is Send+Sync when T is
unsafe impl<T: Send + Sync> Send for SyncLazy<T> {}
unsafe impl<T: Send + Sync> Sync for SyncLazy<T> {}

/// A cached value that can be invalidated.
pub struct CachedValue<T> {
    value: Option<T>,
    compute: Box<dyn FnMut() -> T>,
}

impl<T> CachedValue<T> {
    /// Create a new cached value with the given computation.
    pub fn new(compute: impl FnMut() -> T + 'static) -> Self {
        Self {
            value: None,
            compute: Box::new(compute),
        }
    }

    /// Get the cached value, computing it if not yet cached.
    pub fn get(&mut self) -> &T {
        if self.value.is_none() {
            self.value = Some((self.compute)());
        }
        self.value.as_ref().unwrap()
    }

    /// Invalidate the cached value, forcing recomputation on next access.
    pub fn invalidate(&mut self) {
        self.value = None;
    }

    /// Check if a value is currently cached.
    pub fn is_cached(&self) -> bool {
        self.value.is_some()
    }

    /// Get the cached value, or compute it with a fallback closure if not cached.
    pub fn get_or_compute_with(&mut self, fallback: impl FnOnce() -> T) -> &T {
        if self.value.is_none() {
            self.value = Some(fallback());
        }
        self.value.as_ref().unwrap()
    }

    /// Take the cached value out, leaving the cache invalidated.
    /// Returns `None` if no value was cached.
    pub fn take(&mut self) -> Option<T> {
        self.value.take()
    }

    /// Check if the cached value is stale according to a predicate.
    /// Returns `false` if no value is cached.
    pub fn is_stale(&self, predicate: impl FnOnce(&T) -> bool) -> bool {
        match &self.value {
            Some(v) => predicate(v),
            None => false,
        }
    }

    /// Return a reference to the cached value without triggering computation.
    /// Returns `None` if no value has been computed yet.
    pub fn get_if_cached(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Force recomputation of the cached value immediately.
    pub fn refresh(&mut self) {
        self.value = Some((self.compute)());
    }
}

impl<T: fmt::Debug> fmt::Display for CachedValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            Some(v) => write!(f, "Cached({:?})", v),
            None => write!(f, "Empty"),
        }
    }
}

/// A cached value that auto-invalidates after a configurable duration.
pub struct TimedCache<T> {
    value: Option<T>,
    compute: Box<dyn FnMut() -> T>,
    duration: Duration,
    last_computed: Option<Instant>,
}

impl<T> TimedCache<T> {
    /// Create a new timed cache with the given computation and TTL duration.
    pub fn new(duration: Duration, compute: impl FnMut() -> T + 'static) -> Self {
        Self {
            value: None,
            compute: Box::new(compute),
            duration,
            last_computed: None,
        }
    }

    /// Get the cached value, recomputing if expired or not yet computed.
    pub fn get(&mut self) -> &T {
        let expired = match self.last_computed {
            Some(ts) => ts.elapsed() >= self.duration,
            None => true,
        };
        if expired {
            self.value = Some((self.compute)());
            self.last_computed = Some(Instant::now());
        }
        self.value.as_ref().unwrap()
    }

    /// Force invalidation, causing recomputation on next access.
    pub fn invalidate(&mut self) {
        self.value = None;
        self.last_computed = None;
    }

    /// Check if the cached value has expired.
    pub fn is_expired(&self) -> bool {
        match self.last_computed {
            Some(ts) => ts.elapsed() >= self.duration,
            None => true,
        }
    }

    /// Check if a value is currently cached (not expired).
    pub fn is_cached(&self) -> bool {
        self.value.is_some() && !self.is_expired()
    }
}

/// Memoization cache for a function: caches results keyed by input.
pub struct MemoizedFn<K, V> {
    cache: HashMap<K, V>,
    compute: Box<dyn FnMut(&K) -> V>,
}

impl<K: Eq + std::hash::Hash + Clone, V> MemoizedFn<K, V> {
    /// Create a new memoized function wrapper.
    pub fn new(compute: impl FnMut(&K) -> V + 'static) -> Self {
        Self {
            cache: HashMap::new(),
            compute: Box::new(compute),
        }
    }

    /// Call the function with the given key, returning a cached result if available.
    pub fn call(&mut self, key: K) -> &V {
        if !self.cache.contains_key(&key) {
            let value = (self.compute)(&key);
            self.cache.insert(key.clone(), value);
        }
        self.cache.get(&key).unwrap()
    }

    /// Clear all cached results.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Return the number of cached entries.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

/// Accumulated statistics for lazy operations.
#[derive(Debug, Clone, PartialEq)]
pub struct LazyStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl LazyStats {
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
    pub fn merge(&mut self, other: &LazyStats) {
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

impl Default for LazyStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LazyStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LazyStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for lazy.
#[derive(Debug, Clone)]
pub struct LazyValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl LazyValidator {
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

impl Default for LazyValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LazySequence
// ---------------------------------------------------------------------------

/// A chain of deferred computations that are evaluated lazily.
/// Each step transforms the value from the previous step.
pub struct LazySequence<T: 'static> {
    steps: Vec<Box<dyn FnOnce(T) -> T>>,
    initial: Option<T>,
}

impl<T: 'static> LazySequence<T> {
    /// Create a new lazy sequence with an initial value.
    pub fn new(initial: T) -> Self {
        Self {
            steps: Vec::new(),
            initial: Some(initial),
        }
    }

    /// Add a transformation step to the sequence.
    pub fn then(mut self, f: impl FnOnce(T) -> T + 'static) -> Self {
        self.steps.push(Box::new(f));
        self
    }

    /// Evaluate the entire sequence and return the final value.
    /// Consumes the sequence.
    pub fn evaluate(mut self) -> T {
        let mut value = self.initial.take().expect("sequence already evaluated");
        for step in self.steps {
            value = step(value);
        }
        value
    }

    /// Number of transformation steps in the sequence.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Whether the sequence has been evaluated (initial value consumed).
    pub fn is_consumed(&self) -> bool {
        self.initial.is_none()
    }
}

// ---------------------------------------------------------------------------
// LazyCache
// ---------------------------------------------------------------------------

/// A cache mapping keys to values with per-entry TTL expiration.
pub struct LazyCache<K: Eq + std::hash::Hash + Clone, V: Clone> {
    entries: HashMap<K, LazyCacheEntry<V>>,
    default_ttl: Duration,
}

struct LazyCacheEntry<V> {
    value: V,
    inserted_at: Instant,
    ttl: Duration,
}

impl<V> LazyCacheEntry<V> {
    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() >= self.ttl
    }
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> LazyCache<K, V> {
    /// Create a new cache with a default TTL.
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            default_ttl,
        }
    }

    /// Insert a value with the default TTL.
    pub fn insert(&mut self, key: K, value: V) {
        self.entries.insert(key, LazyCacheEntry {
            value,
            inserted_at: Instant::now(),
            ttl: self.default_ttl,
        });
    }

    /// Insert a value with a custom TTL.
    pub fn insert_with_ttl(&mut self, key: K, value: V, ttl: Duration) {
        self.entries.insert(key, LazyCacheEntry {
            value,
            inserted_at: Instant::now(),
            ttl,
        });
    }

    /// Get a value, returning None if expired or not present.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some(&entry.value)
            }
        })
    }

    /// Get a clone of the value.
    pub fn get_cloned(&self, key: &K) -> Option<V> {
        self.get(key).cloned()
    }

    /// Remove an entry.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries.remove(key).map(|e| e.value)
    }

    /// Remove all expired entries.
    pub fn evict_expired(&mut self) {
        self.entries.retain(|_, entry| !entry.is_expired());
    }

    /// Number of entries (including potentially expired ones).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Number of non-expired entries.
    pub fn active_count(&self) -> usize {
        self.entries.values().filter(|e| !e.is_expired()).count()
    }

    /// Whether the cache has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Check if a key exists and is not expired.
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Get or insert a value using a closure if not present/expired.
    /// Returns a clone of the value.
    pub fn get_or_insert_with(&mut self, key: K, f: impl FnOnce() -> V) -> V {
        if let Some(val) = self.get(&key).cloned() {
            return val;
        }
        let value = f();
        self.insert(key, value.clone());
        value
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Evaluate multiple lazy computations and return the first `Some` result.
pub fn lazy_race<T>(mut computations: Vec<Box<dyn FnOnce() -> Option<T>>>) -> Option<T> {
    for computation in computations.drain(..) {
        if let Some(result) = computation() {
            return Some(result);
        }
    }
    None
}

/// Evaluate multiple lazy computations and return all `Some` results.
pub fn lazy_all<T>(mut computations: Vec<Box<dyn FnOnce() -> Option<T>>>) -> Vec<T> {
    let mut results = Vec::new();
    for computation in computations.drain(..) {
        if let Some(result) = computation() {
            results.push(result);
        }
    }
    results
}

/// Create a lazy value that maps the result of another lazy value.
pub fn lazy_map<T: 'static, U: 'static>(
    mut lazy: Lazy<T>,
    f: impl FnOnce(&T) -> U + 'static,
) -> Lazy<U> {
    Lazy::new(move || {
        let val = lazy.get();
        f(val)
    })
}

/// Create a `Lazy<T>` that is already initialized with the given value.
pub fn lazy_from_value<T: 'static>(value: T) -> Lazy<T> {
    let lazy = Lazy {
        cell: OnceCell::new(),
        init: None,
    };
    let _ = lazy.cell.set(value);
    lazy
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn lazy_computes_once() {
        let count = Rc::new(Cell::new(0));
        let count2 = count.clone();
        let mut lazy = Lazy::new(move || {
            count2.set(count2.get() + 1);
            42
        });
        assert!(!lazy.is_initialized());
        assert_eq!(*lazy.get(), 42);
        assert_eq!(*lazy.get(), 42);
        assert!(lazy.is_initialized());
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn sync_lazy_is_thread_safe() {
        let lazy = std::sync::Arc::new(SyncLazy::new(|| 42));
        let lazy2 = lazy.clone();
        let handle = std::thread::spawn(move || *lazy2.get());
        assert_eq!(*lazy.get(), 42);
        assert_eq!(handle.join().unwrap(), 42);
    }

    #[test]
    fn cached_value_invalidation() {
        let mut counter = 0u32;
        let mut cached = CachedValue::new(move || {
            counter += 1;
            counter
        });
        assert_eq!(*cached.get(), 1);
        assert_eq!(*cached.get(), 1);
        cached.invalidate();
        assert_eq!(*cached.get(), 2);
    }

    #[test]
    fn try_get_before_init() {
        let lazy = Lazy::new(|| 99);
        assert!(lazy.try_get().is_none());
    }

    #[test]
    fn try_get_after_init() {
        let mut lazy = Lazy::new(|| 99);
        let _ = lazy.get();
        assert_eq!(lazy.try_get(), Some(&99));
    }

    #[test]
    fn into_inner_initialized() {
        let mut lazy = Lazy::new(|| String::from("hello"));
        let _ = lazy.get();
        assert_eq!(lazy.into_inner(), Some(String::from("hello")));
    }

    #[test]
    fn into_inner_uninitialized() {
        let lazy = Lazy::new(|| 42);
        assert_eq!(lazy.into_inner(), None);
    }

    #[test]
    fn cached_take() {
        let mut cached = CachedValue::new(|| 10);
        assert_eq!(*cached.get(), 10);
        assert!(cached.is_cached());
        let taken = cached.take();
        assert_eq!(taken, Some(10));
        assert!(!cached.is_cached());
    }

    #[test]
    fn cached_get_or_compute_with() {
        let mut cached = CachedValue::new(|| 1);
        let v = cached.get_or_compute_with(|| 999);
        assert_eq!(*v, 999);
        // Once cached, original compute is not used either
        assert_eq!(*cached.get(), 999);
    }

    #[test]
    fn cached_is_stale() {
        let mut cached = CachedValue::new(|| 5);
        // No value cached yet
        assert!(!cached.is_stale(|_| true));
        let _ = cached.get();
        assert!(cached.is_stale(|v| *v < 10));
        assert!(!cached.is_stale(|v| *v > 10));
    }

    #[test]
    fn timed_cache_expiry() {
        use std::time::Duration;
        let mut counter = 0u32;
        let mut tc = TimedCache::new(Duration::from_millis(50), move || {
            counter += 1;
            counter
        });
        assert_eq!(*tc.get(), 1);
        assert!(tc.is_cached());
        assert_eq!(*tc.get(), 1); // still cached
        std::thread::sleep(Duration::from_millis(60));
        assert!(tc.is_expired());
        assert_eq!(*tc.get(), 2); // recomputed
    }

    #[test]
    fn timed_cache_invalidate() {
        use std::time::Duration;
        let mut counter = 0u32;
        let mut tc = TimedCache::new(Duration::from_secs(60), move || {
            counter += 1;
            counter
        });
        assert_eq!(*tc.get(), 1);
        tc.invalidate();
        assert!(!tc.is_cached());
        assert_eq!(*tc.get(), 2);
    }

    #[test]
    fn memoized_fn_caching() {
        let mut memo = MemoizedFn::new(|k: &i32| k * 2);
        assert_eq!(*memo.call(3), 6);
        assert_eq!(*memo.call(3), 6); // cached
        assert_eq!(*memo.call(5), 10);
        assert_eq!(memo.len(), 2);
        memo.clear();
        assert!(memo.is_empty());
    }

    #[test]
    fn error_display() {
        assert_eq!(
            LazyError::AlreadyInitialized.to_string(),
            "value has already been initialized"
        );
        assert_eq!(
            LazyError::NotInitialized.to_string(),
            "value has not been initialized"
        );
        assert_eq!(
            LazyError::ComputationFailed("oops".into()).to_string(),
            "computation failed: oops"
        );
    }

    #[test]
    fn eq_lazyerror_same() {
        assert_eq!(LazyError::AlreadyInitialized, LazyError::AlreadyInitialized);
    }

    #[test]
    fn ne_lazyerror_diff() {
        assert_ne!(LazyError::AlreadyInitialized, LazyError::NotInitialized);
    }

    #[test]
    fn display_lazyerror_variants() {
        assert!(!LazyError::AlreadyInitialized.to_string().is_empty());
        assert!(!LazyError::NotInitialized.to_string().is_empty());
    }

    #[test]
    fn lazy_sequence_simple() {
        let seq = LazySequence::new(1)
            .then(|x| x + 1)
            .then(|x| x * 3);
        assert_eq!(seq.step_count(), 2);
        assert_eq!(seq.evaluate(), 6);
    }

    #[test]
    fn lazy_sequence_no_steps() {
        let seq = LazySequence::new(42);
        assert_eq!(seq.step_count(), 0);
        assert_eq!(seq.evaluate(), 42);
    }

    #[test]
    fn lazy_sequence_string_transform() {
        let seq = LazySequence::new("hello".to_string())
            .then(|s| s.to_uppercase())
            .then(|s| format!("{s}!"));
        assert_eq!(seq.evaluate(), "HELLO!");
    }

    #[test]
    fn lazy_sequence_consumed_after_evaluate() {
        let seq = LazySequence::new(1);
        let seq = seq.then(|x| x + 1);
        assert!(!seq.is_consumed());
        let _ = seq.evaluate();
    }

    #[test]
    fn lazy_cache_insert_get() {
        let mut cache: LazyCache<String, i32> = LazyCache::new(Duration::from_secs(60));
        cache.insert("key".to_string(), 42);
        assert_eq!(cache.get(&"key".to_string()), Some(&42));
        assert!(cache.contains_key(&"key".to_string()));
    }

    #[test]
    fn lazy_cache_missing_key() {
        let cache: LazyCache<String, i32> = LazyCache::new(Duration::from_secs(60));
        assert!(cache.get(&"nope".to_string()).is_none());
    }

    #[test]
    fn lazy_cache_remove() {
        let mut cache: LazyCache<String, i32> = LazyCache::new(Duration::from_secs(60));
        cache.insert("key".to_string(), 42);
        let removed = cache.remove(&"key".to_string());
        assert_eq!(removed, Some(42));
        assert!(cache.is_empty());
    }

    #[test]
    fn lazy_cache_clear() {
        let mut cache: LazyCache<String, i32> = LazyCache::new(Duration::from_secs(60));
        cache.insert("a".to_string(), 1);
        cache.insert("b".to_string(), 2);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn lazy_cache_get_cloned() {
        let mut cache: LazyCache<String, String> = LazyCache::new(Duration::from_secs(60));
        cache.insert("k".to_string(), "value".to_string());
        assert_eq!(cache.get_cloned(&"k".to_string()), Some("value".to_string()));
    }

    #[test]
    fn lazy_cache_get_or_insert_with() {
        let mut cache: LazyCache<String, i32> = LazyCache::new(Duration::from_secs(60));
        let val = cache.get_or_insert_with("key".to_string(), || 42);
        assert_eq!(val, 42);
        let val2 = cache.get_or_insert_with("key".to_string(), || 99);
        assert_eq!(val2, 42);
    }

    #[test]
    fn lazy_cache_expired_entry_not_returned() {
        let mut cache: LazyCache<String, i32> = LazyCache::new(Duration::from_millis(0));
        cache.insert("key".to_string(), 42);
        std::thread::sleep(Duration::from_millis(1));
        assert!(cache.get(&"key".to_string()).is_none());
    }

    #[test]
    fn lazy_cache_evict_expired() {
        let mut cache: LazyCache<String, i32> = LazyCache::new(Duration::from_millis(0));
        cache.insert("a".to_string(), 1);
        cache.insert("b".to_string(), 2);
        std::thread::sleep(Duration::from_millis(1));
        cache.evict_expired();
        assert!(cache.is_empty());
    }

    #[test]
    fn lazy_cache_active_count() {
        let mut cache: LazyCache<String, i32> = LazyCache::new(Duration::from_secs(60));
        cache.insert("a".to_string(), 1);
        cache.insert_with_ttl("b".to_string(), 2, Duration::from_millis(0));
        std::thread::sleep(Duration::from_millis(1));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.active_count(), 1);
    }

    #[test]
    fn lazy_race_first_wins() {
        let computations: Vec<Box<dyn FnOnce() -> Option<i32>>> = vec![
            Box::new(|| None),
            Box::new(|| Some(42)),
            Box::new(|| Some(99)),
        ];
        assert_eq!(lazy_race(computations), Some(42));
    }

    #[test]
    fn lazy_race_all_none() {
        let computations: Vec<Box<dyn FnOnce() -> Option<i32>>> = vec![
            Box::new(|| None),
            Box::new(|| None),
        ];
        assert_eq!(lazy_race(computations), None);
    }

    #[test]
    fn lazy_race_empty() {
        let computations: Vec<Box<dyn FnOnce() -> Option<i32>>> = vec![];
        assert_eq!(lazy_race(computations), None);
    }

    #[test]
    fn lazy_all_collects_results() {
        let computations: Vec<Box<dyn FnOnce() -> Option<i32>>> = vec![
            Box::new(|| Some(1)),
            Box::new(|| None),
            Box::new(|| Some(3)),
        ];
        assert_eq!(lazy_all(computations), vec![1, 3]);
    }

    #[test]
    fn lazy_map_transforms() {
        let lazy = Lazy::new(|| 21);
        let mut mapped = lazy_map(lazy, |v| v * 2);
        assert_eq!(*mapped.get(), 42);
    }

    #[test]
    fn lazy_sequence_many_steps() {
        let mut seq = LazySequence::new(0_i32);
        for i in 1..=10 {
            seq = seq.then(move |x| x + i);
        }
        assert_eq!(seq.evaluate(), 55);
    }

    #[test]
    fn lazy_cache_custom_ttl() {
        let mut cache: LazyCache<String, i32> = LazyCache::new(Duration::from_secs(60));
        cache.insert_with_ttl("short".to_string(), 1, Duration::from_millis(0));
        cache.insert("long".to_string(), 2);
        std::thread::sleep(Duration::from_millis(1));
        assert!(cache.get(&"short".to_string()).is_none());
        assert_eq!(cache.get(&"long".to_string()), Some(&2));
    }

    #[test]
    fn lazy_stats_new_defaults() {
        let stats = LazyStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn lazy_stats_record_success() {
        let mut stats = LazyStats::new();
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
    fn lazy_stats_record_failure() {
        let mut stats = LazyStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn lazy_stats_reset() {
        let mut stats = LazyStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn lazy_stats_merge() {
        let mut a = LazyStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = LazyStats::new();
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
    fn lazy_stats_display() {
        let mut stats = LazyStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn lazy_stats_default() {
        let stats = LazyStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn lazy_validator_accepts_valid_name() {
        let v = LazyValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn lazy_validator_rejects_empty() {
        let v = LazyValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn lazy_validator_rejects_too_long() {
        let v = LazyValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn lazy_validator_forbidden_prefix() {
        let v = LazyValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn lazy_validator_allowed_chars() {
        let v = LazyValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn lazy_validator_range() {
        let v = LazyValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn lazy_sanitize_removes_control() {
        let result = LazyValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn lazy_truncate_short_string() {
        assert_eq!(LazyValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn lazy_truncate_long_string() {
        let result = LazyValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn lazy_is_ascii_printable() {
        assert!(LazyValidator::is_ascii_printable("Hello World 123"));
        assert!(!LazyValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn cached_value_get_if_cached_none() {
        let cached = CachedValue::new(|| 42);
        assert!(cached.get_if_cached().is_none());
    }

    #[test]
    fn cached_value_get_if_cached_some() {
        let mut cached = CachedValue::new(|| 42);
        let _ = cached.get();
        assert_eq!(cached.get_if_cached(), Some(&42));
    }

    #[test]
    fn cached_value_refresh() {
        let mut counter = 0u32;
        let mut cached = CachedValue::new(move || {
            counter += 1;
            counter
        });
        assert_eq!(*cached.get(), 1);
        cached.refresh();
        assert_eq!(cached.get_if_cached(), Some(&2));
        cached.refresh();
        assert_eq!(cached.get_if_cached(), Some(&3));
    }

    #[test]
    fn cached_value_display_empty() {
        let cached = CachedValue::new(|| 42);
        assert_eq!(format!("{cached}"), "Empty");
    }

    #[test]
    fn cached_value_display_cached() {
        let mut cached = CachedValue::new(|| 42);
        let _ = cached.get();
        assert_eq!(format!("{cached}"), "Cached(42)");
    }

    #[test]
    fn lazy_error_is_not_initialized() {
        assert!(LazyError::NotInitialized.is_not_initialized());
        assert!(!LazyError::AlreadyInitialized.is_not_initialized());
        assert!(!LazyError::ComputationFailed("x".into()).is_not_initialized());
    }

    #[test]
    fn lazy_error_is_already_initialized() {
        assert!(LazyError::AlreadyInitialized.is_already_initialized());
        assert!(!LazyError::NotInitialized.is_already_initialized());
        assert!(!LazyError::ComputationFailed("x".into()).is_already_initialized());
    }

    #[test]
    fn lazy_from_value_already_initialized() {
        let lazy = lazy_from_value(99);
        assert!(lazy.is_initialized());
        assert_eq!(lazy.try_get(), Some(&99));
    }

    #[test]
    fn lazy_from_value_into_inner() {
        let lazy = lazy_from_value("hello".to_string());
        assert_eq!(lazy.into_inner(), Some("hello".to_string()));
    }

    #[test]
    fn memoized_fn_with_fn_trait() {
        let mut memo = MemoizedFn::new(|s: &String| s.len());
        assert_eq!(*memo.call("hello".to_string()), 5);
        assert_eq!(*memo.call("hi".to_string()), 2);
        assert_eq!(memo.len(), 2);
        assert_eq!(*memo.call("hello".to_string()), 5); // cached
        assert_eq!(memo.len(), 2); // no new entry
        memo.clear();
        assert!(memo.is_empty());
    }
}
