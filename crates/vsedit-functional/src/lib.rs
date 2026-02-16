//! Functional programming utilities.
//!
//! Equivalent to VS Code's `vs/base/common/functional.ts`.

use std::collections::HashMap;


/// Creates a function that can only be called once. Subsequent calls return the
/// first result.
pub fn once<F, T>(f: F) -> impl FnMut() -> T
where
    F: FnOnce() -> T,
    T: Clone,
{
    let mut result: Option<T> = None;
    let mut f = Some(f);
    move || {
        if let Some(r) = &result {
            r.clone()
        } else {
            let r = (f.take().expect("once fn already consumed"))();
            result = Some(r.clone());
            r
        }
    }
}

/// Identity function — returns its argument unchanged.
pub fn identity<T>(x: T) -> T {
    x
}

/// Compose two functions: `compose(f, g)(x)` = `g(f(x))`.
pub fn compose<A, B, C, F, G>(f: F, g: G) -> impl Fn(A) -> C
where
    F: Fn(A) -> B,
    G: Fn(B) -> C,
{
    move |a| g(f(a))
}

/// Create a function that always returns the same value.
pub fn constant<T: Clone>(value: T) -> impl Fn() -> T {
    move || value.clone()
}

/// Create a memoized function that caches the last result.
pub fn memoize_last<A, R, F>(mut f: F) -> impl FnMut(A) -> R
where
    A: PartialEq + Clone,
    R: Clone,
    F: FnMut(&A) -> R,
{
    let mut last: Option<(A, R)> = None;
    move |arg: A| {
        if let Some((ref last_arg, ref last_result)) = last {
            if *last_arg == arg {
                return last_result.clone();
            }
        }
        let result = f(&arg);
        last = Some((arg, result.clone()));
        result
    }
}

/// Compose two functions left-to-right: `pipe2(f, g)(x)` = `g(f(x))`.
///
/// This is an alias for [`compose`] that reads in pipeline order,
/// making it clearer that data flows from `f` into `g`.
pub fn pipe2<A, B, C>(f: impl Fn(A) -> B, g: impl Fn(B) -> C) -> impl Fn(A) -> C {
    move |a| g(f(a))
}

/// Returns a function that negates the boolean result of `f`.
pub fn negate<A>(f: impl Fn(A) -> bool) -> impl Fn(A) -> bool {
    move |a| !f(a)
}

/// Flips the first two arguments of a binary function.
pub fn flip<A, B, C>(f: impl Fn(A, B) -> C) -> impl Fn(B, A) -> C {
    move |b, a| f(a, b)
}

/// Calls `f` with a reference to the value for side-effects, then returns the
/// value unchanged. Useful for inserting logging into a pipeline.
pub fn tap<T: Clone>(f: impl Fn(&T)) -> impl Fn(T) -> T {
    move |value| {
        f(&value);
        value
    }
}

/// Returns a closure that only invokes `f` once every `count` calls.
pub fn debounce_count<F: FnMut()>(mut f: F, count: usize) -> impl FnMut() {
    let mut calls: usize = 0;
    move || {
        calls += 1;
        if calls >= count {
            calls = 0;
            f();
        }
    }
}

/// Memoizes a function using a [`HashMap`] cache keyed by the argument.
///
/// Every distinct argument is computed only once; subsequent calls with the
/// same argument return a clone of the cached result.
pub fn memoize<A, R, F>(mut f: F) -> impl FnMut(A) -> R
where
    A: Eq + std::hash::Hash + Clone,
    R: Clone,
    F: FnMut(&A) -> R,
{
    let mut cache: HashMap<A, R> = HashMap::new();
    move |arg: A| {
        if let Some(cached) = cache.get(&arg) {
            return cached.clone();
        }
        let result = f(&arg);
        cache.insert(arg, result.clone());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_once() {
        let mut counter = 0;
        let mut f = once(move || {
            counter += 1;
            counter
        });
        assert_eq!(f(), 1);
        assert_eq!(f(), 1);
    }

    #[test]
    fn test_identity() {
        assert_eq!(identity(42), 42);
        assert_eq!(identity("hello"), "hello");
    }

    #[test]
    fn test_compose() {
        let double = |x: i32| x * 2;
        let add_one = |x: i32| x + 1;
        let double_then_add = compose(double, add_one);
        assert_eq!(double_then_add(5), 11);
    }

    #[test]
    fn test_memoize_last() {
        let mut expensive = memoize_last(|x: &i32| x * 2);
        assert_eq!(expensive(5), 10);
        assert_eq!(expensive(5), 10); // cached
        assert_eq!(expensive(3), 6); // recomputed
    }

    #[test]
    fn test_pipe2() {
        let double = |x: i32| x * 2;
        let add_one = |x: i32| x + 1;
        let pipeline = pipe2(double, add_one);
        assert_eq!(pipeline(5), 11);
        assert_eq!(pipeline(0), 1);
    }

    #[test]
    fn test_negate() {
        let is_positive = |x: i32| x > 0;
        let is_non_positive = negate(is_positive);
        assert!(is_non_positive(-1));
        assert!(is_non_positive(0));
        assert!(!is_non_positive(1));
    }

    #[test]
    fn test_flip() {
        let subtract = |a: i32, b: i32| a - b;
        let flipped = flip(subtract);
        assert_eq!(flipped(3, 10), 7); // 10 - 3
        assert_eq!(flipped(1, 5), 4); // 5 - 1
    }

    #[test]
    fn test_tap() {
        use std::cell::RefCell;
        let log: RefCell<Vec<i32>> = RefCell::new(Vec::new());
        let tap_fn = tap(|x: &i32| log.borrow_mut().push(*x));
        assert_eq!(tap_fn(42), 42);
        assert_eq!(tap_fn(7), 7);
        assert_eq!(*log.borrow(), vec![42, 7]);
    }

    #[test]
    fn test_debounce_count() {
        use std::cell::RefCell;
        let counter: RefCell<i32> = RefCell::new(0);
        let mut debounced = debounce_count(|| *counter.borrow_mut() += 1, 3);
        debounced();
        debounced();
        assert_eq!(*counter.borrow(), 0);
        debounced(); // 3rd call triggers
        assert_eq!(*counter.borrow(), 1);
        debounced();
        debounced();
        debounced(); // 6th call triggers again
        assert_eq!(*counter.borrow(), 2);
    }

    #[test]
    fn test_memoize() {
        use std::cell::RefCell;
        let call_count: RefCell<i32> = RefCell::new(0);
        let mut cached = memoize(move |x: &i32| {
            *call_count.borrow_mut() += 1;
            x * x
        });
        assert_eq!(cached(4), 16);
        assert_eq!(cached(4), 16); // cached
        assert_eq!(cached(5), 25);
        assert_eq!(cached(4), 16); // still cached
    }
}
