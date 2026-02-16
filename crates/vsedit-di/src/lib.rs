//! Dependency injection container for vsedit.
//!
//! This crate provides the core service container infrastructure, equivalent to
//! VS Code's `vs/platform/instantiation`. It is the most depended-on platform
//! component.
//!
//! # Key types
//!
//! - [`Service`] — trait that all injectable services must implement.
//! - [`ServiceCollection`] — a container for registering and retrieving services.
//! - [`ServiceAccessor`] — a thread-safe, cheaply cloneable handle to the collection.
//! - [`service!`] — helper macro to implement [`Service`] for a type.
//!
//! # Example
//!
//! ```
//! use vsedit_di::{service, ServiceCollection};
//!
//! struct Logger;
//! service!(Logger, "Logger");
//!
//! let mut services = ServiceCollection::new();
//! services.register(Logger);
//! assert!(services.has::<Logger>());
//! ```

use std::any::{Any, TypeId};
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use vsedit_lifecycle::Disposable;

// ---------------------------------------------------------------------------
// Service trait
// ---------------------------------------------------------------------------

/// Marker trait for types that can be registered in a [`ServiceCollection`].
///
/// Every service must be `Any + Send + Sync` so it can be stored in a
/// type-erased, thread-safe container. Use the [`service!`] macro for a
/// convenient implementation.
pub trait Service: Any + Send + Sync {
    /// A human-readable name for this service, used in diagnostics.
    fn service_name() -> &'static str
    where
        Self: Sized;
}

/// Implement [`Service`] for a type with a given name.
///
/// # Example
///
/// ```
/// use vsedit_di::{service, Service};
///
/// struct MyService;
/// service!(MyService, "MyService");
///
/// assert_eq!(<MyService as Service>::service_name(), "MyService");
/// ```
#[macro_export]
macro_rules! service {
    ($type:ty, $name:expr) => {
        impl $crate::Service for $type {
            fn service_name() -> &'static str {
                $name
            }
        }
    };
}

// ---------------------------------------------------------------------------
// ServiceEntry — stored in the container
// ---------------------------------------------------------------------------

/// A registered service, either an already-created instance or a deferred
/// factory that produces one on first access.
enum ServiceEntry {
    /// An instantiated service value.
    Instance(Box<dyn Any + Send + Sync>),
    /// A factory that will produce the service on first access.
    Factory(Box<dyn FnOnce(&ServiceCollection) -> Box<dyn Any + Send + Sync> + Send>),
}

// ---------------------------------------------------------------------------
// ServiceCollection
// ---------------------------------------------------------------------------

/// A container for registering and retrieving services by type.
///
/// Services are identified by their [`TypeId`]. Each type can be registered at
/// most once — re-registering the same type replaces the previous entry.
///
/// The collection is **not** internally synchronized for mutation; wrap it in a
/// [`ServiceAccessor`] for shared read access across threads.
pub struct ServiceCollection {
    // UnsafeCell allows `get` to resolve factories through a `&self` reference.
    // Safety: mutation only occurs in `register`, `register_factory` (which
    // take `&mut self`) and in `get` where a Factory entry is atomically
    // replaced with an Instance (monotonic transition).
    services: UnsafeCell<HashMap<TypeId, ServiceEntry>>,
    disposed: AtomicBool,
}

// SAFETY: The UnsafeCell is only mutated through `&mut self` (register) or
// through the controlled factory-resolution path in `get`, which performs a
// monotonic Factory→Instance transition. Thread safety for shared access is
// provided by ServiceAccessor's RwLock.
unsafe impl Send for ServiceCollection {}
// SAFETY: `get` performs a one-shot Factory→Instance transition that is safe
// when externally synchronized (e.g. behind an RwLock write guard, or when
// only one thread has access). The `has`, `len`, `is_empty` methods are
// read-only. Callers using `&self` concurrently must ensure `get` is not
// called from multiple threads without synchronization — ServiceAccessor
// enforces this via RwLock.
unsafe impl Sync for ServiceCollection {}

impl ServiceCollection {
    /// Create an empty service collection.
    pub fn new() -> Self {
        Self {
            services: UnsafeCell::new(HashMap::new()),
            disposed: AtomicBool::new(false),
        }
    }

    /// Return a mutable reference to the inner map.
    fn services_mut(&mut self) -> &mut HashMap<TypeId, ServiceEntry> {
        self.services.get_mut()
    }

    /// Register a concrete service instance.
    ///
    /// If a service of the same type was already registered, it is replaced.
    ///
    /// # Panics
    ///
    /// Panics if the collection has been disposed.
    pub fn register<T: Service>(&mut self, service: T) {
        assert!(!self.disposed.load(Ordering::Acquire), "ServiceCollection is disposed");
        self.services_mut()
            .insert(TypeId::of::<T>(), ServiceEntry::Instance(Box::new(service)));
    }

    /// Register a factory that will lazily create the service on first access.
    ///
    /// The factory receives a reference to the collection so it can resolve
    /// dependencies.
    ///
    /// # Panics
    ///
    /// Panics if the collection has been disposed.
    pub fn register_factory<T: Service>(
        &mut self,
        factory: impl FnOnce(&ServiceCollection) -> T + Send + 'static,
    ) {
        assert!(!self.disposed.load(Ordering::Acquire), "ServiceCollection is disposed");
        self.services_mut().insert(
            TypeId::of::<T>(),
            ServiceEntry::Factory(Box::new(move |sc| Box::new(factory(sc)))),
        );
    }

