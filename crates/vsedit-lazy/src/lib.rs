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

// ── LazyPool ──

/// A pool of lazy values keyed by string identifiers.
///
/// Each entry stores a lazily-initialized value together with an optional TTL.
/// Once the TTL has elapsed the value is considered expired and will be
/// re-initialized on the next access.
pub struct LazyPool<T> {
    entries: HashMap<String, LazyPoolEntry<T>>,
}

struct LazyPoolEntry<T> {
    value: Option<T>,
    factory: Box<dyn Fn() -> T>,
    created_at: Option<Instant>,
    ttl: Option<Duration>,
}

impl<T> LazyPool<T> {
    /// Create an empty pool.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a key with a factory function and an optional TTL.
    pub fn register(
        &mut self,
        key: impl Into<String>,
        ttl: Option<Duration>,
        factory: impl Fn() -> T + 'static,
    ) {
        self.entries.insert(
            key.into(),
            LazyPoolEntry {
                value: None,
                factory: Box::new(factory),
                created_at: None,
                ttl,
            },
        );
    }

    /// Get a reference to the value for `key`, initializing or refreshing it as
    /// necessary.  Returns `None` when the key has not been registered.
    pub fn get(&mut self, key: &str) -> Option<&T> {
        let entry = self.entries.get_mut(key)?;
        let expired = match (entry.created_at, entry.ttl) {
            (Some(created), Some(ttl)) => created.elapsed() >= ttl,
            _ => false,
        };
        if entry.value.is_none() || expired {
            entry.value = Some((entry.factory)());
            entry.created_at = Some(Instant::now());
        }
        entry.value.as_ref()
    }

    /// Return `true` if the key exists in the pool (whether initialized or
    /// not).
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// Return the number of registered keys.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` when no keys have been registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Evict the cached value for `key` so it will be re-created on next
    /// access.
    pub fn invalidate(&mut self, key: &str) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.value = None;
            entry.created_at = None;
        }
    }

    /// Evict all cached values.
    pub fn invalidate_all(&mut self) {
        for entry in self.entries.values_mut() {
            entry.value = None;
            entry.created_at = None;
        }
    }

    /// Return all keys that currently hold an initialized value.
    pub fn initialized_keys(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter_map(|(k, e)| if e.value.is_some() { Some(k.as_str()) } else { None })
            .collect()
    }

    /// Check whether the cached value for `key` has expired.  Returns `None`
    /// when the key is unknown.
    pub fn is_expired(&self, key: &str) -> Option<bool> {
        let entry = self.entries.get(key)?;
        Some(match (entry.created_at, entry.ttl) {
            (Some(created), Some(ttl)) => created.elapsed() >= ttl,
            (None, _) => true, // never initialized ⇒ treat as expired
            _ => false,
        })
    }
}

impl<T> Default for LazyPool<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Batch initialization ──

/// Initialize multiple lazy values at once, returning results keyed by name.
pub fn lazy_batch_init<T>(
    items: Vec<(String, Box<dyn FnOnce() -> T>)>,
) -> HashMap<String, T> {
    let mut results = HashMap::with_capacity(items.len());
    for (key, factory) in items {
        results.insert(key, factory());
    }
    results
}

// ── Lazy chain ──

/// A lazy value whose initializer depends on the result of another lazy value.
///
/// `LazyChain` links two deferred computations: the *source* lazy produces a
/// `T`, and a *transform* converts it into a `U`.  Neither runs until the
/// chain is resolved.
pub struct LazyChain<T, U> {
    source: Option<Box<dyn FnOnce() -> T>>,
    transform: Option<Box<dyn FnOnce(&T) -> U>>,
    source_value: Option<T>,
    result: Option<U>,
}

impl<T, U> LazyChain<T, U> {
    /// Create a new lazy chain.
    pub fn new(
        source: impl FnOnce() -> T + 'static,
        transform: impl FnOnce(&T) -> U + 'static,
    ) -> Self {
        Self {
            source: Some(Box::new(source)),
            transform: Some(Box::new(transform)),
            source_value: None,
            result: None,
        }
    }

    /// Resolve the chain, returning a reference to the final value.
    pub fn resolve(&mut self) -> &U {
        if self.result.is_none() {
            if self.source_value.is_none() {
                let src_fn = self.source.take().expect("source already consumed");
                self.source_value = Some(src_fn());
            }
            let tf = self.transform.take().expect("transform already consumed");
            let source_val = self.source_value.as_ref().unwrap();
            self.result = Some(tf(source_val));
        }
        self.result.as_ref().unwrap()
    }

    /// Return `true` when the chain has already been resolved.
    pub fn is_resolved(&self) -> bool {
        self.result.is_some()
    }

    /// Access the intermediate source value, if already computed.
    pub fn source_value(&self) -> Option<&T> {
        self.source_value.as_ref()
    }
}

// ── LazyExpiring ──

/// A single lazy value with built-in expiration.
pub struct LazyExpiring<T> {
    value: Option<T>,
    factory: Box<dyn Fn() -> T>,
    created_at: Option<Instant>,
    ttl: Duration,
}

impl<T> LazyExpiring<T> {
    /// Create a new expiring lazy value.
    pub fn new(ttl: Duration, factory: impl Fn() -> T + 'static) -> Self {
        Self {
            value: None,
            factory: Box::new(factory),
            created_at: None,
            ttl,
        }
    }

    /// Get the value, initializing or refreshing if expired.
    pub fn get(&mut self) -> &T {
        let expired = self
            .created_at
            .map(|c| c.elapsed() >= self.ttl)
            .unwrap_or(true);
        if expired {
            self.value = Some((self.factory)());
            self.created_at = Some(Instant::now());
        }
        self.value.as_ref().unwrap()
    }

    /// Return the remaining time-to-live, or `None` if not yet initialized.
    pub fn remaining_ttl(&self) -> Option<Duration> {
        self.created_at
            .map(|c| self.ttl.saturating_sub(c.elapsed()))
    }

