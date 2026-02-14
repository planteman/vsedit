//! Functional programming utilities.
//!
//! Equivalent to VS Code's `vs/base/common/functional.ts`.

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
        let mut call_count = 0;
        let mut expensive = memoize_last(move |x: &i32| {
            call_count += 1;
            x * 2
        });
        assert_eq!(expensive(5), 10);
        assert_eq!(expensive(5), 10); // cached
        assert_eq!(expensive(3), 6); // recomputed
    }
}