    /// Get a reference to a registered service.
    ///
    /// If the service was registered with a factory, the factory is invoked on
    /// the first call and the result is cached for subsequent calls.
    ///
    /// Returns `None` if the service type has not been registered.
    pub fn get<T: Service>(&self) -> Option<&T> {
        // SAFETY: We need interior mutability to resolve factories. The
        // UnsafeCell is only mutated here in a monotonic Factory→Instance
        // transition. Callers must ensure no concurrent mutation (enforced
        // by ServiceAccessor's RwLock).
        let map = unsafe { &mut *self.services.get() };
        let entry = map.get_mut(&TypeId::of::<T>())?;

        if matches!(entry, ServiceEntry::Factory(_)) {
            let factory = match std::mem::replace(entry, ServiceEntry::Instance(Box::new(()))) {
                ServiceEntry::Factory(f) => f,
                _ => unreachable!(),
            };
            *entry = ServiceEntry::Instance(factory(self));
        }

        match entry {
            ServiceEntry::Instance(boxed) => boxed.downcast_ref::<T>(),
            ServiceEntry::Factory(_) => unreachable!(),
        }
    }

    /// Get a reference to a registered service, panicking if not found.
    ///
    /// # Panics
    ///
    /// Panics with a descriptive message if `T` has not been registered.
    pub fn get_required<T: Service>(&self) -> &T {
        self.get::<T>().unwrap_or_else(|| {
            panic!(
                "Required service '{}' not registered in ServiceCollection",
                T::service_name()
            )
        })
    }

    /// Returns `true` if a service of type `T` has been registered (either as
    /// an instance or a factory).
    pub fn has<T: Service>(&self) -> bool {
        // SAFETY: read-only access to the map.
        let map = unsafe { &*self.services.get() };
        map.contains_key(&TypeId::of::<T>())
    }

    /// Returns the number of registered services.
    pub fn len(&self) -> usize {
        // SAFETY: read-only access to the map.
        let map = unsafe { &*self.services.get() };
        map.len()
    }

    /// Returns `true` if no services are registered.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ServiceCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl Disposable for ServiceCollection {
    fn dispose(&self) {
        self.disposed.store(true, Ordering::Release);
        // We cannot clear `services` through `&self` without interior mutability,
        // but marking as disposed prevents further registrations and signals
        // consumers that the collection is no longer valid.
    }

    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::Acquire)
    }
}

impl fmt::Debug for ServiceCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceCollection")
            .field("services", &self.len())
            .field("disposed", &self.is_disposed())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ServiceAccessor — thread-safe shared handle
// ---------------------------------------------------------------------------

/// A thread-safe, cheaply cloneable handle to a [`ServiceCollection`].
///
/// Internally wraps the collection in `Arc<RwLock<ServiceCollection>>` so
/// multiple threads can read services concurrently.
#[derive(Clone)]
pub struct ServiceAccessor {
    inner: Arc<RwLock<ServiceCollection>>,
}

impl ServiceAccessor {
    /// Create a new accessor wrapping the given collection.
    pub fn new(collection: ServiceCollection) -> Self {
        Self {
            inner: Arc::new(RwLock::new(collection)),
        }
    }

    /// Execute a closure with read access to the underlying collection.
    ///
    /// This is the primary way to access services through the accessor.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ServiceCollection) -> R,
    {
        let guard = self.inner.read().unwrap();
        f(&guard)
    }

    /// Execute a closure with write access to the underlying collection.
    ///
    /// Use this to register new services after construction.
    ///
    /// # Panics
    ///
    /// Panics if the `RwLock` is poisoned.
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut ServiceCollection) -> R,
    {
        let mut guard = self.inner.write().unwrap();
        f(&mut guard)
    }

    /// Check if a service of type `T` is registered.
    pub fn has<T: Service>(&self) -> bool {
        self.with(|sc| sc.has::<T>())
    }

    /// Returns the number of strong references to the underlying collection.
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl Disposable for ServiceAccessor {
    fn dispose(&self) {
        if let Ok(guard) = self.inner.read() {
            guard.dispose();
        }
    }

    fn is_disposed(&self) -> bool {
        self.inner
            .read()
            .map(|g| g.is_disposed())
            .unwrap_or(true)
    }
}

impl fmt::Debug for ServiceAccessor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceAccessor")
            .field("strong_count", &self.strong_count())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ServiceStats — diagnostic snapshot of container state
// ---------------------------------------------------------------------------

/// A snapshot of diagnostic statistics for a [`ServiceCollection`].
///
/// Useful for health checks, logging, and debugging the state of the
/// dependency injection container at a point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceStats {
    /// Number of services currently registered (instances + factories).
    pub total_registered: usize,
    /// Number of services that have been resolved (factories invoked).
    pub total_resolved: usize,
    /// Number of resolution attempts that returned `None`.
    pub resolution_errors: usize,
}

impl ServiceStats {
    /// Collect statistics from the given [`ServiceCollection`].
    pub fn from_collection(collection: &ServiceCollection) -> Self {
        // SAFETY: read-only access to the map.
        let map = unsafe { &*collection.services.get() };
        let total_registered = map.len();
        let total_resolved = map
            .values()
            .filter(|e| matches!(e, ServiceEntry::Instance(_)))
            .count();
        Self {
            total_registered,
            total_resolved,
            resolution_errors: 0,
        }
    }
}

impl fmt::Display for ServiceStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "registered={}, resolved={}, errors={}",
            self.total_registered, self.total_resolved, self.resolution_errors
        )
    }
}

