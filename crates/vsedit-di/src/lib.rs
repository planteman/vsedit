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
// ServiceDescriptor — singleton vs transient metadata
// ---------------------------------------------------------------------------

/// Describes how a service should be instantiated and managed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceKind {
    /// A single shared instance is created on first resolve and reused.
    Singleton,
    /// A new instance is created on every resolve.
    Transient,
}

impl fmt::Display for ServiceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Singleton => write!(f, "Singleton"),
            Self::Transient => write!(f, "Transient"),
        }
    }
}

/// Metadata describing a registered service: its name, kind, lifecycle, and
/// declared dependencies.
#[derive(Debug, Clone)]
pub struct ServiceDescriptor {
    /// Human-readable service name.
    pub name: String,
    /// Whether the service is singleton or transient.
    pub kind: ServiceKind,
    /// Current lifecycle phase.
    pub lifecycle: ServiceLifecycle,
    /// Names of services this service depends on.
    pub dependencies: Vec<String>,
}

impl ServiceDescriptor {
    /// Create a new descriptor.
    pub fn new(name: impl Into<String>, kind: ServiceKind) -> Self {
        Self {
            name: name.into(),
            kind,
            lifecycle: ServiceLifecycle::Registered,
            dependencies: Vec::new(),
        }
    }

    /// Builder: add a dependency.
    pub fn depends_on(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    /// Transition to the [`Active`](ServiceLifecycle::Active) state.
    pub fn activate(&mut self) {
        self.lifecycle = ServiceLifecycle::Active;
    }

    /// Transition to the [`Disposed`](ServiceLifecycle::Disposed) state.
    pub fn mark_disposed(&mut self) {
        self.lifecycle = ServiceLifecycle::Disposed;
    }

    /// Returns `true` when this is a singleton service.
    pub fn is_singleton(&self) -> bool {
        self.kind == ServiceKind::Singleton
    }

    /// Returns `true` when this is a transient service.
    pub fn is_transient(&self) -> bool {
        self.kind == ServiceKind::Transient
    }
}

impl fmt::Display for ServiceDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}] ({})", self.name, self.kind, self.lifecycle)
    }
}

// ---------------------------------------------------------------------------
// ServiceDescriptorRegistry — tracks descriptors for all services
// ---------------------------------------------------------------------------

/// A registry mapping service names to their [`ServiceDescriptor`]s.
///
/// This sits alongside [`ServiceCollection`] and provides metadata that the
/// collection itself does not track (kind, declared dependencies, etc.).
pub struct ServiceDescriptorRegistry {
    descriptors: HashMap<String, ServiceDescriptor>,
}

impl ServiceDescriptorRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            descriptors: HashMap::new(),
        }
    }

    /// Register a descriptor. Replaces any previous descriptor with the same
    /// name.
    pub fn register(&mut self, descriptor: ServiceDescriptor) {
        self.descriptors.insert(descriptor.name.clone(), descriptor);
    }

    /// Look up a descriptor by service name.
    pub fn get(&self, name: &str) -> Option<&ServiceDescriptor> {
        self.descriptors.get(name)
    }

    /// Mutably look up a descriptor by service name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ServiceDescriptor> {
        self.descriptors.get_mut(name)
    }

    /// Build the dependency graph from all registered descriptors.
    pub fn dependency_graph(&self) -> HashMap<String, Vec<String>> {
        self.descriptors
            .iter()
            .map(|(name, desc)| (name.clone(), desc.dependencies.clone()))
            .collect()
    }

    /// Validate that all declared dependencies reference services that are also
    /// registered, and that no circular dependencies exist.
    ///
    /// Returns a list of error messages (empty when valid).
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Check for missing dependencies.
        for (name, desc) in &self.descriptors {
            for dep in &desc.dependencies {
                if !self.descriptors.contains_key(dep) {
                    errors.push(format!(
                        "Service '{}' depends on '{}' which is not registered",
                        name, dep
                    ));
                }
            }
        }

        // Check for circular dependencies.
        let graph = self.dependency_graph();
        if let Some(cycle) = detect_circular_dependency(&graph) {
            errors.push(format!("Circular dependency detected: {}", cycle.join(" -> ")));
        }

        errors
    }

    /// Number of descriptors registered.
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// Returns `true` if no descriptors are registered.
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }

    /// Iterate over all descriptors.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ServiceDescriptor)> {
        self.descriptors.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Return only singleton descriptors.
    pub fn singletons(&self) -> Vec<&ServiceDescriptor> {
        self.descriptors.values().filter(|d| d.is_singleton()).collect()
    }

    /// Return only transient descriptors.
    pub fn transients(&self) -> Vec<&ServiceDescriptor> {
        self.descriptors.values().filter(|d| d.is_transient()).collect()
    }
}

impl Default for ServiceDescriptorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ResolutionEvent / ResolutionTrace — resolution tracing/logging
// ---------------------------------------------------------------------------

/// A single resolution event recorded by [`ResolutionTrace`].
#[derive(Debug, Clone)]
pub struct ResolutionEvent {
    /// The service that was resolved.
    pub service_name: String,
    /// Whether the resolution was successful.
    pub success: bool,
    /// Sequential event number (zero-based).
    pub sequence: usize,
}

