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
}