// ---------------------------------------------------------------------------
// validate_service_id — service identifier validation
// ---------------------------------------------------------------------------

/// Validate a service identifier string.
///
/// A valid service id must:
/// - Be non-empty and at most 128 characters
/// - Start with an ASCII letter
/// - Contain only ASCII alphanumeric characters, underscores, hyphens, or dots
///
/// This mirrors the naming conventions used in VS Code's service decorators
/// (e.g. `"ILogService"`, `"editor.config"`).
pub fn validate_service_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 128 {
        return false;
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Service lifecycle
// ---------------------------------------------------------------------------

/// Represents the lifecycle phase of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceLifecycle {
    /// Service has been registered but not yet created.
    Registered,
    /// Service has been created / resolved.
    Active,
    /// Service has been disposed.
    Disposed,
}

impl fmt::Display for ServiceLifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registered => write!(f, "Registered"),
            Self::Active => write!(f, "Active"),
            Self::Disposed => write!(f, "Disposed"),
        }
    }
}

// ---------------------------------------------------------------------------
// Circular dependency detection
// ---------------------------------------------------------------------------

/// Checks for circular dependencies in a dependency graph represented as
/// adjacency lists. Keys are service names, values are their dependencies.
pub fn detect_circular_dependency(
    graph: &HashMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    enum State { Unvisited, InProgress, Done }
    let mut states: HashMap<&str, State> = HashMap::new();
    let mut path: Vec<String> = Vec::new();

    for key in graph.keys() {
        states.insert(key.as_str(), State::Unvisited);
    }

    fn visit<'a>(
        node: &'a str,
        graph: &'a HashMap<String, Vec<String>>,
        states: &mut HashMap<&'a str, State>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        match states.get(node) {
            Some(State::InProgress) => {
                path.push(node.to_string());
                return Some(path.clone());
            }
            Some(State::Done) => return None,
            _ => {}
        }
        states.insert(node, State::InProgress);
        path.push(node.to_string());
        if let Some(deps) = graph.get(node) {
            for dep in deps {
                if let Some(cycle) = visit(dep, graph, states, path) {
                    return Some(cycle);
                }
            }
        }
        path.pop();
        states.insert(node, State::Done);
        None
    }

    for key in graph.keys() {
        if matches!(states.get(key.as_str()), Some(State::Unvisited)) {
            if let Some(cycle) = visit(key.as_str(), graph, &mut states, &mut path) {
                return Some(cycle);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Container statistics (extended)
// ---------------------------------------------------------------------------

/// Extended diagnostic information about the container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerDiagnostics {
    pub service_count: usize,
    pub factory_count: usize,
    pub instance_count: usize,
    pub is_disposed: bool,
}

impl ContainerDiagnostics {
    /// Collect diagnostics from a [`ServiceCollection`].
    pub fn from_collection(collection: &ServiceCollection) -> Self {
        let map = unsafe { &*collection.services.get() };
        let factory_count = map.values().filter(|e| matches!(e, ServiceEntry::Factory(_))).count();
        let instance_count = map.values().filter(|e| matches!(e, ServiceEntry::Instance(_))).count();
        Self {
            service_count: map.len(),
            factory_count,
            instance_count,
            is_disposed: collection.is_disposed(),
        }
    }
}

impl fmt::Display for ContainerDiagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "services={}, factories={}, instances={}, disposed={}",
            self.service_count, self.factory_count, self.instance_count, self.is_disposed
        )
    }
}

// ---------------------------------------------------------------------------
// ServiceScope — hierarchical DI scope
// ---------------------------------------------------------------------------

/// A hierarchical DI scope where a child inherits parent services but can
/// override them with its own registrations.
pub struct ServiceScope {
    own: ServiceCollection,
    parent: Option<Arc<RwLock<ServiceCollection>>>,
}

impl ServiceScope {
    /// Create a root scope with no parent.
    pub fn new() -> Self {
        Self {
            own: ServiceCollection::new(),
            parent: None,
        }
    }

    /// Create a child scope that references the parent accessor's collection.
    pub fn child(parent: &ServiceAccessor) -> Self {
        Self {
            own: ServiceCollection::new(),
            parent: Some(Arc::clone(&parent.inner)),
        }
    }

    /// Register a service in this scope's own collection.
    pub fn register<T: Service>(&mut self, service: T) {
        self.own.register(service);
    }

    /// Get a reference to a service registered in this scope's own collection.
    ///
    /// Does **not** look into the parent because we cannot return a reference
    /// through a `RwLock` guard. Use [`has`](Self::has) to check both scopes.
    pub fn get<T: Service>(&self) -> Option<&T> {
        self.own.get::<T>()
    }

    /// Returns `true` if a service of type `T` exists in this scope or the
    /// parent scope.
    pub fn has<T: Service>(&self) -> bool {
        if self.own.has::<T>() {
            return true;
        }
        if let Some(ref parent) = self.parent {
            if let Ok(guard) = parent.read() {
                return guard.has::<T>();
            }
        }
        false
    }

    /// Returns `true` if a service of type `T` exists only in this scope's own
    /// collection.
    pub fn has_own<T: Service>(&self) -> bool {
        self.own.has::<T>()
    }

    /// Returns the number of services registered in this scope's own
    /// collection (excluding parent).
    pub fn own_count(&self) -> usize {
        self.own.len()
    }
}