    /// Return `true` when the cached value has expired.
    pub fn is_expired(&self) -> bool {
        self.created_at
            .map(|c| c.elapsed() >= self.ttl)
            .unwrap_or(true)
    }

    /// Manually expire the cached value.
    pub fn expire(&mut self) {
        self.value = None;
        self.created_at = None;
    }
}

// ---------------------------------------------------------------------------
// Lazy utility functions
// ---------------------------------------------------------------------------

/// Creates a `Lazy<T>` that is already initialized with the given value.
pub fn lazy_of<T: 'static>(value: T) -> Lazy<T> {
    let mut lazy = Lazy::new(move || unreachable!());
    let _ = lazy.cell.set(value);
    lazy.init = None;
    lazy
}

/// Creates a `CachedValue<T>` whose compute function always returns `value`.
pub fn cached_constant<T: Clone + 'static>(value: T) -> CachedValue<T> {
    CachedValue::new(move || value.clone())
}

/// Runs a closure and returns both the result and the elapsed duration.
pub fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let result = f();
    (result, start.elapsed())
}

/// Returns `true` if a `CachedValue` is cached and the cached value satisfies
/// the given predicate.
pub fn cached_matches<T>(cache: &CachedValue<T>, pred: impl FnOnce(&T) -> bool) -> bool {
    cache.get_if_cached().map_or(false, pred)
}

/// Returns `true` if a `MemoizedFn` already has a cached result for the key.
pub fn memo_contains<K: Eq + std::hash::Hash + Clone, V>(
    memo: &MemoizedFn<K, V>,
    key: &K,
) -> bool {
    memo.cache.contains_key(key)
}

/// Creates a `MemoizedFn<String, usize>` that computes string lengths.
pub fn memo_strlen() -> MemoizedFn<String, usize> {
    MemoizedFn::new(|s: &String| s.len())
}

/// Helper: measure how many times a cached value has been refreshed by
/// wrapping a counter-incrementing closure.
pub fn counting_cached(counter: std::rc::Rc<std::cell::Cell<u32>>) -> CachedValue<u32> {
    CachedValue::new(move || {
        let n = counter.get() + 1;
        counter.set(n);
        n
    })
}

// ---------------------------------------------------------------------------
// Lazy<T> – additional methods
// ---------------------------------------------------------------------------

impl<T: fmt::Debug> fmt::Debug for Lazy<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.cell.get() {
            Some(v) => f.debug_tuple("Lazy").field(v).finish(),
            None => f.write_str("Lazy(<uninit>)"),
        }
    }
}

impl<T: Clone> Lazy<T> {
    /// Clone the inner value if it has been initialized.
    pub fn cloned(&self) -> Option<T> {
        self.cell.get().cloned()
    }
}

// ---------------------------------------------------------------------------
// SyncLazy<T> – additional methods
// ---------------------------------------------------------------------------

impl<T> SyncLazy<T> {
    /// Try to get the value without initializing it.
    pub fn try_get(&self) -> Option<&T> {
        self.cell.get()
    }
}

impl<T: fmt::Debug> fmt::Debug for SyncLazy<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.cell.get() {
            Some(v) => f.debug_tuple("SyncLazy").field(v).finish(),
            None => f.write_str("SyncLazy(<uninit>)"),
        }
    }
}

// ---------------------------------------------------------------------------
// MemoizedFn – additional methods
// ---------------------------------------------------------------------------

impl<K: Eq + std::hash::Hash + Clone, V> MemoizedFn<K, V> {
    /// Remove a single entry from the cache, returning the value if present.
    pub fn evict(&mut self, key: &K) -> Option<V> {
        self.cache.remove(key)
    }

    /// Return `true` if the cache contains an entry for `key`.
    pub fn contains(&self, key: &K) -> bool {
        self.cache.contains_key(key)
    }

    /// Return an iterator over all cached keys.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.cache.keys()
    }

    /// Peek at a cached value without triggering computation.
    pub fn peek(&self, key: &K) -> Option<&V> {
        self.cache.get(key)
    }
}

// ---------------------------------------------------------------------------
// LazyStats – additional methods
// ---------------------------------------------------------------------------

impl LazyStats {
    /// Return the number of successful operations.
    pub fn successes(&self) -> u64 {
        self.successful_operations
    }

    /// Return the number of failed operations.
    pub fn failures(&self) -> u64 {
        self.failed_operations
    }

    /// Return total accumulated time in nanoseconds.
    pub fn total_time_ns(&self) -> u64 {
        self.total_time_ns
    }

    /// Return the median estimate (average of min and max) in nanoseconds.
    /// Returns `None` when no operations have been recorded.
    pub fn midrange_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            return None;
        }
        Some((self.min_operation_ns + self.max_operation_ns) / 2)
    }
}

// ---------------------------------------------------------------------------
// LazyValidator – additional methods
// ---------------------------------------------------------------------------

impl LazyValidator {
    /// Validate that a string is non-empty and at most `max` bytes long.
    pub fn validate_byte_length(s: &str, max: usize) -> Result<(), String> {
        if s.is_empty() {
            return Err("string must not be empty".to_string());
        }
        if s.len() > max {
            return Err(format!(
                "byte length {} exceeds maximum {}",
                s.len(),
                max
            ));
        }
        Ok(())
    }

    /// Normalize whitespace: collapse runs of whitespace into a single space
    /// and trim leading/trailing whitespace.
    pub fn normalize_whitespace(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut prev_ws = true; // treat start as whitespace to trim leading
        for ch in s.chars() {
            if ch.is_whitespace() {
                if !prev_ws {
                    result.push(' ');
                }
                prev_ws = true;
            } else {
                result.push(ch);
                prev_ws = false;
            }
        }
        // trim trailing space
        if result.ends_with(' ') {
            result.pop();
        }
        result
    }
}

// ---------------------------------------------------------------------------
// LazyFallback
// ---------------------------------------------------------------------------

