//! Reactive observable system.
//!
//! Provides `IObservable<T>` and related types, equivalent to
//! VS Code's `vs/base/common/observable.ts`.

use std::sync::{Arc, Mutex};
use vsedit_events::Emitter;

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

    /// Subscribe to value changes. Returns a handle that unsubscribes on drop.
    pub fn on_change(
        &self,
        listener: impl Fn(&T) + Send + Sync + 'static,
    ) -> vsedit_events::DisposableHandle {
        self.on_change.event().on(listener)
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

/// A derived observable that is computed from another observable.
pub struct DerivedObservable<T: Clone + PartialEq + Send + Sync + 'static> {
    inner: Arc<ObservableValue<T>>,
    _subscription: vsedit_events::DisposableHandle,
}

impl<T: Clone + PartialEq + Send + Sync + 'static> DerivedObservable<T> {
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
    ) -> vsedit_events::DisposableHandle {
        self.inner.on_change(listener)
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
}