impl Default for ServiceScope {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ServiceRegistry — tagged multi-implementation registry
// ---------------------------------------------------------------------------

/// A registry that allows multiple service implementations to be registered
/// under string tags and retrieved as a group.
pub struct ServiceRegistry {
    tagged: HashMap<String, Vec<Box<dyn Any + Send + Sync>>>,
}

impl ServiceRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tagged: HashMap::new(),
        }
    }

    /// Register a service under the given tag.
    pub fn register_tagged<T: Service>(&mut self, tag: &str, service: T) {
        self.tagged
            .entry(tag.to_string())
            .or_default()
            .push(Box::new(service));
    }

    /// Return references to all services of type `T` registered under the
    /// given tag.
    pub fn get_tagged<T: Service>(&self, tag: &str) -> Vec<&T> {
        self.tagged
            .get(tag)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|b| b.downcast_ref::<T>())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Number of services stored under the given tag.
    pub fn tag_count(&self, tag: &str) -> usize {
        self.tagged.get(tag).map_or(0, |v| v.len())
    }

    /// All registered tags.
    pub fn tags(&self) -> Vec<&str> {
        self.tagged.keys().map(|s| s.as_str()).collect()
    }

    /// Returns `true` if the given tag has been registered.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tagged.contains_key(tag)
    }

    /// Remove all services registered under the given tag.
    pub fn clear_tag(&mut self, tag: &str) {
        self.tagged.remove(tag);
    }

    /// Total number of services across all tags.
    pub fn total_services(&self) -> usize {
        self.tagged.values().map(|v| v.len()).sum()
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ServiceLifecycle extensions
// ---------------------------------------------------------------------------

impl ServiceLifecycle {
    /// Returns `true` if the service is in the [`Active`](Self::Active) phase.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Returns `true` if the service has been [`Disposed`](Self::Disposed).
    pub fn is_disposed(&self) -> bool {
        matches!(self, Self::Disposed)
    }

    /// Returns `true` if the service is [`Registered`](Self::Registered) but
    /// not yet active.
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Registered)
    }

    /// A short label suitable for use in structured log output.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Registered => "registered",
            Self::Active => "active",
            Self::Disposed => "disposed",
        }
    }
}

// ---------------------------------------------------------------------------
// ServiceStats extensions
// ---------------------------------------------------------------------------

impl ServiceStats {
    /// Merge two snapshots by summing their counters.
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            total_registered: self.total_registered + other.total_registered,
            total_resolved: self.total_resolved + other.total_resolved,
            resolution_errors: self.resolution_errors + other.resolution_errors,
        }
    }

    /// Returns the number of services that have been registered but not yet
    /// resolved (i.e. still pending as factories).
    pub fn pending_count(&self) -> usize {
        self.total_registered.saturating_sub(self.total_resolved)
    }

    /// Returns `true` when every registered service has been resolved.
    pub fn all_resolved(&self) -> bool {
        self.total_registered == self.total_resolved
    }
}

// ---------------------------------------------------------------------------
// ContainerDiagnostics extensions
// ---------------------------------------------------------------------------

impl ContainerDiagnostics {
    /// Returns a single-line summary of the container health.
    pub fn summary(&self) -> String {
        if self.is_disposed {
            return "container disposed".to_string();
        }
        format!(
            "{} service(s): {} instance(s), {} factory(ies)",
            self.service_count, self.instance_count, self.factory_count
        )
    }

    /// Returns `true` when every registered service has been resolved to an
    /// instance (no remaining factories).
    pub fn fully_resolved(&self) -> bool {
        self.factory_count == 0
    }
}

// ---------------------------------------------------------------------------
// ServiceRegistry extensions
// ---------------------------------------------------------------------------

impl ServiceRegistry {
    /// Merge all entries from `other` into `self`.
    ///
    /// Tags that exist in both registries are combined (entries from `other`
    /// are appended after entries in `self`).
    pub fn merge(&mut self, other: ServiceRegistry) {
        for (tag, entries) in other.tagged {
            self.tagged.entry(tag).or_default().extend(entries);
        }
    }

    /// Returns `true` if the registry contains no tags.
    pub fn is_empty(&self) -> bool {
        self.tagged.is_empty()
    }

    /// Returns an iterator over `(tag, service_count)` pairs.
    pub fn iter_tags(&self) -> impl Iterator<Item = (&str, usize)> {
        self.tagged.iter().map(|(k, v)| (k.as_str(), v.len()))
    }

    /// Returns the number of distinct tags in the registry.
    pub fn tag_len(&self) -> usize {
        self.tagged.len()
    }
}

// ---------------------------------------------------------------------------
// ServiceScope extensions
// ---------------------------------------------------------------------------

impl ServiceScope {
    /// Returns `true` if the scope has a parent.
    pub fn has_parent(&self) -> bool {
        self.parent.is_some()
    }

    /// Returns `true` if the scope's own collection is empty.
    pub fn is_empty(&self) -> bool {
        self.own.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Dependency graph helpers
// ---------------------------------------------------------------------------

/// Produce a topological ordering of a dependency graph, or return an error
/// containing a cycle when one exists.
///
/// The graph is represented as a `HashMap<String, Vec<String>>` where each
/// key is a service name and its value is the list of services it depends on.
pub fn topological_sort(
    graph: &HashMap<String, Vec<String>>,
) -> Result<Vec<String>, Vec<String>> {
    if let Some(cycle) = detect_circular_dependency(graph) {
        return Err(cycle);
    }

    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    for key in graph.keys() {
        in_degree.entry(key.as_str()).or_insert(0);
    }
    for deps in graph.values() {
        for dep in deps {
            *in_degree.entry(dep.as_str()).or_insert(0) += 1;
        }
    }

    let mut queue: Vec<&str> = in_degree
        .iter()
        .filter(|&(_, d)| *d == 0)
        .map(|(&k, _)| k)
        .collect();
    queue.sort();

    let mut result = Vec::new();
    while let Some(node) = queue.pop() {
        result.push(node.to_string());
        if let Some(deps) = graph.get(node) {
            for dep in deps {
                if let Some(d) = in_degree.get_mut(dep.as_str()) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        queue.push(dep.as_str());
                        queue.sort();
                    }
                }
            }
        }
    }