/// Records an ordered sequence of service resolution events for diagnostics.
///
/// Attach a `ResolutionTrace` to your container and call [`record`](Self::record)
/// each time a service is resolved to build a complete audit trail.
pub struct ResolutionTrace {
    events: Vec<ResolutionEvent>,
}

impl ResolutionTrace {
    /// Create a new, empty trace.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Record a successful resolution.
    pub fn record_success(&mut self, service_name: impl Into<String>) {
        let seq = self.events.len();
        self.events.push(ResolutionEvent {
            service_name: service_name.into(),
            success: true,
            sequence: seq,
        });
    }

    /// Record a failed resolution.
    pub fn record_failure(&mut self, service_name: impl Into<String>) {
        let seq = self.events.len();
        self.events.push(ResolutionEvent {
            service_name: service_name.into(),
            success: false,
            sequence: seq,
        });
    }

    /// All events in order.
    pub fn events(&self) -> &[ResolutionEvent] {
        &self.events
    }

    /// Number of events recorded.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns `true` if no events have been recorded.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Count of successful resolutions.
    pub fn success_count(&self) -> usize {
        self.events.iter().filter(|e| e.success).count()
    }

    /// Count of failed resolutions.
    pub fn failure_count(&self) -> usize {
        self.events.iter().filter(|e| !e.success).count()
    }

    /// Return the names of all services that failed to resolve (deduplicated).
    pub fn failed_services(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .events
            .iter()
            .filter(|e| !e.success)
            .map(|e| e.service_name.as_str())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Format the trace as a human-readable multi-line log.
    pub fn format_log(&self) -> String {
        let mut out = String::new();
        for evt in &self.events {
            let status = if evt.success { "OK" } else { "FAIL" };
            out.push_str(&format!(
                "[{}] #{}: {}\n",
                status, evt.sequence, evt.service_name
            ));
        }
        out
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

impl Default for ResolutionTrace {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ResolutionTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "trace: {} event(s), {} ok, {} failed",
            self.len(),
            self.success_count(),
            self.failure_count()
        )
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

// -- ServiceScopeGuard for scoped injection ----------------------------------

/// A scope that tracks services registered within it. When dropped,
/// the services are conceptually removed from the scope.
#[derive(Debug)]
pub struct ServiceScopeGuard {
    scope_name: String,
    registered_types: Vec<String>,
    active: bool,
}

impl ServiceScopeGuard {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            scope_name: name.into(),
            registered_types: Vec::new(),
            active: true,
        }
    }

    pub fn scope_name(&self) -> &str {
        &self.scope_name
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Record that a service type was registered in this scope.
    pub fn track(&mut self, type_name: &str) {
        if self.active {
            self.registered_types.push(type_name.to_string());
        }
    }

    /// Number of services registered in this scope.
    pub fn service_count(&self) -> usize {
        self.registered_types.len()
    }

    /// Get all tracked type names.
    pub fn tracked_types(&self) -> &[String] {
        &self.registered_types
    }

    /// Close the scope, marking it inactive.
    pub fn close(&mut self) {
        self.active = false;
    }
}

impl fmt::Display for ServiceScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.active { "active" } else { "closed" };
        write!(
            f,
            "Scope({}, {}, {} services)",
            self.scope_name,
            status,
            self.registered_types.len()
        )
    }
}

impl Drop for ServiceScopeGuard {
    fn drop(&mut self) {
        self.active = false;
    }
}

// -- ServiceDecorator for wrapping services ----------------------------------

/// Describes a decorator that wraps an existing service.
#[derive(Debug, Clone)]
pub struct ServiceDecorator {
    pub target_service: String,
    pub decorator_name: String,
    pub priority: i32,
}

impl ServiceDecorator {
    pub fn new(target: &str, decorator: &str, priority: i32) -> Self {
        Self {
            target_service: target.to_string(),
            decorator_name: decorator.to_string(),
            priority,
        }
    }
}

impl fmt::Display for ServiceDecorator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Decorator({} -> {}, priority={})",
            self.decorator_name, self.target_service, self.priority
        )
    }
}

/// Registry for service decorators.
#[derive(Debug, Default)]
pub struct DecoratorRegistry {
    decorators: Vec<ServiceDecorator>,
}

impl DecoratorRegistry {
    pub fn new() -> Self {
        Self {
            decorators: Vec::new(),
        }
    }

    pub fn register(&mut self, decorator: ServiceDecorator) {
        self.decorators.push(decorator);
        self.decorators.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn for_service(&self, service_name: &str) -> Vec<&ServiceDecorator> {
        self.decorators
            .iter()
            .filter(|d| d.target_service == service_name)
            .collect()
    }

    pub fn count(&self) -> usize {
        self.decorators.len()
    }
}

// -- ServiceDiagnostics showing dependency graph -----------------------------

/// A node in the service dependency graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyNode {
    pub service_name: String,
    pub dependencies: Vec<String>,
}

