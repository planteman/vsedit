//! Lazy evaluation and caching utilities.
//!
//! Provides `Lazy<T>` for deferred computation, equivalent to VS Code's
//! `vs/base/common/lazy.ts`.

use std::cell::OnceCell;
use std::sync::OnceLock;

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
}