/// A lazy value with a fallback computation used when the primary fails.
pub struct LazyFallback<T> {
    primary: Option<Box<dyn FnOnce() -> Result<T, String>>>,
    fallback: Option<Box<dyn FnOnce() -> T>>,
    value: Option<T>,
    used_fallback: bool,
}

impl<T> LazyFallback<T> {
    /// Create a new lazy value with a primary and fallback computation.
    pub fn new(
        primary: impl FnOnce() -> Result<T, String> + 'static,
        fallback: impl FnOnce() -> T + 'static,
    ) -> Self {
        Self {
            primary: Some(Box::new(primary)),
            fallback: Some(Box::new(fallback)),
            value: None,
            used_fallback: false,
        }
    }

    /// Get the value, trying the primary computation first and falling back
    /// if it returns an error.
    pub fn get(&mut self) -> &T {
        if self.value.is_none() {
            let primary = self.primary.take().expect("primary already consumed");
            match primary() {
                Ok(v) => {
                    self.value = Some(v);
                }
                Err(_) => {
                    let fallback = self.fallback.take().expect("fallback already consumed");
                    self.value = Some(fallback());
                    self.used_fallback = true;
                }
            }
        }
        self.value.as_ref().unwrap()
    }

    /// Return `true` if the fallback was used.
    pub fn used_fallback(&self) -> bool {
        self.used_fallback
    }

    /// Return `true` if the value has been resolved.
    pub fn is_resolved(&self) -> bool {
        self.value.is_some()
    }
}

// ---------------------------------------------------------------------------
// WriteOnceLazy
// ---------------------------------------------------------------------------

/// A value that can be set exactly once and then read many times.
/// Unlike `Lazy`, the value is provided externally rather than by a closure.
pub struct WriteOnceLazy<T> {
    cell: OnceCell<T>,
}

impl<T> WriteOnceLazy<T> {
    /// Create a new empty write-once cell.
    pub fn new() -> Self {
        Self {
            cell: OnceCell::new(),
        }
    }

    /// Set the value. Returns `Err` with the value back if already set.
    pub fn set(&self, value: T) -> Result<(), T> {
        self.cell.set(value).map_err(|v| v)
    }

    /// Get the value. Returns `None` if not yet set.
    pub fn get(&self) -> Option<&T> {
        self.cell.get()
    }

    /// Return `true` if a value has been written.
    pub fn is_set(&self) -> bool {
        self.cell.get().is_some()
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> Option<T> {
        self.cell.into_inner()
    }
}

impl<T> Default for WriteOnceLazy<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for WriteOnceLazy<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.cell.get() {
            Some(v) => f.debug_tuple("WriteOnceLazy").field(v).finish(),
            None => f.write_str("WriteOnceLazy(<empty>)"),
        }
    }
}

// ── LazyMap ──

/// A map whose values are lazily initialized on first access.
///
/// Each key is associated with an initializer closure. The closure runs at most
/// once, on the first call to [`get`](LazyMap::get).
pub struct LazyMap {
    initializers: HashMap<String, Box<dyn Fn() -> String>>,
    values: HashMap<String, String>,
}

impl LazyMap {
    /// Create an empty `LazyMap`.
    pub fn new() -> Self {
        Self {
            initializers: HashMap::new(),
            values: HashMap::new(),
        }
    }

    /// Register a key with its initializer closure.
    pub fn insert(&mut self, key: String, initializer: Box<dyn Fn() -> String>) {
        self.initializers.insert(key, initializer);
    }

    /// Get the value for `key`, initializing it on first access.
    pub fn get(&mut self, key: &str) -> Option<&String> {
        if !self.values.contains_key(key) {
            if let Some(init) = self.initializers.get(key) {
                let val = init();
                self.values.insert(key.to_string(), val);
            }
        }
        self.values.get(key)
    }

    /// Return `true` if the value for `key` has already been initialized.
    pub fn is_initialized(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// Number of registered keys (initialized or not).
    pub fn key_count(&self) -> usize {
        self.initializers.len()
    }

    /// Number of keys whose values have been materialized.
    pub fn initialized_count(&self) -> usize {
        self.values.len()
    }
}

impl Default for LazyMap {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LazyMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LazyMap({}/{} initialized)",
            self.initialized_count(),
            self.key_count(),
        )
    }
}

// ── LazyPipeline ──

/// A sequential chain of transformations applied to an initial `i64` value.
///
/// Steps are accumulated with [`then`](LazyPipeline::then) and executed lazily
/// by [`evaluate`](LazyPipeline::evaluate).
pub struct LazyPipeline {
    initial: i64,
    steps: Vec<Box<dyn Fn(i64) -> i64>>,
}

impl LazyPipeline {
    /// Create a new pipeline starting from `initial`.
    pub fn new(initial: i64) -> Self {
        Self {
            initial,
            steps: Vec::new(),
        }
    }

    /// Append a transformation step.
    pub fn then(mut self, f: Box<dyn Fn(i64) -> i64>) -> Self {
        self.steps.push(f);
        self
    }

    /// Run all steps sequentially and return the final value.
    pub fn evaluate(&self) -> i64 {
        self.steps.iter().fold(self.initial, |acc, f| f(acc))
    }

    /// Number of registered steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
}

impl fmt::Display for LazyPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LazyPipeline(initial={}, steps={})",
            self.initial,
            self.steps.len(),
        )
    }
}

// ── LazyProfile ──

/// Tracks initialization durations for profiling lazy evaluations.
pub struct LazyProfile {
    records: Vec<(String, u64)>,
}

impl LazyProfile {
    /// Create an empty profile.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Record an initialization event with the given `name` and `duration_us`
    /// (microseconds).
    pub fn record_init(&mut self, name: &str, duration_us: u64) {
        self.records.push((name.to_string(), duration_us));
    }