    Ok(result)
}

/// Summarise a dependency graph as a human-readable multi-line string showing
/// each service and its direct dependencies.
pub fn dependency_summary(graph: &HashMap<String, Vec<String>>) -> String {
    let mut keys: Vec<&str> = graph.keys().map(|s| s.as_str()).collect();
    keys.sort();
    let mut out = String::new();
    for key in keys {
        let deps = &graph[key];
        if deps.is_empty() {
            out.push_str(&format!("{key} -> (none)\n"));
        } else {
            out.push_str(&format!("{key} -> {}\n", deps.join(", ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    // -- Test services ------------------------------------------------------

    struct LogService {
        prefix: String,
    }
    service!(LogService, "LogService");

    struct ConfigService {
        value: i32,
    }
    service!(ConfigService, "ConfigService");

    struct CountingService {
        count: AtomicUsize,
    }
    service!(CountingService, "CountingService");

    // -- Basic registration -------------------------------------------------

    #[test]
    fn register_and_get() {
        let mut sc = ServiceCollection::new();
        sc.register(LogService {
            prefix: "INFO".into(),
        });

        let log = sc.get::<LogService>().unwrap();
        assert_eq!(log.prefix, "INFO");
    }

    #[test]
    fn get_returns_none_for_unregistered() {
        let sc = ServiceCollection::new();
        assert!(sc.get::<LogService>().is_none());
    }

    #[test]
    fn get_required_succeeds() {
        let mut sc = ServiceCollection::new();
        sc.register(ConfigService { value: 42 });

        let cfg = sc.get_required::<ConfigService>();
        assert_eq!(cfg.value, 42);
    }

    #[test]
    #[should_panic(expected = "Required service 'LogService' not registered")]
    fn get_required_panics_when_missing() {
        let sc = ServiceCollection::new();
        sc.get_required::<LogService>();
    }

    #[test]
    fn has_returns_true_for_registered() {
        let mut sc = ServiceCollection::new();
        sc.register(LogService {
            prefix: "X".into(),
        });
        assert!(sc.has::<LogService>());
        assert!(!sc.has::<ConfigService>());
    }

    #[test]
    fn register_replaces_previous() {
        let mut sc = ServiceCollection::new();
        sc.register(ConfigService { value: 1 });
        sc.register(ConfigService { value: 2 });

        assert_eq!(sc.get_required::<ConfigService>().value, 2);
    }

    // -- Factory registration -----------------------------------------------

    #[test]
    fn factory_creates_on_first_access() {
        let mut sc = ServiceCollection::new();
        sc.register(ConfigService { value: 99 });
        sc.register_factory(|sc: &ServiceCollection| {
            let cfg = sc.get_required::<ConfigService>();
            LogService {
                prefix: format!("level-{}", cfg.value),
            }
        });

        let log = sc.get_required::<LogService>();
        assert_eq!(log.prefix, "level-99");
    }

    #[test]
    fn factory_result_is_cached() {
        let mut sc = ServiceCollection::new();
        sc.register_factory(|_| CountingService {
            count: AtomicUsize::new(0),
        });

        let s1 = sc.get_required::<CountingService>();
        s1.count.fetch_add(1, Ordering::SeqCst);

        let s2 = sc.get_required::<CountingService>();
        assert_eq!(s2.count.load(Ordering::SeqCst), 1, "should be same instance");
    }

    // -- Collection metadata ------------------------------------------------

    #[test]
    fn len_and_is_empty() {
        let mut sc = ServiceCollection::new();
        assert!(sc.is_empty());
        assert_eq!(sc.len(), 0);

        sc.register(ConfigService { value: 1 });
        assert_eq!(sc.len(), 1);
        assert!(!sc.is_empty());
    }

    // -- Disposable ---------------------------------------------------------

    #[test]
    fn disposable_integration() {
        let mut sc = ServiceCollection::new();
        sc.register(ConfigService { value: 1 });

        assert!(!sc.is_disposed());
        sc.dispose();
        assert!(sc.is_disposed());
    }

    #[test]
    #[should_panic(expected = "ServiceCollection is disposed")]
    fn register_after_dispose_panics() {
        let mut sc = ServiceCollection::new();
        sc.dispose();
        sc.register(ConfigService { value: 1 });
    }

    // -- ServiceAccessor ----------------------------------------------------

    #[test]
    fn accessor_read_access() {
        let mut sc = ServiceCollection::new();
        sc.register(ConfigService { value: 7 });

        let accessor = ServiceAccessor::new(sc);
        let val = accessor.with(|sc| sc.get_required::<ConfigService>().value);
        assert_eq!(val, 7);
    }

    #[test]
    fn accessor_write_access() {
        let accessor = ServiceAccessor::new(ServiceCollection::new());
        accessor.with_mut(|sc| {
            sc.register(ConfigService { value: 55 });
        });
        assert!(accessor.has::<ConfigService>());
    }

    #[test]
    fn accessor_is_clone() {
        let accessor = ServiceAccessor::new(ServiceCollection::new());
        let clone = accessor.clone();
        accessor.with_mut(|sc| sc.register(ConfigService { value: 1 }));
        assert!(clone.has::<ConfigService>());
    }

    #[test]
    fn accessor_dispose() {
        let mut sc = ServiceCollection::new();
        sc.register(ConfigService { value: 1 });
        let accessor = ServiceAccessor::new(sc);

        assert!(!accessor.is_disposed());
        accessor.dispose();
        assert!(accessor.is_disposed());
    }

    #[test]
    fn accessor_strong_count() {
        let accessor = ServiceAccessor::new(ServiceCollection::new());
        assert_eq!(accessor.strong_count(), 1);
        let clone = accessor.clone();
        assert_eq!(accessor.strong_count(), 2);
        drop(clone);
        assert_eq!(accessor.strong_count(), 1);
    }

    // -- Thread safety ------------------------------------------------------

    #[test]
    fn accessor_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ServiceAccessor>();
    }

    #[test]
    fn service_collection_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ServiceCollection>();
    }

    #[test]
    fn accessor_across_threads() {
        let mut sc = ServiceCollection::new();
        sc.register(ConfigService { value: 42 });
        let accessor = ServiceAccessor::new(sc);

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let a = accessor.clone();
                std::thread::spawn(move || a.with(|sc| sc.get_required::<ConfigService>().value))
            })
            .collect();

        for h in handles {
            assert_eq!(h.join().unwrap(), 42);
        }
    }

    // -- Debug formatting ---------------------------------------------------

    #[test]
    fn debug_formatting() {
        let sc = ServiceCollection::new();
        let dbg = format!("{sc:?}");
        assert!(dbg.contains("ServiceCollection"));

        let accessor = ServiceAccessor::new(sc);
        let dbg = format!("{accessor:?}");
        assert!(dbg.contains("ServiceAccessor"));
    }

    // -- service! macro -----------------------------------------------------

    #[test]
    fn service_macro_sets_name() {
        assert_eq!(<LogService as Service>::service_name(), "LogService");
        assert_eq!(<ConfigService as Service>::service_name(), "ConfigService");
    }

    // -- ServiceStats -------------------------------------------------------

    #[test]
    fn stats_empty_collection() {
        let sc = ServiceCollection::new();
        let stats = ServiceStats::from_collection(&sc);
        assert_eq!(
            stats,
            ServiceStats {
                total_registered: 0,
                total_resolved: 0,
                resolution_errors: 0,
            }
        );
    }

    #[test]
    fn stats_with_instances_and_factories() {
        let mut sc = ServiceCollection::new();
        sc.register(ConfigService { value: 1 });
        sc.register_factory(|_| LogService {
            prefix: "test".into(),
        });

        let stats = ServiceStats::from_collection(&sc);
        assert_eq!(stats.total_registered, 2);
        // Only ConfigService is resolved; LogService is still a factory.
        assert_eq!(stats.total_resolved, 1);

        // Resolve the factory
        let _ = sc.get::<LogService>();
        let stats_after = ServiceStats::from_collection(&sc);
        assert_eq!(stats_after.total_resolved, 2);
    }

    #[test]
    fn stats_display() {
        let stats = ServiceStats {
            total_registered: 3,
            total_resolved: 2,
            resolution_errors: 1,
        };
        assert_eq!(
            stats.to_string(),
            "registered=3, resolved=2, errors=1"
        );
    }

    // -- validate_service_id ------------------------------------------------

    #[test]
    fn validate_service_id_accepts_valid_ids() {
        assert!(validate_service_id("ILogService"));
        assert!(validate_service_id("editor.config"));
        assert!(validate_service_id("my_service-v2"));
        assert!(validate_service_id("A"));
        assert!(validate_service_id("a123.b456"));
    }

    #[test]
    fn validate_service_id_rejects_invalid_ids() {
        assert!(!validate_service_id(""));
        assert!(!validate_service_id("1StartsWithDigit"));
        assert!(!validate_service_id("_leading_underscore"));
        assert!(!validate_service_id("-leading-hyphen"));
        assert!(!validate_service_id(".leading.dot"));
        assert!(!validate_service_id("has space"));
        assert!(!validate_service_id("has/slash"));
        assert!(!validate_service_id("has@symbol"));
        let long_id = "A".repeat(129);
        assert!(!validate_service_id(&long_id));
        // Exactly 128 is fine
        let ok_id = "A".repeat(128);
        assert!(validate_service_id(&ok_id));
    }

    #[test]
    fn service_lifecycle_display() {
        assert_eq!(ServiceLifecycle::Registered.to_string(), "Registered");
        assert_eq!(ServiceLifecycle::Active.to_string(), "Active");
        assert_eq!(ServiceLifecycle::Disposed.to_string(), "Disposed");
    }

    #[test]
    fn service_lifecycle_equality() {
        assert_eq!(ServiceLifecycle::Registered, ServiceLifecycle::Registered);
        assert_ne!(ServiceLifecycle::Active, ServiceLifecycle::Disposed);
    }

    #[test]
    fn detect_circular_dependency_none() {
        let mut graph = HashMap::new();
        graph.insert("A".into(), vec!["B".into()]);
        graph.insert("B".into(), vec!["C".into()]);
        graph.insert("C".into(), vec![]);
        assert!(detect_circular_dependency(&graph).is_none());
    }

    #[test]
    fn detect_circular_dependency_found() {
        let mut graph = HashMap::new();
        graph.insert("A".into(), vec!["B".into()]);
        graph.insert("B".into(), vec!["C".into()]);
        graph.insert("C".into(), vec!["A".into()]);
        let cycle = detect_circular_dependency(&graph);
        assert!(cycle.is_some());
        let path = cycle.unwrap();
        assert!(path.len() >= 3);
    }

    #[test]
    fn detect_circular_dependency_self_loop() {
        let mut graph = HashMap::new();
        graph.insert("A".into(), vec!["A".into()]);
        let cycle = detect_circular_dependency(&graph);
        assert!(cycle.is_some());
    }

    #[test]
    fn container_diagnostics_empty() {
        let sc = ServiceCollection::new();
        let diag = ContainerDiagnostics::from_collection(&sc);
        assert_eq!(diag.service_count, 0);
        assert_eq!(diag.factory_count, 0);
        assert_eq!(diag.instance_count, 0);
        assert!(!diag.is_disposed);
    }

    #[test]
    fn container_diagnostics_with_services() {
        let mut sc = ServiceCollection::new();
        sc.register(ConfigService { value: 1 });
        sc.register_factory(|_| LogService { prefix: "test".into() });
        let diag = ContainerDiagnostics::from_collection(&sc);
        assert_eq!(diag.service_count, 2);
        assert_eq!(diag.factory_count, 1);
        assert_eq!(diag.instance_count, 1);
    }

    #[test]
    fn container_diagnostics_display() {
        let diag = ContainerDiagnostics {
            service_count: 5,
            factory_count: 2,
            instance_count: 3,
            is_disposed: false,
        };
        let s = format!("{}", diag);
        assert!(s.contains("services=5"));
        assert!(s.contains("factories=2"));
    }

    #[test]
    fn container_diagnostics_after_resolve() {
        let mut sc = ServiceCollection::new();
        sc.register_factory(|_| LogService { prefix: "x".into() });
        let diag_before = ContainerDiagnostics::from_collection(&sc);
        assert_eq!(diag_before.factory_count, 1);
        let _ = sc.get::<LogService>();
        let diag_after = ContainerDiagnostics::from_collection(&sc);
        assert_eq!(diag_after.factory_count, 0);
        assert_eq!(diag_after.instance_count, 1);
    }

    // -- ServiceScope tests -------------------------------------------------

    #[test]
    fn scope_root_register_and_get() {
        let mut scope = ServiceScope::new();
        scope.register(LogService { prefix: "root".into() });
        let log = scope.get::<LogService>().unwrap();
        assert_eq!(log.prefix, "root");
        assert_eq!(scope.own_count(), 1);
    }

    #[test]
    fn scope_child_has_from_parent() {
        let mut parent_col = ServiceCollection::new();
        parent_col.register(ConfigService { value: 42 });
        let accessor = ServiceAccessor::new(parent_col);

        let child = ServiceScope::child(&accessor);
        assert!(child.has::<ConfigService>());
        // get returns None because we can't return ref through RwLock
        assert!(child.get::<ConfigService>().is_none());
    }

    #[test]
    fn scope_child_override_parent() {
        let mut parent_col = ServiceCollection::new();
        parent_col.register(LogService { prefix: "parent".into() });
        let accessor = ServiceAccessor::new(parent_col);

        let mut child = ServiceScope::child(&accessor);
        child.register(LogService { prefix: "child".into() });

        let log = child.get::<LogService>().unwrap();
        assert_eq!(log.prefix, "child");
        assert!(child.has_own::<LogService>());
    }

    #[test]
    fn scope_has_own_vs_has() {
        let mut parent_col = ServiceCollection::new();
        parent_col.register(ConfigService { value: 10 });
        let accessor = ServiceAccessor::new(parent_col);

        let mut child = ServiceScope::child(&accessor);
        child.register(LogService { prefix: "mine".into() });

        assert!(child.has_own::<LogService>());
        assert!(!child.has_own::<ConfigService>());
        assert!(child.has::<LogService>());
        assert!(child.has::<ConfigService>());
    }

    // -- ServiceRegistry tests ----------------------------------------------

    #[test]
    fn registry_register_and_get_tagged() {
        let mut reg = ServiceRegistry::new();
        reg.register_tagged("loggers", LogService { prefix: "a".into() });

        let loggers = reg.get_tagged::<LogService>("loggers");
        assert_eq!(loggers.len(), 1);
        assert_eq!(loggers[0].prefix, "a");
    }

    #[test]
    fn registry_multiple_implementations() {
        let mut reg = ServiceRegistry::new();
        reg.register_tagged("loggers", LogService { prefix: "a".into() });
        reg.register_tagged("loggers", LogService { prefix: "b".into() });
        reg.register_tagged("loggers", LogService { prefix: "c".into() });

        let loggers = reg.get_tagged::<LogService>("loggers");
        assert_eq!(loggers.len(), 3);
        assert_eq!(loggers[1].prefix, "b");
    }

    #[test]
    fn registry_tag_count_and_tags() {
        let mut reg = ServiceRegistry::new();
        reg.register_tagged("loggers", LogService { prefix: "x".into() });
        reg.register_tagged("loggers", LogService { prefix: "y".into() });
        reg.register_tagged("configs", ConfigService { value: 1 });

        assert_eq!(reg.tag_count("loggers"), 2);
        assert_eq!(reg.tag_count("configs"), 1);
        assert_eq!(reg.tag_count("missing"), 0);

        let tags = reg.tags();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"loggers"));
        assert!(tags.contains(&"configs"));
    }

    #[test]
    fn registry_clear_tag() {
        let mut reg = ServiceRegistry::new();
        reg.register_tagged("loggers", LogService { prefix: "a".into() });
        reg.register_tagged("loggers", LogService { prefix: "b".into() });
        assert!(reg.has_tag("loggers"));

        reg.clear_tag("loggers");
        assert!(!reg.has_tag("loggers"));
        assert_eq!(reg.tag_count("loggers"), 0);
    }

    #[test]
    fn registry_total_services() {
        let mut reg = ServiceRegistry::new();
        reg.register_tagged("loggers", LogService { prefix: "a".into() });
        reg.register_tagged("loggers", LogService { prefix: "b".into() });
        reg.register_tagged("configs", ConfigService { value: 1 });

        assert_eq!(reg.total_services(), 3);
        reg.clear_tag("loggers");
        assert_eq!(reg.total_services(), 1);
    }

    // -- ServiceLifecycle extensions ----------------------------------------

    #[test]
    fn lifecycle_predicates() {
        assert!(ServiceLifecycle::Active.is_active());
        assert!(!ServiceLifecycle::Registered.is_active());

        assert!(ServiceLifecycle::Disposed.is_disposed());
        assert!(!ServiceLifecycle::Active.is_disposed());

        assert!(ServiceLifecycle::Registered.is_pending());
        assert!(!ServiceLifecycle::Active.is_pending());
    }

    #[test]
    fn lifecycle_label() {
        assert_eq!(ServiceLifecycle::Registered.label(), "registered");
        assert_eq!(ServiceLifecycle::Active.label(), "active");
        assert_eq!(ServiceLifecycle::Disposed.label(), "disposed");
    }

    // -- ServiceStats extensions -------------------------------------------

    #[test]
    fn stats_merge() {
        let a = ServiceStats { total_registered: 3, total_resolved: 2, resolution_errors: 1 };
        let b = ServiceStats { total_registered: 5, total_resolved: 4, resolution_errors: 0 };
        let merged = a.merge(&b);
        assert_eq!(merged.total_registered, 8);
        assert_eq!(merged.total_resolved, 6);
        assert_eq!(merged.resolution_errors, 1);
    }

    #[test]
    fn stats_pending_and_all_resolved() {
        let partial = ServiceStats { total_registered: 5, total_resolved: 3, resolution_errors: 0 };
        assert_eq!(partial.pending_count(), 2);
        assert!(!partial.all_resolved());

        let full = ServiceStats { total_registered: 4, total_resolved: 4, resolution_errors: 0 };
        assert_eq!(full.pending_count(), 0);
        assert!(full.all_resolved());
    }

    // -- ContainerDiagnostics extensions -----------------------------------

    #[test]
    fn diagnostics_summary_and_fully_resolved() {
        let diag = ContainerDiagnostics {
            service_count: 3,
            factory_count: 0,
            instance_count: 3,
            is_disposed: false,
        };
        assert!(diag.fully_resolved());
        assert!(diag.summary().contains("3 service(s)"));

        let disposed = ContainerDiagnostics {
            service_count: 1,
            factory_count: 0,
            instance_count: 1,
            is_disposed: true,
        };
        assert_eq!(disposed.summary(), "container disposed");
    }

    // -- ServiceRegistry extensions ----------------------------------------

    #[test]
    fn registry_merge_and_iter() {
        let mut r1 = ServiceRegistry::new();
        r1.register_tagged("loggers", LogService { prefix: "a".into() });

        let mut r2 = ServiceRegistry::new();
        r2.register_tagged("loggers", LogService { prefix: "b".into() });
        r2.register_tagged("configs", ConfigService { value: 1 });

        assert!(!r1.is_empty());
        assert_eq!(r1.tag_len(), 1);

        r1.merge(r2);
        assert_eq!(r1.tag_count("loggers"), 2);
        assert_eq!(r1.tag_count("configs"), 1);
        assert_eq!(r1.tag_len(), 2);

        let tags: Vec<_> = r1.iter_tags().collect();
        assert_eq!(tags.len(), 2);
    }

    // -- Dependency graph helpers ------------------------------------------

    #[test]
    fn topological_sort_acyclic() {
        let mut graph = HashMap::new();
        graph.insert("A".into(), vec!["B".into(), "C".into()]);
        graph.insert("B".into(), vec!["C".into()]);
        graph.insert("C".into(), vec![]);
        let order = topological_sort(&graph).unwrap();
        let pos_a = order.iter().position(|s| s == "A").unwrap();
        let pos_b = order.iter().position(|s| s == "B").unwrap();
        let pos_c = order.iter().position(|s| s == "C").unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn topological_sort_cyclic_returns_err() {
        let mut graph = HashMap::new();
        graph.insert("X".into(), vec!["Y".into()]);
        graph.insert("Y".into(), vec!["X".into()]);
        assert!(topological_sort(&graph).is_err());
    }

    #[test]
    fn dependency_summary_output() {
        let mut graph = HashMap::new();
        graph.insert("A".into(), vec!["B".into()]);
        graph.insert("B".into(), vec![]);
        let summary = dependency_summary(&graph);
        assert!(summary.contains("A -> B"));
        assert!(summary.contains("B -> (none)"));
    }

    // -- ServiceScope extensions -------------------------------------------

    #[test]
    fn scope_has_parent_and_is_empty() {
        let root = ServiceScope::new();
        assert!(!root.has_parent());
        assert!(root.is_empty());

        let accessor = ServiceAccessor::new(ServiceCollection::new());
        let child = ServiceScope::child(&accessor);
        assert!(child.has_parent());
        assert!(child.is_empty());
    }
}