/// Build a simple dependency graph from descriptors.
pub fn build_dependency_graph(descriptors: &[ServiceDescriptor]) -> Vec<DependencyNode> {
    descriptors
        .iter()
        .map(|d| DependencyNode {
            service_name: d.name.clone(),
            dependencies: d.dependencies.clone(),
        })
        .collect()
}

/// Find services with no dependencies (roots).
pub fn find_root_services(graph: &[DependencyNode]) -> Vec<&str> {
    graph
        .iter()
        .filter(|n| n.dependencies.is_empty())
        .map(|n| n.service_name.as_str())
        .collect()
}

/// Find services that nothing depends on (leaves).
pub fn find_leaf_services(graph: &[DependencyNode]) -> Vec<&str> {
    let all_deps: std::collections::HashSet<&str> = graph
        .iter()
        .flat_map(|n| n.dependencies.iter().map(|s| s.as_str()))
        .collect();
    graph
        .iter()
        .filter(|n| !all_deps.contains(n.service_name.as_str()))
        .map(|n| n.service_name.as_str())
        .collect()
}

/// Detect circular dependencies (simple check).
pub fn has_circular_dependency(graph: &[DependencyNode]) -> bool {
    for node in graph {
        if node.dependencies.contains(&node.service_name) {
            return true;
        }
    }
    // Check 2-level cycles
    for node in graph {
        for dep in &node.dependencies {
            if let Some(dep_node) = graph.iter().find(|n| n.service_name == *dep) {
                if dep_node.dependencies.contains(&node.service_name) {
                    return true;
                }
            }
        }
    }
    false
}

// -- Lazy service initialization tracking ------------------------------------

/// Tracks whether services have been lazily initialized.
#[derive(Debug, Default)]
pub struct LazyInitTracker {
    initialized: HashMap<String, bool>,
}

impl LazyInitTracker {
    pub fn new() -> Self {
        Self {
            initialized: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str) {
        self.initialized.insert(name.to_string(), false);
    }

    pub fn mark_initialized(&mut self, name: &str) {
        if let Some(v) = self.initialized.get_mut(name) {
            *v = true;
        }
    }

    pub fn is_initialized(&self, name: &str) -> bool {
        self.initialized.get(name).copied().unwrap_or(false)
    }

    pub fn initialized_count(&self) -> usize {
        self.initialized.values().filter(|&&v| v).count()
    }

    pub fn pending_count(&self) -> usize {
        self.initialized.values().filter(|&&v| !v).count()
    }

    pub fn all_names(&self) -> Vec<&str> {
        self.initialized.keys().map(|s| s.as_str()).collect()
    }
}

impl fmt::Display for LazyInitTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LazyInit({} initialized, {} pending)",
            self.initialized_count(),
            self.pending_count()
        )
    }
}

// ---------------------------------------------------------------------------
// ServiceHealthChecker - service health checker
// ---------------------------------------------------------------------------

/// Severity level for service health checker issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ServiceHealthCheckerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ServiceHealthCheckerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ServiceHealthChecker].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceHealthCheckerEntry {
    pub id: String,
    pub label: String,
    pub severity: ServiceHealthCheckerSeverity,
    pub detail: Option<String>,
    pub service_count: usize,
    enabled: bool,
}

impl ServiceHealthCheckerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ServiceHealthCheckerSeverity::Low,
            detail: None,
            service_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ServiceHealthCheckerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_service_count(mut self, val: usize) -> Self {
        self.service_count = val;
        self
    }

    pub fn all_healthy(&self) -> bool {
        self.enabled && self.severity >= ServiceHealthCheckerSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.service_count, det)
    }
}

impl fmt::Display for ServiceHealthCheckerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ServiceHealthCheckerEntry] items.
#[derive(Debug, Clone)]
pub struct ServiceHealthChecker {
    entries: Vec<ServiceHealthCheckerEntry>,
    name: String,
    capacity: usize,
}

impl ServiceHealthChecker {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ServiceHealthCheckerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ServiceHealthCheckerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ServiceHealthCheckerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn service_count(&self) -> usize { self.entries.len() }

    pub fn all_healthy(&self) -> bool {
        self.entries.iter().any(|e| e.all_healthy())
    }

    pub fn entries_by_severity(&self, severity: ServiceHealthCheckerSeverity) -> Vec<&ServiceHealthCheckerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ServiceHealthCheckerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ServiceHealthCheckerEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&ServiceHealthCheckerEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// ServiceInitProfiler - service initialization profiler
// ---------------------------------------------------------------------------

/// Configuration for [ServiceInitProfiler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInitProfilerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub init_time_ms: usize,
}

impl ServiceInitProfilerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, init_time_ms: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_init_time_ms(mut self, val: usize) -> Self { self.init_time_ms = val; self }
}