    /// Look up the duration recorded under `name`.
    pub fn get_duration(&self, name: &str) -> Option<u64> {
        self.records
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| *d)
    }

    /// Sum of all recorded durations.
    pub fn total_duration(&self) -> u64 {
        self.records.iter().map(|(_, d)| *d).sum()
    }

    /// The entry with the longest duration.
    pub fn slowest(&self) -> Option<(&str, u64)> {
        self.records
            .iter()
            .max_by_key(|(_, d)| *d)
            .map(|(n, d)| (n.as_str(), *d))
    }

    /// The entry with the shortest duration.
    pub fn fastest(&self) -> Option<(&str, u64)> {
        self.records
            .iter()
            .min_by_key(|(_, d)| *d)
            .map(|(n, d)| (n.as_str(), *d))
    }

    /// Mean duration across all entries. Returns `0.0` when empty.
    pub fn average_duration(&self) -> f64 {
        if self.records.is_empty() {
            return 0.0;
        }
        self.total_duration() as f64 / self.records.len() as f64
    }

    /// Number of recorded initializations.
    pub fn init_count(&self) -> usize {
        self.records.len()
    }
}

impl Default for LazyProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LazyProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LazyProfile({} inits, total={}µs)",
            self.init_count(),
            self.total_duration(),
        )
    }
}

// ── LazyFactory ──

/// Registry of named factory closures that produce `String` values on demand.
pub struct LazyFactory {
    factories: HashMap<String, Box<dyn Fn() -> String>>,
}

impl LazyFactory {
    /// Create an empty factory registry.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a factory under `type_name`.
    pub fn register(&mut self, type_name: &str, factory: Box<dyn Fn() -> String>) {
        self.factories.insert(type_name.to_string(), factory);
    }

    /// Invoke the factory registered under `type_name`, if any.
    pub fn create(&self, type_name: &str) -> Option<String> {
        self.factories.get(type_name).map(|f| f())
    }

    /// List all registered type names.
    pub fn registered_types(&self) -> Vec<&str> {
        self.factories.keys().map(|k| k.as_str()).collect()
    }

    /// Return `true` if `type_name` has been registered.
    pub fn is_registered(&self, type_name: &str) -> bool {
        self.factories.contains_key(type_name)
    }

    /// Number of registered factories.
    pub fn type_count(&self) -> usize {
        self.factories.len()
    }
}

impl Default for LazyFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LazyFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LazyFactory({} types)", self.type_count())
    }
}

// --- LruCache: least-recently-used cache ---

pub struct LruCache<K: Eq + std::hash::Hash + Clone, V: Clone> {
    capacity: usize,
    entries: Vec<(K, V)>,
    hits: usize,
    misses: usize,
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> LruCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self { capacity: capacity.max(1), entries: Vec::new(), hits: 0, misses: 0 }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(idx) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(idx);
            self.entries.push(entry);
            self.hits += 1;
            self.entries.last().map(|(_, v)| v)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if let Some(idx) = self.entries.iter().position(|(k, _)| *k == key) {
            self.entries.remove(idx);
        }
        if self.entries.len() >= self.capacity {
            self.evict_oldest();
        }
        self.entries.push((key, value));
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(idx) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(idx).1)
        } else {
            None
        }
    }

    pub fn evict_oldest(&mut self) -> Option<(K, V)> {
        if self.entries.is_empty() { None } else { Some(self.entries.remove(0)) }
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn capacity(&self) -> usize { self.capacity }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }
}

// --- ExpiringCacheEntry / ExpiringCache ---

struct ExpiringEntry<V> {
    value: V,
    inserted_at: Instant,
    ttl: Duration,
}

pub struct ExpiringCache<K: Eq + std::hash::Hash + Clone, V: Clone> {
    entries: HashMap<K, ExpiringEntry<V>>,
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> ExpiringCache<K, V> {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    pub fn insert_with_ttl(&mut self, key: K, value: V, ttl: Duration) {
        self.entries.insert(key, ExpiringEntry { value, inserted_at: Instant::now(), ttl });
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).and_then(|e| {
            if e.inserted_at.elapsed() < e.ttl { Some(&e.value) } else { None }
        })
    }

    pub fn remove_expired(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, e| e.inserted_at.elapsed() < e.ttl);
        before - self.entries.len()
    }

    pub fn entry_count(&self) -> usize { self.entries.len() }

    pub fn expired_count(&self) -> usize {
        self.entries.values().filter(|e| e.inserted_at.elapsed() >= e.ttl).count()
    }
}

// --- ComputeOnce ---

pub struct ComputeOnce<T> {
    value: Option<T>,
}

impl<T> ComputeOnce<T> {
    pub fn new() -> Self { Self { value: None } }

    pub fn is_computed(&self) -> bool { self.value.is_some() }

    pub fn reset(&mut self) { self.value = None; }

    pub fn get_or_init<F: FnOnce() -> T>(&mut self, f: F) -> &T {
        if self.value.is_none() {
            self.value = Some(f());
        }
        self.value.as_ref().unwrap()
    }

    pub fn get(&self) -> Option<&T> { self.value.as_ref() }
}


// ---------------------------------------------------------------------------
// lazy – Data validation and analysis helpers
// ---------------------------------------------------------------------------

/// Result of validating a value against a schema-like rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XLazyValidationResult {
    Ok,
    Error(String),
    Warning(String),
}

impl XLazyValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error(m) | Self::Warning(m) => Some(m),
        }
    }
}

/// A key-value pair with optional metadata tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XLazyTaggedEntry {
    pub key: String,
    pub value: String,
    pub tag: Option<String>,
}

impl XLazyTaggedEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into(), tag: None }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn matches_tag(&self, tag: &str) -> bool {
        self.tag.as_deref() == Some(tag)
    }
}

/// Validate that a string is non-empty and within a max length.
pub fn x_lazy_validate_string(value: &str, max_len: usize) -> XLazyValidationResult {
    if value.is_empty() {
        return XLazyValidationResult::Error("value must not be empty".into());
    }
    if value.len() > max_len {
        return XLazyValidationResult::Error(
            format!("value exceeds max length of {max_len}"),
        );
    }
    XLazyValidationResult::Ok
}