impl Default for ServiceInitProfilerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ServiceInitProfiler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInitProfilerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ServiceInitProfilerItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn is_initialized(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ServiceInitProfilerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ServiceInitProfilerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ServiceInitProfiler {
    config: ServiceInitProfilerConfig,
    items: Vec<ServiceInitProfilerItem>,
}

impl ServiceInitProfiler {
    pub fn new(config: ServiceInitProfilerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ServiceInitProfilerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ServiceInitProfilerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ServiceInitProfilerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn init_time_ms(&self) -> usize { self.items.len() }

    pub fn is_initialized(&self) -> bool {
        self.items.iter().any(|i| i.is_initialized())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ServiceInitProfilerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ServiceInitProfilerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ServiceInitProfilerConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



/// Dependency injection configuration manager.
#[derive(Debug, Clone)]
pub struct DiConfig {
    entries: Vec<DiEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single dependency injection entry.
#[derive(Debug, Clone, PartialEq)]
pub struct DiEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl DiEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl DiConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: DiEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&DiEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut DiEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&DiEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&DiEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&DiEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<DiEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Dependency injection container — extended utilities (qu)
// ---------------------------------------------------------------------------

/// Metric accumulator for di operations.
#[derive(Debug, Clone)]
pub struct QuMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QuMetrics {
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

/// Sliding-window rate counter for di.
#[derive(Debug, Clone)]
pub struct QuRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QuRateWindow {
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

/// A small LRU-style cache for di lookups.
#[derive(Debug, Clone)]
pub struct QuLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QuLruCache {
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
// xb_ utilities – batch 10
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer10 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer10 {
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
pub fn xb_fnv1a_10(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_10<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_10<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_10(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_10(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 27
// ---------------------------------------------------------------------------

/// Generic object pool `Xc27Pool<T>`.
pub struct Xc27Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc27Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc27PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc27Pool<T> {
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
    pub fn stats(&self) -> Xc27PoolStats {
        Xc27PoolStats {
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

impl<T> Default for Xc27Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc27Scheduler`.
pub struct Xc27Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc27Scheduler {
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

impl Default for Xc27Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_27 hash for the given byte slice.
pub fn xc_27_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_27 convention.
pub fn xc_27_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe22 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe22Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe22PipelineError {
    pub stage: Xe22Stage,
    pub message: String,
}

impl std::fmt::Display for Xe22PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe22Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe22Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError>>>,
    stage_names: Vec<Xe22Stage>,
}

impl Xe22Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe22Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe22Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe22Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe22Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> {
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

    pub fn compose(mut self, other: Xe22Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe22CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe22CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe22Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe22CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe22CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe22Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe22CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_22_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe22CacheEntry {
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

    fn xe_22_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe22CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_22_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> {
    Ok(data)
}

pub fn xe_22_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_22_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_22_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_22_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe22PipelineError> {
    Err(Xe22PipelineError {
        stage: Xe22Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #98
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf98Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf98TrieNode {
    children: std::collections::HashMap<char, Xf98TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf98Trie {
    root: Xf98TrieNode,
    count: usize,
}

impl Xf98Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf98TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf98TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf98TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf98BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf98BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 26).
pub struct Xh26SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh26SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 68 as u64,
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

/// A compact bit set supporting boolean operations (variant 26).
pub struct Xh26BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh26BitSet {
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

    // -- ServiceDescriptor / ServiceDescriptorRegistry ----------------------

    #[test]
    fn descriptor_builder_and_lifecycle_transitions() {
        let mut desc = ServiceDescriptor::new("ILogService", ServiceKind::Singleton)
            .depends_on("IConfigService")
            .depends_on("IFileService");

        assert_eq!(desc.name, "ILogService");
        assert!(desc.is_singleton());
        assert!(!desc.is_transient());
        assert_eq!(desc.dependencies.len(), 2);
        assert_eq!(desc.lifecycle, ServiceLifecycle::Registered);
        assert_eq!(desc.to_string(), "ILogService [Singleton] (Registered)");

        desc.activate();
        assert_eq!(desc.lifecycle, ServiceLifecycle::Active);

        desc.mark_disposed();
        assert_eq!(desc.lifecycle, ServiceLifecycle::Disposed);
    }

    #[test]
    fn descriptor_registry_validate_detects_missing_and_cycles() {
        let mut registry = ServiceDescriptorRegistry::new();
        registry.register(
            ServiceDescriptor::new("A", ServiceKind::Singleton).depends_on("B"),
        );
        registry.register(
            ServiceDescriptor::new("B", ServiceKind::Transient).depends_on("C"),
        );
        // "C" is not registered — should produce a missing-dependency error.
        let errors = registry.validate();
        assert!(
            errors.iter().any(|e| e.contains("depends on 'C'")),
            "expected missing dep error, got: {:?}",
            errors,
        );

        // Now register C with a cycle back to A.
        registry.register(
            ServiceDescriptor::new("C", ServiceKind::Singleton).depends_on("A"),
        );
        let errors = registry.validate();
        assert!(
            errors.iter().any(|e| e.contains("Circular dependency")),
            "expected cycle error, got: {:?}",
            errors,
        );
    }

    #[test]
    fn descriptor_registry_singletons_and_transients() {
        let mut registry = ServiceDescriptorRegistry::new();
        registry.register(ServiceDescriptor::new("S1", ServiceKind::Singleton));
        registry.register(ServiceDescriptor::new("S2", ServiceKind::Singleton));
        registry.register(ServiceDescriptor::new("T1", ServiceKind::Transient));

        assert_eq!(registry.len(), 3);
        assert_eq!(registry.singletons().len(), 2);
        assert_eq!(registry.transients().len(), 1);
        assert!(!registry.is_empty());

        // Dependency graph should have three nodes.
        let graph = registry.dependency_graph();
        assert_eq!(graph.len(), 3);

        // All deps are empty so validation should pass.
        assert!(registry.validate().is_empty());
    }

    // -- ResolutionTrace ----------------------------------------------------

    #[test]
    fn resolution_trace_records_and_reports() {
        let mut trace = ResolutionTrace::new();
        assert!(trace.is_empty());

        trace.record_success("IConfigService");
        trace.record_success("ILogService");
        trace.record_failure("IMissingService");
        trace.record_success("IEditorService");

        assert_eq!(trace.len(), 4);
        assert_eq!(trace.success_count(), 3);
        assert_eq!(trace.failure_count(), 1);
        assert_eq!(trace.failed_services(), vec!["IMissingService"]);

        let log = trace.format_log();
        assert!(log.contains("[OK] #0: IConfigService"));
        assert!(log.contains("[FAIL] #2: IMissingService"));

        let display = trace.to_string();
        assert!(display.contains("4 event(s)"));
        assert!(display.contains("3 ok"));
        assert!(display.contains("1 failed"));

        trace.clear();
        assert!(trace.is_empty());
    }

    #[test]
    fn resolution_trace_deduplicates_failed_services() {
        let mut trace = ResolutionTrace::new();
        trace.record_failure("X");
        trace.record_failure("Y");
        trace.record_failure("X"); // duplicate
        let failed = trace.failed_services();
        assert_eq!(failed, vec!["X", "Y"]);
    }

    #[test]
    fn descriptor_registry_get_mut_activates() {
        let mut registry = ServiceDescriptorRegistry::new();
        registry.register(ServiceDescriptor::new("Svc", ServiceKind::Singleton));

        assert_eq!(
            registry.get("Svc").unwrap().lifecycle,
            ServiceLifecycle::Registered
        );
        registry.get_mut("Svc").unwrap().activate();
        assert_eq!(
            registry.get("Svc").unwrap().lifecycle,
            ServiceLifecycle::Active
        );
    }

    #[test]
    fn service_kind_display() {
        assert_eq!(ServiceKind::Singleton.to_string(), "Singleton");
        assert_eq!(ServiceKind::Transient.to_string(), "Transient");
    }

    // -- ServiceScopeGuard tests ----------------------------------------------

    #[test]
    fn scope_guard_track_and_close() {
        let mut scope = ServiceScopeGuard::new("test-scope");
        assert!(scope.is_active());
        scope.track("Logger");
        scope.track("Config");
        assert_eq!(scope.service_count(), 2);
        scope.close();
        assert!(!scope.is_active());
    }

    #[test]
    fn scope_guard_no_track_when_closed() {
        let mut scope = ServiceScopeGuard::new("closed");
        scope.close();
        scope.track("ShouldNotTrack");
        assert_eq!(scope.service_count(), 0);
    }

    #[test]
    fn scope_guard_display() {
        let scope = ServiceScopeGuard::new("my-scope");
        let s = scope.to_string();
        assert!(s.contains("my-scope"));
        assert!(s.contains("active"));
    }

    // -- ServiceDecorator tests -----------------------------------------------

    #[test]
    fn decorator_registry_for_service() {
        let mut reg = DecoratorRegistry::new();
        reg.register(ServiceDecorator::new("Logger", "LogDecorator", 10));
        reg.register(ServiceDecorator::new("Logger", "MetricsDecorator", 5));
        reg.register(ServiceDecorator::new("Config", "ConfigDecorator", 1));
        let logger_decs = reg.for_service("Logger");
        assert_eq!(logger_decs.len(), 2);
        assert_eq!(logger_decs[0].priority, 10);
    }

    #[test]
    fn decorator_display() {
        let dec = ServiceDecorator::new("Target", "Dec", 5);
        let s = dec.to_string();
        assert!(s.contains("Dec"));
        assert!(s.contains("Target"));
    }

    // -- LazyInitTracker tests ------------------------------------------------

    #[test]
    fn lazy_init_tracker_workflow() {
        let mut tracker = LazyInitTracker::new();
        tracker.register("A");
        tracker.register("B");
        assert_eq!(tracker.pending_count(), 2);
        assert_eq!(tracker.initialized_count(), 0);

        tracker.mark_initialized("A");
        assert!(tracker.is_initialized("A"));
        assert!(!tracker.is_initialized("B"));
        assert_eq!(tracker.initialized_count(), 1);
    }

    #[test]
    fn lazy_init_tracker_display() {
        let mut tracker = LazyInitTracker::new();
        tracker.register("X");
        tracker.mark_initialized("X");
        let s = tracker.to_string();
        assert!(s.contains("1 initialized"));
        assert!(s.contains("0 pending"));
    }

    #[test]
    fn lazy_init_unknown_service() {
        let tracker = LazyInitTracker::new();
        assert!(!tracker.is_initialized("nonexistent"));
    }

    // -- Dependency graph tests -----------------------------------------------

    #[test]
    fn build_dependency_graph_from_descriptors() {
        let descriptors = vec![
            ServiceDescriptor::new("A", ServiceKind::Singleton),
            ServiceDescriptor::new("B", ServiceKind::Transient).depends_on("A"),
        ];
        let graph = build_dependency_graph(&descriptors);
        assert_eq!(graph.len(), 2);
        assert!(graph[0].dependencies.is_empty());
        assert_eq!(graph[1].dependencies, vec!["A".to_string()]);
    }

    #[test]
    fn find_root_services_works() {
        let graph = vec![
            DependencyNode {
                service_name: "A".into(),
                dependencies: vec![],
            },
            DependencyNode {
                service_name: "B".into(),
                dependencies: vec!["A".into()],
            },
        ];
        let roots = find_root_services(&graph);
        assert_eq!(roots, vec!["A"]);
    }

    #[test]
    fn find_leaf_services_works() {
        let graph = vec![
            DependencyNode {
                service_name: "A".into(),
                dependencies: vec![],
            },
            DependencyNode {
                service_name: "B".into(),
                dependencies: vec!["A".into()],
            },
        ];
        let leaves = find_leaf_services(&graph);
        assert_eq!(leaves, vec!["B"]);
    }

    #[test]
    fn circular_dependency_self_ref() {
        let graph = vec![DependencyNode {
            service_name: "A".into(),
            dependencies: vec!["A".into()],
        }];
        assert!(has_circular_dependency(&graph));
    }

    #[test]
    fn circular_dependency_mutual() {
        let graph = vec![
            DependencyNode {
                service_name: "A".into(),
                dependencies: vec!["B".into()],
            },
            DependencyNode {
                service_name: "B".into(),
                dependencies: vec!["A".into()],
            },
        ];
        assert!(has_circular_dependency(&graph));
    }

    #[test]
    fn no_circular_dependency() {
        let graph = vec![
            DependencyNode {
                service_name: "A".into(),
                dependencies: vec![],
            },
            DependencyNode {
                service_name: "B".into(),
                dependencies: vec!["A".into()],
            },
        ];
        assert!(!has_circular_dependency(&graph));
    }

#[test]
    fn servicehealthchecker_severity_ordering() {
        assert!(ServiceHealthCheckerSeverity::Critical > ServiceHealthCheckerSeverity::High);
        assert!(ServiceHealthCheckerSeverity::High > ServiceHealthCheckerSeverity::Medium);
        assert!(ServiceHealthCheckerSeverity::Medium > ServiceHealthCheckerSeverity::Low);
    }

    #[test]
    fn servicehealthchecker_severity_display() {
        assert_eq!(ServiceHealthCheckerSeverity::Low.to_string(), "low");
        assert_eq!(ServiceHealthCheckerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn servicehealthchecker_entry_creation() {
        let e = ServiceHealthCheckerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ServiceHealthCheckerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn servicehealthchecker_entry_builder() {
        let e = ServiceHealthCheckerEntry::new("e2", "Entry 2")
            .with_severity(ServiceHealthCheckerSeverity::High)
            .with_detail("some detail")
            .with_service_count(42);
        assert_eq!(e.severity, ServiceHealthCheckerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.service_count, 42);
    }

    #[test]
    fn servicehealthchecker_entry_enable_disable() {
        let mut e = ServiceHealthCheckerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn servicehealthchecker_add_and_count() {
        let mut mgr = ServiceHealthChecker::new("test");
        mgr.add(ServiceHealthCheckerEntry::new("a", "A"));
        mgr.add(ServiceHealthCheckerEntry::new("b", "B").with_severity(ServiceHealthCheckerSeverity::High));
        assert_eq!(mgr.service_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn servicehealthchecker_remove() {
        let mut mgr = ServiceHealthChecker::new("test");
        mgr.add(ServiceHealthCheckerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn servicehealthchecker_capacity() {
        let mut mgr = ServiceHealthChecker::new("test").with_capacity(1);
        assert!(mgr.add(ServiceHealthCheckerEntry::new("a", "A")));
        assert!(!mgr.add(ServiceHealthCheckerEntry::new("b", "B")));
    }

    #[test]
    fn servicehealthchecker_sorted_by_severity() {
        let mut mgr = ServiceHealthChecker::new("test");
        mgr.add(ServiceHealthCheckerEntry::new("lo", "Low"));
        mgr.add(ServiceHealthCheckerEntry::new("hi", "High").with_severity(ServiceHealthCheckerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ServiceHealthCheckerSeverity::Critical);
    }

    #[test]
    fn servicehealthchecker_summary() {
        let mgr = ServiceHealthChecker::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn serviceinitprofiler_config_defaults() {
        let cfg = ServiceInitProfilerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn serviceinitprofiler_item_creation() {
        let item = ServiceInitProfilerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn serviceinitprofiler_add_and_get() {
        let mut mgr = ServiceInitProfiler::new(ServiceInitProfilerConfig::new("test"));
        mgr.add(ServiceInitProfilerItem::new("k1", "v1"));
        assert_eq!(mgr.init_time_ms(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn serviceinitprofiler_remove_item() {
        let mut mgr = ServiceInitProfiler::new(ServiceInitProfilerConfig::new("test"));
        mgr.add(ServiceInitProfilerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn serviceinitprofiler_sorted_by_priority() {
        let mut mgr = ServiceInitProfiler::new(ServiceInitProfilerConfig::new("test"));
        mgr.add(ServiceInitProfilerItem::new("lo", "low").with_priority(1));
        mgr.add(ServiceInitProfilerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn serviceinitprofiler_items_with_tag() {
        let mut mgr = ServiceInitProfiler::new(ServiceInitProfilerConfig::new("test"));
        mgr.add(ServiceInitProfilerItem::new("a", "1").with_tag("x"));
        mgr.add(ServiceInitProfilerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn serviceinitprofiler_report() {
        let mgr = ServiceInitProfiler::new(ServiceInitProfilerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn di_entry_creation() {
        let e = DiEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn di_entry_with_priority() {
        let e = DiEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn di_entry_metadata() {
        let e = DiEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn di_entry_remove_meta() {
        let mut e = DiEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn di_entry_activate_deactivate() {
        let mut e = DiEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn di_config_add_sorted() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("lo", "Lo").with_priority(1));
        c.add(DiEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn di_config_capacity() {
        let mut c = DiConfig::new(1);
        assert!(c.add(DiEntry::new("a", "A")));
        assert!(!c.add(DiEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn di_config_remove() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn di_config_get() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn di_config_active_entries() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("a", "A"));
        c.add(DiEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn di_config_enable_disable() {
        let mut c = DiConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn di_config_clear() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn di_config_find_by_label() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn di_config_top_n() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("a", "A").with_priority(1));
        c.add(DiEntry::new("b", "B").with_priority(2));
        c.add(DiEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn di_config_deactivate_activate_all() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("a", "A"));
        c.add(DiEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn di_config_highest_priority() {
        let mut c = DiConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(DiEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn di_config_contains() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn di_config_labels() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("a", "Alpha"));
        c.add(DiEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn di_config_drain_inactive() {
        let mut c = DiConfig::new(10);
        c.add(DiEntry::new("a", "A"));
        c.add(DiEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qu_metrics_empty() {
        let m = QuMetrics::new("di");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qu_metrics_record_and_mean() {
        let mut m = QuMetrics::new("di");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qu_metrics_min_max() {
        let mut m = QuMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qu_metrics_variance_and_std() {
        let mut m = QuMetrics::new("v");
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
    fn qu_metrics_percentile() {
        let mut m = QuMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qu_metrics_merge() {
        let mut a = QuMetrics::new("a");
        a.record(1.0);
        let mut b = QuMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qu_metrics_reset() {
        let mut m = QuMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qu_rate_window_empty() {
        let rw = QuRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qu_rate_window_tick_and_rate() {
        let mut rw = QuRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qu_lru_cache_basic() {
        let mut c = QuLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qu_lru_cache_contains_and_keys() {
        let mut c = QuLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qu_lru_cache_remove() {
        let mut c = QuLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qu_metrics_sum() {
        let mut m = QuMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qu_metrics_label() {
        let m = QuMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qu_lru_cache_clear() {
        let mut c = QuLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_10_push_and_len() {
        let mut rb = super::XbRingBuffer10::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_10_overwrite() {
        let mut rb = super::XbRingBuffer10::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_10_get_out_of_bounds() {
        let rb = super::XbRingBuffer10::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_10_drain_all() {
        let mut rb = super::XbRingBuffer10::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_10_peek_front_back() {
        let mut rb = super::XbRingBuffer10::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_10_clear() {
        let mut rb = super::XbRingBuffer10::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_10_capacity() {
        let rb = super::XbRingBuffer10::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_10_basic() {
        let h = super::xb_fnv1a_10(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_10(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_10_different_inputs() {
        let h1 = super::xb_fnv1a_10(b"abc");
        let h2 = super::xb_fnv1a_10(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_10_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_10(&data);
        let dec = super::xb_rle_decode_10(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_10_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_10(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_10(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_10_values() {
        assert!((super::xb_clamp_10(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_10(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_10(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_10_values() {
        assert!((super::xb_lerp_10(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_10(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_10(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_10_wrap_around_twice() {
        let mut rb = super::XbRingBuffer10::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 27 ----

    #[test]
    fn xc_27_pool_new_empty() {
        let pool: super::Xc27Pool<i32> = super::Xc27Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_27_pool_release_acquire() {
        let mut pool = super::Xc27Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_27_pool_acquire_empty() {
        let mut pool: super::Xc27Pool<i32> = super::Xc27Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_27_pool_full() {
        let mut pool = super::Xc27Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_27_pool_drain() {
        let mut pool = super::Xc27Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_27_pool_stats() {
        let mut pool = super::Xc27Pool::new(8);
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
    fn xc_27_pool_clear() {
        let mut pool = super::Xc27Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_27_pool_shrink() {
        let mut pool = super::Xc27Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_27_pool_default() {
        let pool: super::Xc27Pool<String> = super::Xc27Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_27_pool_extend() {
        let mut pool = super::Xc27Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_27_pool_retain() {
        let mut pool = super::Xc27Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_27_scheduler_round_robin() {
        let mut sched = super::Xc27Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_27_scheduler_empty() {
        let mut sched = super::Xc27Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_27_scheduler_reset() {
        let mut sched = super::Xc27Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_27_scheduler_add_remove() {
        let mut sched = super::Xc27Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_27_scheduler_targets() {
        let sched = super::Xc27Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_27_hash_empty() {
        assert_eq!(super::xc_27_hash(b""), 5381);
    }

    #[test]
    fn xc_27_hash_data() {
        let h = super::xc_27_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_27_hash(b"hello"), h);
    }

    #[test]
    fn xc_27_reverse_str() {
        assert_eq!(super::xc_27_reverse("abc"), "cba");
        assert_eq!(super::xc_27_reverse(""), "");
    }


    #[test]
    fn xe_22_pipeline_empty() {
        let p = super::Xe22Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_22_pipeline_parse_stage() {
        let p = super::Xe22Pipeline::new()
            .add_parse(super::xe_22_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_22_pipeline_transform_double() {
        let p = super::Xe22Pipeline::new()
            .add_transform(super::xe_22_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_22_pipeline_validate_reverse() {
        let p = super::Xe22Pipeline::new()
            .add_validate(super::xe_22_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_22_pipeline_emit_filter() {
        let p = super::Xe22Pipeline::new()
            .add_emit(super::xe_22_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_22_pipeline_multi_stage() {
        let p = super::Xe22Pipeline::new()
            .add_parse(super::xe_22_pipeline_identity)
            .add_transform(super::xe_22_pipeline_double)
            .add_validate(super::xe_22_pipeline_reverse)
            .add_emit(super::xe_22_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_22_pipeline_error_propagation() {
        let p = super::Xe22Pipeline::new()
            .add_parse(super::xe_22_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe22Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_22_pipeline_compose() {
        let p1 = super::Xe22Pipeline::new()
            .add_parse(super::xe_22_pipeline_identity);
        let p2 = super::Xe22Pipeline::new()
            .add_transform(super::xe_22_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_22_pipeline_error_display() {
        let e = super::Xe22PipelineError {
            stage: super::Xe22Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_22_cache_put_get() {
        let mut c = super::Xe22Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_22_cache_miss() {
        let mut c: super::Xe22Cache<&str, i32> = super::Xe22Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_22_cache_ttl_expiry() {
        let mut c = super::Xe22Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_22_cache_evict() {
        let mut c = super::Xe22Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_22_cache_capacity() {
        let mut c = super::Xe22Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_22_cache_stats() {
        let mut c = super::Xe22Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_22_cache_clear() {
        let mut c = super::Xe22Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #98 --

    #[test]
    fn xf98_trie_insert_search() {
        let mut t = Xf98Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf98_trie_starts_with() {
        let mut t = Xf98Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf98_trie_remove() {
        let mut t = Xf98Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf98_trie_word_count() {
        let mut t = Xf98Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf98_trie_longest_prefix() {
        let mut t = Xf98Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf98_trie_all_words() {
        let mut t = Xf98Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf98_trie_autocomplete() {
        let mut t = Xf98Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf98_trie_empty_search() {
        let t = Xf98Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf98_bloom_add_contains() {
        let mut bf = Xf98BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf98_bloom_probably_absent() {
        let bf = Xf98BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf98_bloom_false_positive_rate() {
        let mut bf = Xf98BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf98_bloom_clear() {
        let mut bf = Xf98BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf98_bloom_union() {
        let mut a = Xf98BloomFilter::xf_new(512, 2);
        let mut b = Xf98BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf98_bloom_intersection_estimate() {
        let mut a = Xf98BloomFilter::xf_new(512, 2);
        let mut b = Xf98BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf98_bloom_union_size_mismatch() {
        let a = Xf98BloomFilter::xf_new(256, 2);
        let b = Xf98BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh26_skip_insert_contains() {
        let mut sl = super::Xh26SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh26_skip_remove() {
        let mut sl = super::Xh26SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh26_skip_len() {
        let mut sl = super::Xh26SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh26_skip_range_query() {
        let mut sl = super::Xh26SkipList::xh_new(4);
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
    fn xh26_skip_floor_ceiling() {
        let mut sl = super::Xh26SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh26_skip_rank() {
        let mut sl = super::Xh26SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh26_skip_empty() {
        let sl = super::Xh26SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh26_skip_duplicates() {
        let mut sl = super::Xh26SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh26_bitset_set_test() {
        let mut bs = super::Xh26BitSet::xh_new(256);
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
    fn xh26_bitset_clear_count() {
        let mut bs = super::Xh26BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh26_bitset_and_or_xor() {
        let mut a = super::Xh26BitSet::xh_new(128);
        let mut b = super::Xh26BitSet::xh_new(128);
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
    fn xh26_bitset_iter_ones() {
        let mut bs = super::Xh26BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh26_bitset_first_last() {
        let mut bs = super::Xh26BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh26_bitset_empty() {
        let bs = super::Xh26BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