/// Validate that a number falls within an inclusive range.
pub fn x_lazy_validate_range(value: i64, min: i64, max: i64) -> XLazyValidationResult {
    if value < min || value > max {
        XLazyValidationResult::Error(
            format!("{value} is outside range [{min}, {max}]"),
        )
    } else {
        XLazyValidationResult::Ok
    }
}

/// Filter entries by tag, returning only matching ones.
pub fn x_lazy_filter_by_tag<'a>(
    entries: &'a [XLazyTaggedEntry],
    tag: &str,
) -> Vec<&'a XLazyTaggedEntry> {
    entries.iter().filter(|e| e.matches_tag(tag)).collect()
}

/// Group entries by their tag (entries without a tag go under `"_untagged"`).
pub fn x_lazy_group_by_tag(
    entries: &[XLazyTaggedEntry],
) -> std::collections::HashMap<String, Vec<&XLazyTaggedEntry>> {
    let mut map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for e in entries {
        let key = e.tag.clone().unwrap_or_else(|| "_untagged".into());
        map.entry(key).or_default().push(e);
    }
    map
}

/// Compute a simple digest of a string (DJB2 hash).
pub fn x_lazy_djb2_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// Deduplicate entries by key, keeping the first occurrence.
pub fn x_lazy_dedup_entries(entries: Vec<XLazyTaggedEntry>) -> Vec<XLazyTaggedEntry> {
    let mut seen = std::collections::HashSet::new();
    entries.into_iter().filter(|e| seen.insert(e.key.clone())).collect()
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

    // ── LazyPool tests ──

    #[test]
    fn lazy_pool_register_and_get() {
        let mut pool: LazyPool<i32> = LazyPool::new();
        pool.register("a", None, || 42);
        pool.register("b", None, || 99);
        assert_eq!(pool.len(), 2);
        assert_eq!(*pool.get("a").unwrap(), 42);
        assert_eq!(*pool.get("b").unwrap(), 99);
        assert!(pool.get("c").is_none());
    }

    #[test]
    fn lazy_pool_invalidate() {
        let counter = Rc::new(Cell::new(0));
        let c2 = counter.clone();
        let mut pool: LazyPool<i32> = LazyPool::new();
        pool.register("x", None, move || {
            c2.set(c2.get() + 1);
            c2.get()
        });
        assert_eq!(*pool.get("x").unwrap(), 1);
        assert_eq!(*pool.get("x").unwrap(), 1); // cached
        pool.invalidate("x");
        assert_eq!(*pool.get("x").unwrap(), 2); // re-initialized
    }

    #[test]
    fn lazy_pool_initialized_keys() {
        let mut pool: LazyPool<i32> = LazyPool::new();
        pool.register("a", None, || 1);
        pool.register("b", None, || 2);
        assert!(pool.initialized_keys().is_empty());
        pool.get("a");
        let keys = pool.initialized_keys();
        assert_eq!(keys.len(), 1);
        assert!(keys.contains(&"a"));
    }

    #[test]
    fn lazy_pool_invalidate_all() {
        let mut pool: LazyPool<i32> = LazyPool::new();
        pool.register("a", None, || 1);
        pool.register("b", None, || 2);
        pool.get("a");
        pool.get("b");
        assert_eq!(pool.initialized_keys().len(), 2);
        pool.invalidate_all();
        assert!(pool.initialized_keys().is_empty());
    }

    // ── LazyChain tests ──

    #[test]
    fn lazy_chain_resolves() {
        let mut chain = LazyChain::new(|| vec![1, 2, 3], |v| v.iter().sum::<i32>());
        assert!(!chain.is_resolved());
        assert_eq!(*chain.resolve(), 6);
        assert!(chain.is_resolved());
        assert_eq!(chain.source_value().unwrap(), &vec![1, 2, 3]);
    }

    // ── Batch init tests ──

    #[test]
    fn lazy_batch_init_basic() {
        let items: Vec<(String, Box<dyn FnOnce() -> i32>)> = vec![
            ("x".into(), Box::new(|| 10)),
            ("y".into(), Box::new(|| 20)),
        ];
        let results = lazy_batch_init(items);
        assert_eq!(results.len(), 2);
        assert_eq!(*results.get("x").unwrap(), 10);
        assert_eq!(*results.get("y").unwrap(), 20);
    }

    // ── LazyExpiring tests ──

    #[test]
    fn lazy_expiring_get_and_expire() {
        let counter = Rc::new(Cell::new(0));
        let c2 = counter.clone();
        let mut lazy = LazyExpiring::new(Duration::from_secs(3600), move || {
            c2.set(c2.get() + 1);
            c2.get()
        });
        assert!(lazy.is_expired());
        assert_eq!(*lazy.get(), 1);
        assert!(!lazy.is_expired());
        assert!(lazy.remaining_ttl().unwrap() > Duration::ZERO);
        lazy.expire();
        assert!(lazy.is_expired());
        assert_eq!(*lazy.get(), 2);
    }

    // --- new tests ---

    #[test]
    fn test_lazy_of_already_initialized() {
        let mut l = lazy_of(42);
        assert!(l.is_initialized());
        assert_eq!(*l.get(), 42);
    }

    #[test]
    fn test_lazy_of_into_inner() {
        let l = lazy_of("hello".to_string());
        assert_eq!(l.into_inner(), Some("hello".to_string()));
    }

    #[test]
    fn test_cached_constant_returns_value() {
        let mut c = cached_constant(99);
        assert_eq!(*c.get(), 99);
        c.invalidate();
        assert_eq!(*c.get(), 99);
    }

    #[test]
    fn test_timed_returns_result_and_duration() {
        let (val, dur) = timed(|| 2 + 3);
        assert_eq!(val, 5);
        assert!(dur.as_nanos() < 1_000_000_000); // less than 1s
    }

    #[test]
    fn test_cached_matches_true() {
        let mut c = cached_constant(10);
        c.get(); // initialise
        assert!(cached_matches(&c, |v| *v == 10));
    }

    #[test]
    fn test_cached_matches_false_not_cached() {
        let c = cached_constant(10);
        assert!(!cached_matches(&c, |v| *v == 10));
    }

    #[test]
    fn test_memo_contains_after_call() {
        let mut m = memo_strlen();
        m.call("abc".to_string());
        assert!(memo_contains(&m, &"abc".to_string()));
        assert!(!memo_contains(&m, &"xyz".to_string()));
    }

    #[test]
    fn test_memo_strlen() {
        let mut m = memo_strlen();
        assert_eq!(*m.call("hello".to_string()), 5);
        assert_eq!(*m.call("".to_string()), 0);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn test_counting_cached_increments() {
        let counter = Rc::new(Cell::new(0));
        let mut c = counting_cached(counter.clone());
        assert_eq!(*c.get(), 1);
        assert_eq!(*c.get(), 1); // still cached
        c.invalidate();
        assert_eq!(*c.get(), 2);
        assert_eq!(counter.get(), 2);
    }

    // ── New tests for extended functionality ──

    #[test]
    fn lazy_debug_uninit() {
        let lazy = Lazy::new(|| 42);
        assert_eq!(format!("{:?}", lazy), "Lazy(<uninit>)");
    }

    #[test]
    fn lazy_debug_init() {
        let mut lazy = Lazy::new(|| 42);
        lazy.get();
        assert_eq!(format!("{:?}", lazy), "Lazy(42)");
    }

    #[test]
    fn lazy_cloned_returns_none_uninit() {
        let lazy = Lazy::new(|| 42);
        assert_eq!(lazy.cloned(), None);
    }

    #[test]
    fn lazy_cloned_returns_value() {
        let mut lazy = Lazy::new(|| String::from("val"));
        lazy.get();
        assert_eq!(lazy.cloned(), Some(String::from("val")));
    }

    #[test]
    fn sync_lazy_try_get_before_init() {
        let sl = SyncLazy::new(|| 7);
        assert!(sl.try_get().is_none());
    }

    #[test]
    fn sync_lazy_try_get_after_init() {
        let sl = SyncLazy::new(|| 7);
        sl.get();
        assert_eq!(sl.try_get(), Some(&7));
    }

    #[test]
    fn sync_lazy_debug_uninit() {
        let sl = SyncLazy::new(|| 99);
        assert_eq!(format!("{:?}", sl), "SyncLazy(<uninit>)");
    }

    #[test]
    fn sync_lazy_debug_init() {
        let sl = SyncLazy::new(|| 99);
        sl.get();
        assert_eq!(format!("{:?}", sl), "SyncLazy(99)");
    }

    #[test]
    fn memoized_fn_evict() {
        let mut memo = MemoizedFn::new(|k: &i32| k * 10);
        memo.call(1);
        memo.call(2);
        assert_eq!(memo.len(), 2);
        let evicted = memo.evict(&1);
        assert_eq!(evicted, Some(10));
        assert_eq!(memo.len(), 1);
        assert!(!memo.contains(&1));
        assert!(memo.contains(&2));
    }

    #[test]
    fn memoized_fn_peek() {
        let mut memo = MemoizedFn::new(|k: &i32| k + 100);
        assert_eq!(memo.peek(&5), None);
        memo.call(5);
        assert_eq!(memo.peek(&5), Some(&105));
    }

    #[test]
    fn memoized_fn_keys() {
        let mut memo = MemoizedFn::new(|k: &String| k.len());
        memo.call("abc".to_string());
        memo.call("de".to_string());
        let mut keys: Vec<&String> = memo.keys().collect();
        keys.sort();
        assert_eq!(keys, vec![&"abc".to_string(), &"de".to_string()]);
    }

    #[test]
    fn lazy_stats_accessors() {
        let mut stats = LazyStats::new();
        stats.record_success(100);
        stats.record_failure(200);
        assert_eq!(stats.successes(), 1);
        assert_eq!(stats.failures(), 1);
        assert_eq!(stats.total_time_ns(), 300);
    }

    #[test]
    fn lazy_stats_midrange() {
        let stats = LazyStats::new();
        assert_eq!(stats.midrange_ns(), None);

        let mut stats = LazyStats::new();
        stats.record_success(100);
        stats.record_success(300);
        assert_eq!(stats.midrange_ns(), Some(200));
    }

    #[test]
    fn lazy_validator_byte_length_ok() {
        assert!(LazyValidator::validate_byte_length("hi", 10).is_ok());
    }

    #[test]
    fn lazy_validator_byte_length_empty() {
        assert!(LazyValidator::validate_byte_length("", 10).is_err());
    }

    #[test]
    fn lazy_validator_byte_length_too_long() {
        assert!(LazyValidator::validate_byte_length("toolong", 3).is_err());
    }

    #[test]
    fn lazy_validator_normalize_whitespace() {
        assert_eq!(
            LazyValidator::normalize_whitespace("  hello   world  "),
            "hello world"
        );
        assert_eq!(
            LazyValidator::normalize_whitespace("no_extra"),
            "no_extra"
        );
        assert_eq!(
            LazyValidator::normalize_whitespace("  a\t\nb  "),
            "a b"
        );
    }

    #[test]
    fn lazy_fallback_uses_primary() {
        let mut fb = LazyFallback::new(|| Ok(42), || 0);
        assert!(!fb.is_resolved());
        assert_eq!(*fb.get(), 42);
        assert!(fb.is_resolved());
        assert!(!fb.used_fallback());
    }

    #[test]
    fn lazy_fallback_uses_fallback_on_error() {
        let mut fb = LazyFallback::new(|| Err("fail".to_string()), || 99);
        assert_eq!(*fb.get(), 99);
        assert!(fb.used_fallback());
    }

    #[test]
    fn write_once_lazy_set_and_get() {
        let wol = WriteOnceLazy::new();
        assert!(!wol.is_set());
        assert!(wol.get().is_none());
        assert!(wol.set(42).is_ok());
        assert!(wol.is_set());
        assert_eq!(wol.get(), Some(&42));
    }

    #[test]
    fn write_once_lazy_double_set_fails() {
        let wol = WriteOnceLazy::new();
        assert!(wol.set(1).is_ok());
        assert!(wol.set(2).is_err());
        assert_eq!(wol.get(), Some(&1));
    }

    #[test]
    fn write_once_lazy_into_inner() {
        let wol = WriteOnceLazy::new();
        wol.set("hello".to_string()).unwrap();
        assert_eq!(wol.into_inner(), Some("hello".to_string()));
    }

    #[test]
    fn write_once_lazy_debug() {
        let wol: WriteOnceLazy<i32> = WriteOnceLazy::new();
        assert_eq!(format!("{:?}", wol), "WriteOnceLazy(<empty>)");
        wol.set(7).unwrap();
        assert_eq!(format!("{:?}", wol), "WriteOnceLazy(7)");
    }

    // ── LazyMap tests ──

    #[test]
    fn lazy_map_initializes_on_first_access() {
        let mut map = LazyMap::new();
        let counter = Rc::new(Cell::new(0u32));
        let c = counter.clone();
        map.insert("key".to_string(), Box::new(move || {
            c.set(c.get() + 1);
            "value".to_string()
        }));
        assert!(!map.is_initialized("key"));
        assert_eq!(map.get("key"), Some(&"value".to_string()));
        assert!(map.is_initialized("key"));
        // second access must not re-run the initializer
        assert_eq!(map.get("key"), Some(&"value".to_string()));
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn lazy_map_missing_key_returns_none() {
        let mut map = LazyMap::new();
        assert_eq!(map.get("missing"), None);
    }

    #[test]
    fn lazy_map_counts() {
        let mut map = LazyMap::new();
        map.insert("a".into(), Box::new(|| "1".into()));
        map.insert("b".into(), Box::new(|| "2".into()));
        assert_eq!(map.key_count(), 2);
        assert_eq!(map.initialized_count(), 0);
        map.get("a");
        assert_eq!(map.initialized_count(), 1);
        assert_eq!(format!("{map}"), "LazyMap(1/2 initialized)");
    }

    // ── LazyPipeline tests ──

    #[test]
    fn lazy_pipeline_no_steps() {
        let chain = LazyPipeline::new(10);
        assert_eq!(chain.evaluate(), 10);
        assert_eq!(chain.step_count(), 0);
    }

    #[test]
    fn lazy_pipeline_sequential_steps() {
        let chain = LazyPipeline::new(2)
            .then(Box::new(|x| x * 3))
            .then(Box::new(|x| x + 10))
            .then(Box::new(|x| x * 2));
        assert_eq!(chain.step_count(), 3);
        // (2 * 3 + 10) * 2 = 32
        assert_eq!(chain.evaluate(), 32);
    }

    #[test]
    fn lazy_pipeline_display() {
        let chain = LazyPipeline::new(5).then(Box::new(|x| x + 1));
        assert_eq!(format!("{chain}"), "LazyPipeline(initial=5, steps=1)");
    }

    // ── LazyProfile tests ──

    #[test]
    fn lazy_profile_records_and_queries() {
        let mut profile = LazyProfile::new();
        profile.record_init("alpha", 100);
        profile.record_init("beta", 300);
        profile.record_init("gamma", 200);

        assert_eq!(profile.get_duration("alpha"), Some(100));
        assert_eq!(profile.get_duration("missing"), None);
        assert_eq!(profile.total_duration(), 600);
        assert_eq!(profile.slowest(), Some(("beta", 300)));
        assert_eq!(profile.fastest(), Some(("alpha", 100)));
        assert!((profile.average_duration() - 200.0).abs() < f64::EPSILON);
        assert_eq!(profile.init_count(), 3);
    }

    #[test]
    fn lazy_profile_empty() {
        let profile = LazyProfile::new();
        assert_eq!(profile.total_duration(), 0);
        assert_eq!(profile.slowest(), None);
        assert_eq!(profile.fastest(), None);
        assert!((profile.average_duration() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lazy_profile_display() {
        let mut p = LazyProfile::new();
        p.record_init("x", 42);
        assert_eq!(format!("{p}"), "LazyProfile(1 inits, total=42µs)");
    }

    // ── LazyFactory tests ──

    #[test]
    fn lazy_factory_create() {
        let mut factory = LazyFactory::new();
        factory.register("greeting", Box::new(|| "hello".to_string()));
        assert!(factory.is_registered("greeting"));
        assert!(!factory.is_registered("other"));
        assert_eq!(factory.create("greeting"), Some("hello".to_string()));
        assert_eq!(factory.create("missing"), None);
    }

    #[test]
    fn lazy_factory_registered_types() {
        let mut factory = LazyFactory::new();
        factory.register("a", Box::new(|| "1".into()));
        factory.register("b", Box::new(|| "2".into()));
        assert_eq!(factory.type_count(), 2);
        let mut types = factory.registered_types();
        types.sort();
        assert_eq!(types, vec!["a", "b"]);
    }

    #[test]
    fn lazy_factory_display() {
        let factory = LazyFactory::new();
        assert_eq!(format!("{factory}"), "LazyFactory(0 types)");
    }

    #[test]
    fn write_once_lazy_default() {
        let wol: WriteOnceLazy<i32> = WriteOnceLazy::default();
        assert!(!wol.is_set());
    }

    #[test]
    fn lru_cache_insert_and_get() {
        let mut cache = LruCache::new(3);
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn lru_cache_evicts_oldest() {
        let mut cache = LruCache::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        assert!(!cache.contains_key(&"a"));
        assert!(cache.contains_key(&"b"));
        assert!(cache.contains_key(&"c"));
    }

    #[test]
    fn lru_cache_get_updates_recency() {
        let mut cache = LruCache::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.get(&"a"); // makes "a" most recent
        cache.insert("c", 3); // should evict "b"
        assert!(cache.contains_key(&"a"));
        assert!(!cache.contains_key(&"b"));
    }

    #[test]
    fn lru_cache_remove() {
        let mut cache = LruCache::new(5);
        cache.insert("x", 10);
        assert_eq!(cache.remove(&"x"), Some(10));
        assert!(cache.is_empty());
    }

    #[test]
    fn lru_cache_hit_rate() {
        let mut cache = LruCache::new(5);
        cache.insert("a", 1);
        cache.get(&"a"); // hit
        cache.get(&"b"); // miss
        assert!((cache.hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn lru_cache_hit_rate_empty() {
        let cache: LruCache<&str, i32> = LruCache::new(5);
        assert_eq!(cache.hit_rate(), 0.0);
    }

    #[test]
    fn expiring_cache_basic() {
        let mut cache = ExpiringCache::new();
        cache.insert_with_ttl("key", 42, Duration::from_secs(60));
        assert_eq!(cache.get(&"key"), Some(&42));
        assert_eq!(cache.entry_count(), 1);
    }

    #[test]
    fn expiring_cache_expired_count() {
        let mut cache = ExpiringCache::new();
        cache.insert_with_ttl("old", 1, Duration::from_nanos(1));
        std::thread::sleep(Duration::from_millis(1));
        assert!(cache.get(&"old").is_none());
        assert_eq!(cache.expired_count(), 1);
    }

    #[test]
    fn expiring_cache_remove_expired() {
        let mut cache = ExpiringCache::new();
        cache.insert_with_ttl("stale", 99, Duration::from_nanos(1));
        std::thread::sleep(Duration::from_millis(1));
        let removed = cache.remove_expired();
        assert_eq!(removed, 1);
        assert_eq!(cache.entry_count(), 0);
    }

    #[test]
    fn compute_once_initial_state() {
        let co: ComputeOnce<i32> = ComputeOnce::new();
        assert!(!co.is_computed());
        assert!(co.get().is_none());
    }

    #[test]
    fn compute_once_computes_value() {
        let mut co = ComputeOnce::new();
        let val = co.get_or_init(|| 42);
        assert_eq!(*val, 42);
        assert!(co.is_computed());
    }

    #[test]
    fn compute_once_reset() {
        let mut co = ComputeOnce::new();
        co.get_or_init(|| 10);
        co.reset();
        assert!(!co.is_computed());
        let val = co.get_or_init(|| 20);
        assert_eq!(*val, 20);
    }

    // -- lazy additional tests -------------------------------------------

    #[test]
    fn x_lazy_validation_ok() {
        let r = x_lazy_validate_string("hello", 100);
        assert!(r.is_ok());
        assert!(r.message().is_none());
    }

    #[test]
    fn x_lazy_validation_empty() {
        let r = x_lazy_validate_string("", 100);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("empty"));
    }

    #[test]
    fn x_lazy_validation_too_long() {
        let r = x_lazy_validate_string("abcdef", 3);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("max length"));
    }

    #[test]
    fn x_lazy_validate_range_ok() {
        assert!(x_lazy_validate_range(5, 1, 10).is_ok());
        assert!(x_lazy_validate_range(1, 1, 10).is_ok());
        assert!(x_lazy_validate_range(10, 1, 10).is_ok());
    }

    #[test]
    fn x_lazy_validate_range_out() {
        assert!(!x_lazy_validate_range(0, 1, 10).is_ok());
        assert!(!x_lazy_validate_range(11, 1, 10).is_ok());
    }

    #[test]
    fn x_lazy_tagged_entry_basic() {
        let e = XLazyTaggedEntry::new("k", "v");
        assert_eq!(e.key, "k");
        assert_eq!(e.value, "v");
        assert!(e.tag.is_none());
    }

    #[test]
    fn x_lazy_tagged_entry_with_tag() {
        let e = XLazyTaggedEntry::new("k", "v").with_tag("important");
        assert!(e.matches_tag("important"));
        assert!(!e.matches_tag("other"));
    }

    #[test]
    fn x_lazy_filter_by_tag_basic() {
        let entries = vec![
            XLazyTaggedEntry::new("a", "1").with_tag("x"),
            XLazyTaggedEntry::new("b", "2").with_tag("y"),
            XLazyTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let filtered = x_lazy_filter_by_tag(&entries, "x");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_lazy_group_by_tag_basic() {
        let entries = vec![
            XLazyTaggedEntry::new("a", "1").with_tag("x"),
            XLazyTaggedEntry::new("b", "2"),
            XLazyTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let groups = x_lazy_group_by_tag(&entries);
        assert_eq!(groups["x"].len(), 2);
        assert_eq!(groups["_untagged"].len(), 1);
    }

    #[test]
    fn x_lazy_djb2_hash_deterministic() {
        let h1 = x_lazy_djb2_hash("hello");
        let h2 = x_lazy_djb2_hash("hello");
        assert_eq!(h1, h2);
        assert_ne!(x_lazy_djb2_hash("hello"), x_lazy_djb2_hash("world"));
    }

    #[test]
    fn x_lazy_dedup_entries_basic() {
        let entries = vec![
            XLazyTaggedEntry::new("a", "1"),
            XLazyTaggedEntry::new("a", "2"),
            XLazyTaggedEntry::new("b", "3"),
        ];
        let deduped = x_lazy_dedup_entries(entries);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].value, "1");
    }

    #[test]
    fn x_lazy_validation_result_warning() {
        let w = XLazyValidationResult::Warning("low disk".into());
        assert!(!w.is_ok());
        assert_eq!(w.message(), Some("low disk"));
    }

    #[test]
    fn x_lazy_filter_by_tag_empty() {
        let entries: Vec<XLazyTaggedEntry> = vec![];
        assert!(x_lazy_filter_by_tag(&entries, "x").is_empty());
    }

    #[test]
    fn x_lazy_tagged_entry_no_tag_match() {
        let e = XLazyTaggedEntry::new("k", "v");
        assert!(!e.matches_tag("any"));
    }

}
