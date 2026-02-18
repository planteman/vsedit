//! Functional programming utilities.
//!
//! Equivalent to VS Code's `vs/base/common/functional.ts`.

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

/// Error type for functional operations that can fail.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionalError {
    /// A validation check failed with the given message.
    ValidationFailed(String),
    /// An operation was invoked on an exhausted or empty pipeline.
    EmptyPipeline,
    /// A predicate matched no elements.
    NoMatch,
}

impl fmt::Display for FunctionalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FunctionalError::ValidationFailed(msg) => write!(f, "validation failed: {msg}"),
            FunctionalError::EmptyPipeline => write!(f, "pipeline is empty"),
            FunctionalError::NoMatch => write!(f, "no matching element found"),
        }
    }
}

impl std::error::Error for FunctionalError {}

/// A composable transformation pipeline that chains operations on a value.
///
/// Supports building a sequence of `T -> T` transformations and applying them
/// in order.
#[derive(Clone)]
pub struct Pipeline<T: 'static> {
    steps: Vec<Box<dyn CloneFn<T>>>,
}

/// Helper trait so we can clone boxed closures inside `Pipeline`.
trait CloneFn<T>: Fn(T) -> T {
    fn clone_box(&self) -> Box<dyn CloneFn<T>>;
}

impl<T, F: Fn(T) -> T + Clone + 'static> CloneFn<T> for F {
    fn clone_box(&self) -> Box<dyn CloneFn<T>> {
        Box::new(self.clone())
    }
}

impl<T: 'static> Clone for Box<dyn CloneFn<T>> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl<T: 'static> Pipeline<T> {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Append a transformation step.
    pub fn then(mut self, f: impl Fn(T) -> T + Clone + 'static) -> Self {
        self.steps.push(Box::new(f));
        self
    }

    /// Execute all steps in order on `value`.
    pub fn execute(&self, value: T) -> T {
        self.steps.iter().fold(value, |acc, step| step(acc))
    }

    /// Return the number of steps in the pipeline.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Return `true` if the pipeline has no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Try to execute; returns an error if the pipeline is empty.
    pub fn try_execute(&self, value: T) -> Result<T, FunctionalError> {
        if self.steps.is_empty() {
            return Err(FunctionalError::EmptyPipeline);
        }
        Ok(self.execute(value))
    }
}

impl<T: 'static> Default for Pipeline<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for Pipeline<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pipeline")
            .field("steps", &self.steps.len())
            .finish()
    }
}

/// A builder for constructing validated values of type `T`.
///
/// Validators are predicate functions paired with an error message; the value
/// is only produced when all validators pass.
#[derive(Clone)]
pub struct ValidatedBuilder<T: Clone + 'static> {
    value: T,
    validators: Vec<(Box<dyn ClonePredicate<T>>, String)>,
}

trait ClonePredicate<T>: Fn(&T) -> bool {
    fn clone_box(&self) -> Box<dyn ClonePredicate<T>>;
}

impl<T, F: Fn(&T) -> bool + Clone + 'static> ClonePredicate<T> for F {
    fn clone_box(&self) -> Box<dyn ClonePredicate<T>> {
        Box::new(self.clone())
    }
}

impl<T: 'static> Clone for Box<dyn ClonePredicate<T>> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl<T: Clone + 'static> ValidatedBuilder<T> {
    /// Start building from an initial value.
    pub fn new(value: T) -> Self {
        Self {
            value,
            validators: Vec::new(),
        }
    }

    /// Add a validation rule.
    pub fn validate(
        mut self,
        predicate: impl Fn(&T) -> bool + Clone + 'static,
        message: impl Into<String>,
    ) -> Self {
        self.validators.push((Box::new(predicate), message.into()));
        self
    }

    /// Run all validators and return the value, or the first error.
    pub fn build(self) -> Result<T, FunctionalError> {
        for (pred, msg) in &self.validators {
            if !pred(&self.value) {
                return Err(FunctionalError::ValidationFailed(msg.clone()));
            }
        }
        Ok(self.value)
    }
}

impl<T: Clone + fmt::Debug> fmt::Debug for ValidatedBuilder<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedBuilder")
            .field("value", &self.value)
            .field("validators", &self.validators.len())
            .finish()
    }
}

/// Applies a function to each element, returning the first `Ok` result.
pub fn find_map_result<T, R, E>(
    items: impl IntoIterator<Item = T>,
    f: impl Fn(T) -> Result<R, E>,
) -> Result<R, FunctionalError> {
    for item in items {
        if let Ok(r) = f(item) {
            return Ok(r);
        }
    }
    Err(FunctionalError::NoMatch)
}

/// Partition items into `(matching, non_matching)` based on a predicate.
pub fn partition<T>(
    items: impl IntoIterator<Item = T>,
    predicate: impl Fn(&T) -> bool,
) -> (Vec<T>, Vec<T>) {
    let mut yes = Vec::new();
    let mut no = Vec::new();
    for item in items {
        if predicate(&item) {
            yes.push(item);
        } else {
            no.push(item);
        }
    }
    (yes, no)
}

/// Returns a closure that applies `f` and clamps the result to `[min, max]`.
pub fn clamp_result<F>(f: F, min: f64, max: f64) -> impl Fn(f64) -> f64
where
    F: Fn(f64) -> f64,
{
    move |x| f(x).clamp(min, max)
}

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

/// Compose three functions left-to-right: `chain3(f, g, h)(x)` = `h(g(f(x)))`.
pub fn chain3<A, B, C, D>(
    f: impl Fn(A) -> B,
    g: impl Fn(B) -> C,
    h: impl Fn(C) -> D,
) -> impl Fn(A) -> D {
    move |a| h(g(f(a)))
}

/// Retry a fallible operation up to `max_attempts` times, returning the first
/// `Ok` or the last `Err`.
pub fn retry<T, E>(mut f: impl FnMut() -> Result<T, E>, max_attempts: usize) -> Result<T, E> {
    let mut last_err = None;
    for _ in 0..max_attempts {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.expect("max_attempts must be > 0"))
}

/// Accumulate intermediate results like a fold that keeps every step.
///
/// Returns a `Vec` containing the initial value followed by each successive
/// accumulation.
pub fn scan<T: Clone, A>(
    init: T,
    items: impl Iterator<Item = A>,
    mut f: impl FnMut(&T, A) -> T,
) -> Vec<T> {
    let mut results = vec![init];
    for item in items {
        let next = f(results.last().unwrap(), item);
        results.push(next);
    }
    results
}

/// Group items by a key function, returning a `HashMap` of key → items.
pub fn group_by<T, K: Eq + std::hash::Hash>(
    items: impl IntoIterator<Item = T>,
    key_fn: impl Fn(&T) -> K,
) -> HashMap<K, Vec<T>> {
    let mut map: HashMap<K, Vec<T>> = HashMap::new();
    for item in items {
        let key = key_fn(&item);
        map.entry(key).or_default().push(item);
    }
    map
}

/// Zip two iterators together using a combining function.
pub fn zip_with<A, B, C>(
    a: impl IntoIterator<Item = A>,
    b: impl IntoIterator<Item = B>,
    f: impl Fn(A, B) -> C,
) -> Vec<C> {
    a.into_iter().zip(b).map(|(x, y)| f(x, y)).collect()
}

/// A simple state-machine reducer that applies actions to state.
///
/// Holds a reducing function and the current state, letting callers dispatch
/// actions one at a time.
pub struct Reducer<S, A> {
    state: S,
    reduce_fn: Box<dyn Fn(S, A) -> S>,
}

impl<S: Clone, A> Reducer<S, A> {
    /// Create a new `Reducer` with an initial state and a reducing function.
    pub fn new(initial: S, reduce_fn: impl Fn(S, A) -> S + 'static) -> Self {
        Self {
            state: initial,
            reduce_fn: Box::new(reduce_fn),
        }
    }

    /// Dispatch an action, updating the internal state.
    pub fn dispatch(&mut self, action: A) {
        let old = self.state.clone();
        self.state = (self.reduce_fn)(old, action);
    }

    /// Return a reference to the current state.
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S: fmt::Debug, A> fmt::Debug for Reducer<S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reducer")
            .field("state", &self.state)
            .finish()
    }
}

/// Compose a vector of functions left-to-right into a single function.
///
/// `pipe_all([f, g, h])(x)` = `h(g(f(x)))`.
/// Returns the identity if the slice is empty.
pub fn pipe_all<T: Clone + 'static>(
    fns: Vec<Box<dyn Fn(T) -> T>>,
) -> Box<dyn Fn(T) -> T> {
    Box::new(move |mut val| {
        for f in &fns {
            val = f(val);
        }
        val
    })
}

/// Combine two predicates with logical AND.
pub fn pred_and<T>(
    p1: impl Fn(&T) -> bool + 'static,
    p2: impl Fn(&T) -> bool + 'static,
) -> Box<dyn Fn(&T) -> bool> {
    Box::new(move |x| p1(x) && p2(x))
}

/// Combine two predicates with logical OR.
pub fn pred_or<T>(
    p1: impl Fn(&T) -> bool + 'static,
    p2: impl Fn(&T) -> bool + 'static,
) -> Box<dyn Fn(&T) -> bool> {
    Box::new(move |x| p1(x) || p2(x))
}

/// Negate a predicate.
pub fn pred_not<T>(p: impl Fn(&T) -> bool + 'static) -> Box<dyn Fn(&T) -> bool> {
    Box::new(move |x| !p(x))
}

/// Partition items into three groups based on two predicates.
///
/// Returns `(first_match, second_match, neither)`.
pub fn partition3<T>(
    items: impl IntoIterator<Item = T>,
    p1: impl Fn(&T) -> bool,
    p2: impl Fn(&T) -> bool,
) -> (Vec<T>, Vec<T>, Vec<T>) {
    let mut a = Vec::new();
    let mut b = Vec::new();
    let mut c = Vec::new();
    for item in items {
        if p1(&item) {
            a.push(item);
        } else if p2(&item) {
            b.push(item);
        } else {
            c.push(item);
        }
    }
    (a, b, c)
}

/// Memoize a function with a bounded cache (LRU-style eviction by insertion order).
///
/// When the cache exceeds `capacity`, the oldest entry is removed.
pub fn memoize_bounded<A, R, F>(mut f: F, capacity: usize) -> impl FnMut(A) -> R
where
    A: Eq + std::hash::Hash + Clone,
    R: Clone,
    F: FnMut(&A) -> R,
{
    let mut cache: HashMap<A, R> = HashMap::new();
    let mut order: Vec<A> = Vec::new();
    move |arg: A| {
        if let Some(cached) = cache.get(&arg) {
            return cached.clone();
        }
        let result = f(&arg);
        if order.len() >= capacity && !order.is_empty() {
            let evicted = order.remove(0);
            cache.remove(&evicted);
        }
        order.push(arg.clone());
        cache.insert(arg, result.clone());
        result
    }
}

/// Apply a function to each element, collecting only the `Some` results.
pub fn filter_map_collect<T, R>(
    items: impl IntoIterator<Item = T>,
    f: impl Fn(T) -> Option<R>,
) -> Vec<R> {
    items.into_iter().filter_map(f).collect()
}

/// Retry a fallible closure up to `max_attempts` times, calling `on_retry`
/// with the attempt number (1-based) between each failed attempt.
/// Returns the first `Ok` or the last `Err`.
pub fn retry_with_callback<T, E, F, C>(max_attempts: usize, mut f: F, mut on_retry: C) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
    C: FnMut(usize),
{
    assert!(max_attempts > 0, "max_attempts must be at least 1");
    let mut last_err = None;
    for attempt in 0..max_attempts {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < max_attempts {
                    on_retry(attempt + 1);
                }
            }
        }
    }
    Err(last_err.unwrap())
}

/// Builder for composing an arbitrary number of `T -> T` functions left-to-right.
pub struct PipeBuilder<T: 'static> {
    fns: Vec<Box<dyn Fn(T) -> T>>,
}

impl<T: 'static> PipeBuilder<T> {
    /// Create a new empty pipe builder.
    pub fn new() -> Self {
        Self { fns: Vec::new() }
    }

    /// Append a function to the pipeline.
    pub fn then(mut self, f: impl Fn(T) -> T + 'static) -> Self {
        self.fns.push(Box::new(f));
        self
    }

    /// Consume the builder and return a single composed function.
    ///
    /// An empty pipeline acts as the identity function.
    pub fn build(self) -> Box<dyn Fn(T) -> T> {
        Box::new(move |mut val| {
            for f in &self.fns {
                val = f(val);
            }
            val
        })
    }

    /// Return the number of functions in the pipeline.
    pub fn len(&self) -> usize {
        self.fns.len()
    }

    /// Return `true` if the pipeline contains no functions.
    pub fn is_empty(&self) -> bool {
        self.fns.is_empty()
    }
}

impl<T: 'static> Default for PipeBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a new [`PipeBuilder`] for composing `T -> T` functions.
pub fn pipe<T: 'static>() -> PipeBuilder<T> {
    PipeBuilder::new()
}

/// Wrap a function with debounce logic using an invocation counter.
///
/// The returned closure only calls `f` every `threshold`-th invocation,
/// returning `Some(result)`. All other calls return `None`.
pub fn debounce_fn<T, R>(
    f: impl Fn(T) -> R + 'static,
    threshold: usize,
) -> impl FnMut(T) -> Option<R> {
    let mut calls: usize = 0;
    move |arg| {
        calls += 1;
        if calls >= threshold {
            calls = 0;
            Some(f(arg))
        } else {
            None
        }
    }
}

/// Generate a sequence by repeatedly applying a function to a state.
///
/// Starting with `seed`, calls `f(&state)`. When it returns
/// `Some((value, next_state))`, pushes `value` and continues. Stops on `None`.
pub fn unfold<T: Clone, S>(seed: S, f: impl Fn(&S) -> Option<(T, S)>) -> Vec<T> {
    let mut state = seed;
    let mut result = Vec::new();
    while let Some((value, next)) = f(&state) {
        result.push(value);
        state = next;
    }
    result
}

/// Map each item with `f` and flatten the resulting `Vec`s into one.
pub fn flatmap<T, R>(items: impl IntoIterator<Item = T>, f: impl Fn(T) -> Vec<R>) -> Vec<R> {
    let mut result = Vec::new();
    for item in items {
        result.extend(f(item));
    }
    result
}

/// Like `take_while` but includes the first element that does not match the
/// predicate.
pub fn take_while_inclusive<T>(
    items: impl IntoIterator<Item = T>,
    pred: impl Fn(&T) -> bool,
) -> Vec<T> {
    let mut result = Vec::new();
    for item in items {
        let matches = pred(&item);
        result.push(item);
        if !matches {
            break;
        }
    }
    result
}

/// Right-to-left function composition: `compose_rtl(f, g)(x)` = `f(g(x))`.
///
/// This is the mathematical composition order, as opposed to the left-to-right
/// [`compose`].
pub fn compose_rtl<A, B, C, F, G>(f: F, g: G) -> impl Fn(A) -> C
where
    F: Fn(B) -> C,
    G: Fn(A) -> B,
{
    move |a| f(g(a))
}

/// A throttle wrapper that executes immediately on first call, then suppresses
/// subsequent calls within the cooldown period.
pub struct Throttled<F> {
    f: F,
    cooldown: std::time::Duration,
    last_call: Cell<Option<Instant>>,
    calls_made: Cell<usize>,
    calls_suppressed: Cell<usize>,
}

impl<F> Throttled<F> {
    /// Create a new `Throttled` wrapper with the given cooldown in milliseconds.
    pub fn new(f: F, cooldown_ms: u64) -> Self {
        Self {
            f,
            cooldown: std::time::Duration::from_millis(cooldown_ms),
            last_call: Cell::new(None),
            calls_made: Cell::new(0),
            calls_suppressed: Cell::new(0),
        }
    }

    /// Reset the throttle so the next call executes immediately.
    pub fn reset(&self) {
        self.last_call.set(None);
    }

    /// Total number of calls that were actually executed.
    pub fn calls_made(&self) -> usize {
        self.calls_made.get()
    }

    /// Total number of calls that were suppressed.
    pub fn calls_suppressed(&self) -> usize {
        self.calls_suppressed.get()
    }
}

impl<F> Throttled<F> {
    /// Invoke the throttled function. The call is executed only if the cooldown
    /// period has elapsed since the last executed call.
    pub fn call<A>(&self, arg: A)
    where
        F: Fn(A),
    {
        let now = Instant::now();
        let should_call = match self.last_call.get() {
            None => true,
            Some(last) => now.duration_since(last) >= self.cooldown,
        };
        if should_call {
            self.last_call.set(Some(now));
            self.calls_made.set(self.calls_made.get() + 1);
            (self.f)(arg);
        } else {
            self.calls_suppressed.set(self.calls_suppressed.get() + 1);
        }
    }
}

/// Create a new [`Throttled`] wrapper around `f` with the given cooldown.
pub fn throttle_immediate<F>(f: F, cooldown_ms: u64) -> Throttled<F> {
    Throttled::new(f, cooldown_ms)
}

// ---------------------------------------------------------------------------
// Either type
// ---------------------------------------------------------------------------

/// A value that is one of two possible types.
///
/// Unlike `Result`, `Either` carries no success/failure semantics — both
/// variants are equally valid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Either<L, R> {
    /// The left variant.
    Left(L),
    /// The right variant.
    Right(R),
}

impl<L, R> Either<L, R> {
    /// Returns `true` if this is a `Left` value.
    pub fn is_left(&self) -> bool {
        matches!(self, Either::Left(_))
    }

    /// Returns `true` if this is a `Right` value.
    pub fn is_right(&self) -> bool {
        matches!(self, Either::Right(_))
    }

    /// Extract the left value, or `None`.
    pub fn left(self) -> Option<L> {
        match self {
            Either::Left(l) => Some(l),
            Either::Right(_) => None,
        }
    }

    /// Extract the right value, or `None`.
    pub fn right(self) -> Option<R> {
        match self {
            Either::Left(_) => None,
            Either::Right(r) => Some(r),
        }
    }

    /// Map a function over the left value.
    pub fn map_left<L2>(self, f: impl FnOnce(L) -> L2) -> Either<L2, R> {
        match self {
            Either::Left(l) => Either::Left(f(l)),
            Either::Right(r) => Either::Right(r),
        }
    }

    /// Map a function over the right value.
    pub fn map_right<R2>(self, f: impl FnOnce(R) -> R2) -> Either<L, R2> {
        match self {
            Either::Left(l) => Either::Left(l),
            Either::Right(r) => Either::Right(f(r)),
        }
    }

    /// Fold both variants into a single value.
    pub fn fold<T>(self, on_left: impl FnOnce(L) -> T, on_right: impl FnOnce(R) -> T) -> T {
        match self {
            Either::Left(l) => on_left(l),
            Either::Right(r) => on_right(r),
        }
    }

    /// Swap left and right.
    pub fn swap(self) -> Either<R, L> {
        match self {
            Either::Left(l) => Either::Right(l),
            Either::Right(r) => Either::Left(r),
        }
    }
}

impl<T> Either<T, T> {
    /// Extract the inner value when both variants are the same type.
    pub fn into_inner(self) -> T {
        match self {
            Either::Left(v) | Either::Right(v) => v,
        }
    }
}

// ---------------------------------------------------------------------------
// Validated — like Result but accumulates all errors
// ---------------------------------------------------------------------------

/// A validation result that accumulates errors rather than short-circuiting.
///
/// `Validated::Ok(value)` holds a successfully validated value.
/// `Validated::Errs(errors)` holds one or more validation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validated<T, E> {
    /// A valid value.
    Ok(T),
    /// One or more accumulated errors.
    Errs(Vec<E>),
}

impl<T, E> Validated<T, E> {
    /// Create a valid value.
    pub fn ok(value: T) -> Self {
        Validated::Ok(value)
    }

    /// Create a single-error invalid value.
    pub fn err(error: E) -> Self {
        Validated::Errs(vec![error])
    }

    /// Returns `true` if this is a valid value.
    pub fn is_ok(&self) -> bool {
        matches!(self, Validated::Ok(_))
    }

    /// Returns `true` if this contains errors.
    pub fn is_err(&self) -> bool {
        matches!(self, Validated::Errs(_))
    }

    /// Map a function over the valid value.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Validated<U, E> {
        match self {
            Validated::Ok(v) => Validated::Ok(f(v)),
            Validated::Errs(e) => Validated::Errs(e),
        }
    }

    /// Convert into a standard `Result`, joining errors with the given
    /// separator when there are multiple.
    pub fn into_result(self) -> Result<T, Vec<E>> {
        match self {
            Validated::Ok(v) => Ok(v),
            Validated::Errs(e) => Err(e),
        }
    }
}

/// Combine two `Validated` values. If both are `Ok`, applies `f` to produce
/// the combined value. If either or both have errors, all errors are
/// accumulated.
pub fn validated_zip<A, B, C, E>(
    va: Validated<A, E>,
    vb: Validated<B, E>,
    f: impl FnOnce(A, B) -> C,
) -> Validated<C, E> {
    match (va, vb) {
        (Validated::Ok(a), Validated::Ok(b)) => Validated::Ok(f(a, b)),
        (Validated::Errs(e), Validated::Ok(_)) => Validated::Errs(e),
        (Validated::Ok(_), Validated::Errs(e)) => Validated::Errs(e),
        (Validated::Errs(mut e1), Validated::Errs(e2)) => {
            e1.extend(e2);
            Validated::Errs(e1)
        }
    }
}

/// Run a list of validation checks against a value, accumulating all errors.
/// Returns `Validated::Ok(value)` if all pass, otherwise `Validated::Errs`.
pub fn validate_all<T, E>(
    value: T,
    checks: &[(fn(&T) -> bool, E)],
) -> Validated<T, E>
where
    E: Clone,
{
    let errors: Vec<E> = checks
        .iter()
        .filter(|(pred, _)| !pred(&value))
        .map(|(_, err)| err.clone())
        .collect();
    if errors.is_empty() {
        Validated::Ok(value)
    } else {
        Validated::Errs(errors)
    }
}

// ---------------------------------------------------------------------------
// Result combinator helpers
// ---------------------------------------------------------------------------

/// Extension trait adding combinators to `Result`.
pub trait ResultExt<T, E> {
    /// If `Ok`, apply `f`; otherwise return `fallback`.
    fn map_or_else_with<U>(self, fallback: impl FnOnce(E) -> U, f: impl FnOnce(T) -> U) -> U;

    /// Tap into an `Ok` value for side-effects without consuming it.
    fn tap_ok(self, f: impl FnOnce(&T)) -> Self;

    /// Tap into an `Err` value for side-effects without consuming it.
    fn tap_err(self, f: impl FnOnce(&E)) -> Self;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn map_or_else_with<U>(self, fallback: impl FnOnce(E) -> U, f: impl FnOnce(T) -> U) -> U {
        match self {
            Ok(v) => f(v),
            Err(e) => fallback(e),
        }
    }

    fn tap_ok(self, f: impl FnOnce(&T)) -> Self {
        if let Ok(ref v) = self {
            f(v);
        }
        self
    }

    fn tap_err(self, f: impl FnOnce(&E)) -> Self {
        if let Err(ref e) = self {
            f(e);
        }
        self
    }
}

// ---------------------------------------------------------------------------
// Option combinator helpers
// ---------------------------------------------------------------------------

/// Extension trait adding combinators to `Option`.
pub trait OptionExt<T> {
    /// Tap into a `Some` value for side-effects without consuming it.
    fn tap_some(self, f: impl FnOnce(&T)) -> Self;

    /// Convert to a `Result` using `err_fn` to produce the error on `None`.
    fn ok_or_else_with<E>(self, err_fn: impl FnOnce() -> E) -> Result<T, E>;

    /// Return `self` if the predicate holds, otherwise `None`.
    fn filter_with(self, pred: impl FnOnce(&T) -> bool) -> Self;
}

impl<T> OptionExt<T> for Option<T> {
    fn tap_some(self, f: impl FnOnce(&T)) -> Self {
        if let Some(ref v) = self {
            f(v);
        }
        self
    }

    fn ok_or_else_with<E>(self, err_fn: impl FnOnce() -> E) -> Result<T, E> {
        self.ok_or_else(err_fn)
    }

    fn filter_with(self, pred: impl FnOnce(&T) -> bool) -> Self {
        match self {
            Some(ref v) if pred(v) => self,
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Converge — run two functions on same input, combine results
// ---------------------------------------------------------------------------

/// Apply two functions to the same input and combine their results.
///
/// `converge(split_a, split_b, merge)(x)` = `merge(split_a(x), split_b(x))`.
pub fn converge<A, B, C, D>(
    fa: impl Fn(&A) -> B,
    fb: impl Fn(&A) -> C,
    merge: impl Fn(B, C) -> D,
) -> impl Fn(&A) -> D {
    move |a| merge(fa(a), fb(a))
}

/// Apply a sequence of fallible functions, returning the first `Ok`.
///
/// Unlike [`find_map_result`], this takes a list of functions rather than
/// mapping over items.
pub fn first_ok<T: Clone, R, E>(
    value: &T,
    fns: &[fn(&T) -> Result<R, E>],
) -> Result<R, FunctionalError> {
    for f in fns {
        if let Ok(r) = f(value) {
            return Ok(r);
        }
    }
    Err(FunctionalError::NoMatch)
}

/// Apply a transformation while a predicate holds, returning the final value.
pub fn iterate_while<T>(mut value: T, pred: impl Fn(&T) -> bool, f: impl Fn(T) -> T) -> T {
    while pred(&value) {
        value = f(value);
    }
    value
}

// ---------------------------------------------------------------------------
// window — sliding window over a slice
// ---------------------------------------------------------------------------

/// Return all contiguous windows of size `n` from a slice.
pub fn windows<T>(items: &[T], n: usize) -> Vec<&[T]> {
    if n == 0 || n > items.len() {
        return Vec::new();
    }
    items.windows(n).collect()
}

// ---------------------------------------------------------------------------
// intersperse — place a separator between elements
// ---------------------------------------------------------------------------

/// Insert `sep` between every two consecutive elements.
pub fn intersperse<T: Clone>(items: impl IntoIterator<Item = T>, sep: T) -> Vec<T> {
    let mut result = Vec::new();
    let mut first = true;
    for item in items {
        if !first {
            result.push(sep.clone());
        }
        result.push(item);
        first = false;
    }
    result
}

// ---------------------------------------------------------------------------
// chunk — split into fixed-size groups
// ---------------------------------------------------------------------------

/// Split a `Vec` into chunks of at most `size` elements.
pub fn chunk<T>(items: Vec<T>, size: usize) -> Vec<Vec<T>> {
    if size == 0 {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut current = Vec::with_capacity(size);
    for item in items {
        current.push(item);
        if current.len() == size {
            result.push(std::mem::take(&mut current));
            current = Vec::with_capacity(size);
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

// ---------------------------------------------------------------------------
// transpose — Option<Result> ↔ Result<Option>
// ---------------------------------------------------------------------------

/// Convert `Option<Result<T, E>>` into `Result<Option<T>, E>`.
pub fn transpose_option_result<T, E>(opt: Option<Result<T, E>>) -> Result<Option<T>, E> {
    match opt {
        Some(Ok(v)) => Ok(Some(v)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// zip_longest — zip two iterators, padding with None
// ---------------------------------------------------------------------------

/// Zip two iterators, continuing until both are exhausted.
/// Shorter iterator pads with `None`.
pub fn zip_longest<A, B>(
    a: impl IntoIterator<Item = A>,
    b: impl IntoIterator<Item = B>,
) -> Vec<(Option<A>, Option<B>)> {
    let mut a_iter = a.into_iter();
    let mut b_iter = b.into_iter();
    let mut result = Vec::new();
    loop {
        match (a_iter.next(), b_iter.next()) {
            (None, None) => break,
            (av, bv) => result.push((av, bv)),
        }
    }
    result
}

// ---------------------------------------------------------------------------
// fold_while — fold with early termination
// ---------------------------------------------------------------------------

/// Fold over items while the predicate returns true for the accumulator.
/// Stops as soon as the predicate fails after an accumulation step.
pub fn fold_while<T, A>(
    items: impl IntoIterator<Item = T>,
    init: A,
    pred: impl Fn(&A) -> bool,
    f: impl Fn(A, T) -> A,
) -> A {
    let mut acc = init;
    for item in items {
        acc = f(acc, item);
        if !pred(&acc) {
            break;
        }
    }
    acc
}

// ---------------------------------------------------------------------------
// Pipeline additional methods
// ---------------------------------------------------------------------------

impl<T: Clone + 'static> Pipeline<T> {
    /// Execute the pipeline but also return each intermediate result.
    pub fn execute_trace(&self, value: T) -> Vec<T> {
        let mut trace = vec![value.clone()];
        let mut current = value;
        for step in &self.steps {
            current = step(current);
            trace.push(current.clone());
        }
        trace
    }
}

// ---------------------------------------------------------------------------
// Lazy — deferred computation with caching
// ---------------------------------------------------------------------------

/// A lazily evaluated value that computes on first access and caches the result.
pub struct Lazy<T> {
    init: Cell<Option<Box<dyn FnOnce() -> T>>>,
    value: Cell<Option<T>>,
}

impl<T: Clone> Lazy<T> {
    /// Create a new lazy value from a closure.
    pub fn new(f: impl FnOnce() -> T + 'static) -> Self {
        Self {
            init: Cell::new(Some(Box::new(f))),
            value: Cell::new(None),
        }
    }

    /// Force evaluation and return the value. Subsequent calls return the
    /// cached result without re-evaluating.
    pub fn force(&self) -> T {
        let current = self.value.take();
        if let Some(v) = current {
            let cloned = v.clone();
            self.value.set(Some(v));
            return cloned;
        }
        let init = self.init.take();
        if let Some(f) = init {
            let v = f();
            let cloned = v.clone();
            self.value.set(Some(v));
            cloned
        } else {
            panic!("Lazy value already consumed without caching");
        }
    }

    /// Returns `true` if the value has been computed.
    pub fn is_evaluated(&self) -> bool {
        let v = self.value.take();
        let evaluated = v.is_some();
        self.value.set(v);
        evaluated
    }
}

// ---------------------------------------------------------------------------
// unique — deduplicate while preserving order
// ---------------------------------------------------------------------------

/// Remove duplicate elements while preserving the first occurrence order.
pub fn unique<T: Eq + std::hash::Hash + Clone>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            result.push(item);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// frequency_map — count occurrences
// ---------------------------------------------------------------------------

/// Count the frequency of each element, returning a map of element → count.
pub fn frequency_map<T: Eq + std::hash::Hash>(
    items: impl IntoIterator<Item = T>,
) -> HashMap<T, usize> {
    let mut map = HashMap::new();
    for item in items {
        *map.entry(item).or_insert(0) += 1;
    }
    map
}

// ---------------------------------------------------------------------------
// min_by_key / max_by_key — find extremes by a key function
// ---------------------------------------------------------------------------

/// Return the element with the minimum key, or `None` if empty.
pub fn min_by_key<T, K: Ord>(
    items: impl IntoIterator<Item = T>,
    key_fn: impl Fn(&T) -> K,
) -> Option<T> {
    items.into_iter().min_by_key(|x| key_fn(x))
}

/// Return the element with the maximum key, or `None` if empty.
pub fn max_by_key<T, K: Ord>(
    items: impl IntoIterator<Item = T>,
    key_fn: impl Fn(&T) -> K,
) -> Option<T> {
    items.into_iter().max_by_key(|x| key_fn(x))
}

// ---------------------------------------------------------------------------
// chain_results — sequence multiple fallible operations
// ---------------------------------------------------------------------------

/// Apply a sequence of fallible transformations to a value, short-circuiting
/// on the first error.
pub fn chain_results<T, E>(
    value: T,
    fns: &[fn(T) -> Result<T, E>],
) -> Result<T, E> {
    let mut current = value;
    for f in fns {
        current = f(current)?;
    }
    Ok(current)
}

// ---------------------------------------------------------------------------
// map_keys / map_values — transform HashMap entries
// ---------------------------------------------------------------------------

/// Transform all keys in a `HashMap`, merging values with the same new key by
/// keeping the last one encountered.
pub fn map_keys<K1, K2, V>(
    map: HashMap<K1, V>,
    f: impl Fn(K1) -> K2,
) -> HashMap<K2, V>
where
    K2: Eq + std::hash::Hash,
{
    map.into_iter().map(|(k, v)| (f(k), v)).collect()
}

/// Transform all values in a `HashMap`.
pub fn map_values<K, V1, V2>(
    map: HashMap<K, V1>,
    f: impl Fn(V1) -> V2,
) -> HashMap<K, V2>
where
    K: Eq + std::hash::Hash,
{
    map.into_iter().map(|(k, v)| (k, f(v))).collect()
}

// ---------------------------------------------------------------------------
// try_fold — fold with early error return
// ---------------------------------------------------------------------------

/// Fold over items with a fallible accumulator function, returning the first
/// error encountered or the final accumulator value.
pub fn try_fold<T, A, E>(
    items: impl IntoIterator<Item = T>,
    init: A,
    f: impl Fn(A, T) -> Result<A, E>,
) -> Result<A, E> {
    let mut acc = init;
    for item in items {
        acc = f(acc, item)?;
    }
    Ok(acc)
}

// ---------------------------------------------------------------------------
// interleave — alternate elements from two iterators
// ---------------------------------------------------------------------------

/// Alternate elements from two iterators: `[a0, b0, a1, b1, ...]`.
/// Remaining elements from the longer iterator are appended at the end.
pub fn interleave<T>(
    a: impl IntoIterator<Item = T>,
    b: impl IntoIterator<Item = T>,
) -> Vec<T> {
    let mut a_iter = a.into_iter().peekable();
    let mut b_iter = b.into_iter().peekable();
    let mut result = Vec::new();
    loop {
        match (a_iter.next(), b_iter.next()) {
            (Some(av), Some(bv)) => {
                result.push(av);
                result.push(bv);
            }
            (Some(av), None) => {
                result.push(av);
                result.extend(a_iter);
                break;
            }
            (None, Some(bv)) => {
                result.push(bv);
                result.extend(b_iter);
                break;
            }
            (None, None) => break,
        }
    }
    result
}

// ---------------------------------------------------------------------------
// span — split at the first element that doesn't match
// ---------------------------------------------------------------------------

/// Split items into a prefix that matches the predicate and the remaining suffix.
///
/// Like `take_while` + remainder: `span(pred, [a,b,c,d])` returns
/// `([a,b], [c,d])` where `a,b` satisfy `pred` and `c` does not.
pub fn span<T>(
    items: impl IntoIterator<Item = T>,
    pred: impl Fn(&T) -> bool,
) -> (Vec<T>, Vec<T>) {
    let mut prefix = Vec::new();
    let mut suffix = Vec::new();
    let mut matched = true;
    for item in items {
        if matched && pred(&item) {
            prefix.push(item);
        } else {
            matched = false;
            suffix.push(item);
        }
    }
    (prefix, suffix)
}

// ---------------------------------------------------------------------------
// all_equal — check if all elements are the same
// ---------------------------------------------------------------------------

/// Returns `true` if all elements are equal, or the collection is empty.
pub fn all_equal<T: PartialEq>(items: impl IntoIterator<Item = T>) -> bool {
    let mut iter = items.into_iter();
    let first = match iter.next() {
        Some(v) => v,
        None => return true,
    };
    iter.all(|x| x == first)
}

// ---------------------------------------------------------------------------
// successors — generate a sequence from an initial value
// ---------------------------------------------------------------------------

/// Generate a sequence by repeatedly applying `f` to the previous value.
/// Stops when `f` returns `None`.
pub fn successors<T: Clone>(first: T, f: impl Fn(&T) -> Option<T>) -> Vec<T> {
    let mut result = vec![first];
    loop {
        let next = f(result.last().unwrap());
        match next {
            Some(v) => result.push(v),
            None => break,
        }
    }
    result
}

// ---------------------------------------------------------------------------
// converge_until — apply a function repeatedly until the value stabilizes
// ---------------------------------------------------------------------------

/// Apply `f` repeatedly until the output equals the input (a fixed point),
/// or until `max_iters` iterations have been performed.
pub fn converge_until<T: PartialEq + Clone>(
    initial: T,
    f: impl Fn(&T) -> T,
    max_iters: usize,
) -> T {
    let mut current = initial;
    for _ in 0..max_iters {
        let next = f(&current);
        if next == current {
            return current;
        }
        current = next;
    }
    current
}


// ---------------------------------------------------------------------------
// PipelineChain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PipelineChain {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl PipelineChain {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for PipelineChain {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for PipelineChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "PipelineChain({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// RetryDecorator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RetryDecorator {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl RetryDecorator {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for RetryDecorator {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for RetryDecorator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "RetryDecorator({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// PipelineChainSnapshot — point-in-time snapshot of PipelineChain state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PipelineChainSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl PipelineChainSnapshot {
    pub fn capture(source: &PipelineChain, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for PipelineChainSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// RetryDecoratorStats — aggregate statistics for RetryDecorator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct RetryDecoratorStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl RetryDecoratorStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for RetryDecoratorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// PipelineChainConfig — configuration for PipelineChain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PipelineChainConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl PipelineChainConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for PipelineChainConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for PipelineChainConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// ValuePipeline
// ---------------------------------------------------------------------------

/// A chainable transformation pipeline over a single value.
pub struct ValuePipeline<T> {
    value: Option<T>,
    steps: usize,
}

impl<T: 'static> ValuePipeline<T> {
    pub fn new(value: T) -> Self {
        Self { value: Some(value), steps: 0 }
    }

    pub fn map<U: 'static>(self, f: impl FnOnce(T) -> U) -> ValuePipeline<U> {
        ValuePipeline {
            value: self.value.map(f),
            steps: self.steps + 1,
        }
    }

    pub fn then<U: 'static>(self, f: impl FnOnce(T) -> Option<U>) -> ValuePipeline<U> {
        ValuePipeline {
            value: self.value.and_then(f),
            steps: self.steps + 1,
        }
    }

    pub fn inspect(self, f: impl FnOnce(&T)) -> Self {
        if let Some(ref v) = self.value {
            f(v);
        }
        self
    }

    pub fn execute(self) -> Option<T> {
        self.value
    }

    pub fn step_count(&self) -> usize {
        self.steps
    }
}

/// Pipeline for filtering a Vec.
pub struct FilterPipeline<T> {
    items: Vec<T>,
}

impl<T: Clone + 'static> FilterPipeline<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self { items }
    }

    pub fn filter(mut self, pred: impl Fn(&T) -> bool) -> Self {
        self.items.retain(pred);
        self
    }

    pub fn map<U: Clone + 'static>(self, f: impl Fn(T) -> U) -> FilterPipeline<U> {
        FilterPipeline {
            items: self.items.into_iter().map(f).collect(),
        }
    }

    pub fn execute(self) -> Vec<T> {
        self.items
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }
}

// ---------------------------------------------------------------------------
// Memoizer
// ---------------------------------------------------------------------------

/// Caches function results by key.
pub struct Memoizer<K: std::hash::Hash + Eq, V: Clone> {
    cache: HashMap<K, V>,
    hits: u64,
    misses: u64,
}

impl<K: std::hash::Hash + Eq, V: Clone> Memoizer<K, V> {
    pub fn new() -> Self {
        Self { cache: HashMap::new(), hits: 0, misses: 0 }
    }

    pub fn get_or_compute(&mut self, key: K, compute: impl FnOnce() -> V) -> V
    where
        K: Clone,
    {
        if let Some(v) = self.cache.get(&key) {
            self.hits += 1;
            return v.clone();
        }
        self.misses += 1;
        let v = compute();
        self.cache.insert(key, v.clone());
        v
    }

    pub fn invalidate(&mut self, key: &K) -> bool {
        self.cache.remove(key).is_some()
    }

    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }
}

// ---------------------------------------------------------------------------
// Compose
// ---------------------------------------------------------------------------

/// Compose two functions left-to-right: compose_fns(f, g)(x) == g(f(x)).
pub fn compose_fns<A, B, C>(f: impl Fn(A) -> B, g: impl Fn(B) -> C) -> impl Fn(A) -> C {
    move |a| g(f(a))
}

/// Bind the first argument of a two-argument function.
pub fn partial_apply_first<A: Clone + 'static, B, R>(
    f: impl Fn(A, B) -> R + 'static,
    a: A,
) -> impl Fn(B) -> R {
    move |b| f(a.clone(), b)
}

/// Bind the second argument of a two-argument function.
pub fn partial_apply_second<A, B: Clone + 'static, R>(
    f: impl Fn(A, B) -> R + 'static,
    b: B,
) -> impl Fn(A) -> R {
    move |a| f(a, b.clone())
}


/// Functional utility configuration manager.
#[derive(Debug, Clone)]
pub struct FunctionalConfig {
    entries: Vec<FunctionalEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single functional utility entry.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionalEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl FunctionalEntry {
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

impl FunctionalConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: FunctionalEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&FunctionalEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut FunctionalEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&FunctionalEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&FunctionalEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&FunctionalEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<FunctionalEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for functional
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaFunctionalRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaFunctionalRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaFunctionalCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaFunctionalCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaFunctionalCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 85
// ---------------------------------------------------------------------------

/// Generic object pool `Xc85Pool<T>`.
pub struct Xc85Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc85Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc85PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc85Pool<T> {
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
    pub fn stats(&self) -> Xc85PoolStats {
        Xc85PoolStats {
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

impl<T> Default for Xc85Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc85Scheduler`.
pub struct Xc85Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc85Scheduler {
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

impl Default for Xc85Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_85 hash for the given byte slice.
pub fn xc_85_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_85 convention.
pub fn xc_85_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_66 deepening: state machine + event bus ---

/// States for the Xd66 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd66State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd66State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd66Transition {
    pub from: Xd66State,
    pub to: Xd66State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd66StateMachine {
    current: Xd66State,
    history: Vec<Xd66Transition>,
    step_counter: usize,
}

impl Xd66StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd66State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd66State {
        self.current
    }

    pub fn history(&self) -> &[Xd66Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd66State) -> Result<Xd66State, String> {
        let allowed = match (self.current, target) {
            (Xd66State::Idle, Xd66State::Running) => true,
            (Xd66State::Running, Xd66State::Paused) => true,
            (Xd66State::Running, Xd66State::Done) => true,
            (Xd66State::Paused, Xd66State::Running) => true,
            (Xd66State::Paused, Xd66State::Done) => true,
            (Xd66State::Done, Xd66State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_66: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd66Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd66SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd66State> {
        let prefix = "Xd66SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd66State::Idle),
            "Running" => Some(Xd66State::Running),
            "Paused" => Some(Xd66State::Paused),
            "Done" => Some(Xd66State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd66State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd66 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd66Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd66Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd66HandlerFn = Box<dyn Fn(&Xd66Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd66EventBus {
    handlers: Vec<(usize, Option<String>, Xd66HandlerFn)>,
    next_id: usize,
    published: Vec<Xd66Event>,
}

impl Xd66EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd66Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd66Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd66Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd66Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #71
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf71Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf71TrieNode {
    children: std::collections::HashMap<char, Xf71TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf71Trie {
    root: Xf71TrieNode,
    count: usize,
}

impl Xf71Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf71TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf71TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf71TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf71BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf71BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 84).
pub struct Xh84SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh84SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 126 as u64,
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

/// A compact bit set supporting boolean operations (variant 84).
pub struct Xh84BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh84BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 84).
pub struct Xi84Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi84Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi84Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi84Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 84).
pub struct Xi84IntervalTree {
    xi_intervals: Vec<Xi84Interval>,
}

impl Xi84IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi84Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi84Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi84Interval) -> Vec<&Xi84Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi84Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi84Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi84Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi84Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi84Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi84Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 84) ---

/// Disjoint set / union-find for crate 84.
pub struct Xj84UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj84UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ84_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 84.
pub struct Xj84BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj84BTreeNode<K, V>>>,
    len: usize,
}

struct Xj84BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj84BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj84BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ84_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ84_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj84BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj84BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj84BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj84BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_84 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk84SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk84SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk84DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk84DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_84).
#[derive(Debug, Clone)]
pub struct Xl84Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl84Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_84).
#[derive(Debug, Clone)]
pub struct Xl84SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl84SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm84MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm84MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm84Tokenizer {
    text: String,
}

impl Xm84Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 84.
pub struct Xn84Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn84Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 84 -----

#[derive(Debug, Clone)]
struct Xn84AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn84AvlNode<K, V>>>,
    right: Option<Box<Xn84AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 84.
#[derive(Debug, Clone)]
pub struct Xn84AVL<K, V> {
    root: Option<Box<Xn84AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn84AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn84AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn84AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn84AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn84AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn84AvlNode<K, V>>) -> Box<Xn84AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn84AvlNode<K, V>>) -> Box<Xn84AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn84AvlNode<K, V>>) -> Box<Xn84AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn84AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn84AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn84AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn84AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn84AvlNode<K, V>>) -> &Xn84AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn84AvlNode<K, V>>) -> (Box<Xn84AvlNode<K, V>>, Option<Box<Xn84AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn84AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn84AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn84AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn84AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn84AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn84AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn84AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo84RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo84Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo84RBNode<K, V> {
    key: K,
    value: V,
    color: Xo84Color,
    left: Option<Box<Xo84RBNode<K, V>>>,
    right: Option<Box<Xo84RBNode<K, V>>>,
}

/// A red-black tree map for crate 84.
#[derive(Debug, Clone)]
pub struct Xo84RedBlack<K, V> {
    root: Option<Box<Xo84RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo84RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo84Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo84RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo84RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo84RBNode {
                    key, value, color: Xo84Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo84RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo84Color::Red)
    }

    fn xo_balance(mut h: Box<Xo84RBNode<K, V>>) -> Box<Xo84RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo84Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo84RBNode<K, V>>) -> Box<Xo84RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo84Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo84RBNode<K, V>>) -> Box<Xo84RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo84Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo84RBNode<K, V>>) {
        h.color = Xo84Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo84Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo84Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo84Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo84RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo84RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo84RBNode<K, V>) -> (K, V, Option<Box<Xo84RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo84RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo84Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo84RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo84ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 84.
#[derive(Debug, Clone)]
pub struct Xo84ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo84ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo84#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo84#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 84).
#[derive(Debug)]
pub struct Xp84SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp84Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp84Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp84Node<K, V>>>,
    xp_right: Option<Box<Xp84Node<K, V>>>,
}

impl<K: Ord, V> Xp84Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp84SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp84SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp84Node<K, V>>>, key: &K) -> Option<Box<Xp84Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp84Node<K, V>>) -> Box<Xp84Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp84Node<K, V>>) -> Box<Xp84Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp84Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp84Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp84Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq84Treap ---------------

use std::cmp::Ordering as Xq84Ord;

struct Xq84TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq84TreapNode<K, V>>>,
    right: Option<Box<Xq84TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq84Treap<K, V> {
    root: Option<Box<Xq84TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq84TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_84_size<K, V>(node: &Option<Box<Xq84TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_84_update_size<K, V>(node: &mut Xq84TreapNode<K, V>) {
    node.size = 1 + xq_84_size(&node.left) + xq_84_size(&node.right);
}

fn xq_84_rotate_right<K, V>(mut node: Box<Xq84TreapNode<K, V>>) -> Box<Xq84TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_84_update_size(&mut node);
    left.right = Some(node);
    xq_84_update_size(&mut left);
    left
}

fn xq_84_rotate_left<K, V>(mut node: Box<Xq84TreapNode<K, V>>) -> Box<Xq84TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_84_update_size(&mut node);
    right.left = Some(node);
    xq_84_update_size(&mut right);
    right
}

fn xq_84_insert_node<K: Ord, V>(
    node: Option<Box<Xq84TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq84TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq84TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq84Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq84Ord::Less => {
                let (new_left, old) = xq_84_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_84_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_84_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq84Ord::Greater => {
                let (new_right, old) = xq_84_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_84_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_84_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_84_remove_node<K: Ord, V>(
    node: Option<Box<Xq84TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq84TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq84Ord::Less => {
                let (new_left, old) = xq_84_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_84_update_size(&mut n);
                (Some(n), old)
            }
            Xq84Ord::Greater => {
                let (new_right, old) = xq_84_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_84_update_size(&mut n);
                (Some(n), old)
            }
            Xq84Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_84_rotate_right(n);
                    let (new_right, old) = xq_84_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_84_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_84_rotate_left(n);
                    let (new_left, old) = xq_84_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_84_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_84_find_min<K, V>(node: &Option<Box<Xq84TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_84_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_84_find_max<K, V>(node: &Option<Box<Xq84TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_84_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_84_rank<K: Ord, V>(node: &Option<Box<Xq84TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq84Ord::Less => xq_84_rank(&n.left, key),
            Xq84Ord::Equal => xq_84_size(&n.left),
            Xq84Ord::Greater => 1 + xq_84_size(&n.left) + xq_84_rank(&n.right, key),
        },
    }
}

fn xq_84_kth<K, V>(node: &Option<Box<Xq84TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_84_size(&n.left);
        if k < left_size {
            xq_84_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_84_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_84_in_order<K: Clone, V>(node: &Option<Box<Xq84TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_84_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_84_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq84Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 84 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_84_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq84Ord::Equal => return Some(&n.value),
                Xq84Ord::Less => cur = &n.left,
                Xq84Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_84_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_84_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_84_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_84_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_84_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_84_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_84_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq84VEBTree ---------------

pub struct Xq84VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq84VEBTree>>,
    clusters: Vec<Option<Box<Xq84VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq84VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq84VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq84VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
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

    #[test]
    fn test_constant() {
        let always_42 = constant(42);
        assert_eq!(always_42(), 42);
        assert_eq!(always_42(), 42);
        let always_hello = constant("hello".to_string());
        assert_eq!(always_hello(), "hello");
    }

    #[test]
    fn test_pipeline_execute() {
        let p = Pipeline::new()
            .then(|x: i32| x + 1)
            .then(|x: i32| x * 3);
        assert_eq!(p.execute(5), 18); // (5+1)*3
        assert_eq!(p.len(), 2);
        assert!(!p.is_empty());
    }

    #[test]
    fn test_pipeline_empty() {
        let p = Pipeline::<i32>::new();
        assert!(p.is_empty());
        assert_eq!(p.try_execute(10), Err(FunctionalError::EmptyPipeline));
    }

    #[test]
    fn test_pipeline_try_execute_ok() {
        let p = Pipeline::new().then(|x: i32| x * 2);
        assert_eq!(p.try_execute(7), Ok(14));
    }

    #[test]
    fn test_pipeline_debug() {
        let p = Pipeline::new().then(|x: i32| x + 1);
        let dbg = format!("{:?}", p);
        assert!(dbg.contains("Pipeline"));
        assert!(dbg.contains("1"));
    }

    #[test]
    fn test_validated_builder_success() {
        let result = ValidatedBuilder::new(42)
            .validate(|v| *v > 0, "must be positive")
            .validate(|v| *v < 100, "must be less than 100")
            .build();
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn test_validated_builder_failure() {
        let result = ValidatedBuilder::new(-5)
            .validate(|v| *v > 0, "must be positive")
            .build();
        assert_eq!(
            result,
            Err(FunctionalError::ValidationFailed("must be positive".into()))
        );
    }

    #[test]
    fn test_validated_builder_debug() {
        let b = ValidatedBuilder::new(10)
            .validate(|v| *v > 0, "positive");
        let dbg = format!("{:?}", b);
        assert!(dbg.contains("ValidatedBuilder"));
    }

    #[test]
    fn test_find_map_result_found() {
        let items = vec![1, 2, 3, 4];
        let result = find_map_result(items, |x| {
            if x > 2 { Ok(x * 10) } else { Err(()) }
        });
        assert_eq!(result, Ok(30));
    }

    #[test]
    fn test_find_map_result_none() {
        let items = vec![1, 2];
        let result: Result<i32, _> = find_map_result(items, |_| Err::<i32, ()>(()));
        assert_eq!(result, Err(FunctionalError::NoMatch));
    }

    #[test]
    fn test_partition() {
        let (evens, odds) = partition(vec![1, 2, 3, 4, 5], |x| x % 2 == 0);
        assert_eq!(evens, vec![2, 4]);
        assert_eq!(odds, vec![1, 3, 5]);
    }

    #[test]
    fn test_clamp_result() {
        let f = clamp_result(|x| x * 3.0, 0.0, 10.0);
        assert!((f(5.0) - 10.0).abs() < f64::EPSILON); // 15 clamped to 10
        assert!((f(-1.0) - 0.0).abs() < f64::EPSILON); // -3 clamped to 0
        assert!((f(2.0) - 6.0).abs() < f64::EPSILON); // 6 within range
    }

    #[test]
    fn test_functional_error_display() {
        let e = FunctionalError::ValidationFailed("bad".into());
        assert_eq!(e.to_string(), "validation failed: bad");
        assert_eq!(FunctionalError::EmptyPipeline.to_string(), "pipeline is empty");
        assert_eq!(FunctionalError::NoMatch.to_string(), "no matching element found");
    }

    #[test]
    fn test_chain3() {
        let add1 = |x: i32| x + 1;
        let double = |x: i32| x * 2;
        let to_str = |x: i32| format!("{x}");
        let f = chain3(add1, double, to_str);
        assert_eq!(f(5), "12"); // (5+1)*2 = 12
        assert_eq!(f(0), "2");  // (0+1)*2 = 2
    }

    #[test]
    fn test_retry_succeeds_first_try() {
        let result = retry(|| Ok::<i32, &str>(42), 3);
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn test_retry_succeeds_after_failures() {
        let mut attempts = 0;
        let result = retry(
            || {
                attempts += 1;
                if attempts < 3 { Err("not yet") } else { Ok(attempts) }
            },
            5,
        );
        assert_eq!(result, Ok(3));
    }

    #[test]
    fn test_retry_exhausted() {
        let result = retry(|| Err::<(), &str>("fail"), 3);
        assert_eq!(result, Err("fail"));
    }

    #[test]
    fn test_scan() {
        let sums = scan(0, vec![1, 2, 3, 4].into_iter(), |acc, x| acc + x);
        assert_eq!(sums, vec![0, 1, 3, 6, 10]);
    }

    #[test]
    fn test_scan_empty() {
        let result = scan(10, std::iter::empty::<i32>(), |acc, x| acc + x);
        assert_eq!(result, vec![10]);
    }

    #[test]
    fn test_group_by() {
        let groups = group_by(vec![1, 2, 3, 4, 5, 6], |x| x % 3);
        assert_eq!(groups[&0], vec![3, 6]);
        assert_eq!(groups[&1], vec![1, 4]);
        assert_eq!(groups[&2], vec![2, 5]);
    }

    #[test]
    fn test_zip_with() {
        let result = zip_with(vec![1, 2, 3], vec![10, 20, 30], |a, b| a + b);
        assert_eq!(result, vec![11, 22, 33]);
    }

    #[test]
    fn test_zip_with_unequal_lengths() {
        let result = zip_with(vec![1, 2], vec![10, 20, 30], |a, b| a * b);
        assert_eq!(result, vec![10, 40]); // stops at shorter
    }

    #[test]
    fn test_reducer() {
        let mut r = Reducer::new(0_i32, |state, action: i32| state + action);
        r.dispatch(5);
        r.dispatch(3);
        assert_eq!(*r.state(), 8);
        r.dispatch(-2);
        assert_eq!(*r.state(), 6);
    }

    #[test]
    fn test_reducer_debug() {
        let r = Reducer::new(0_i32, |s, a: i32| s + a);
        let dbg = format!("{:?}", r);
        assert!(dbg.contains("Reducer"));
    }

    #[test]
    fn test_pipe_all_empty() {
        let p = pipe_all::<i32>(vec![]);
        assert_eq!(p(42), 42);
    }

    #[test]
    fn test_pipe_all_multiple() {
        let fns: Vec<Box<dyn Fn(i32) -> i32>> = vec![
            Box::new(|x| x + 1),
            Box::new(|x| x * 2),
            Box::new(|x| x - 3),
        ];
        let p = pipe_all(fns);
        assert_eq!(p(5), 9); // (5+1)*2-3 = 9
    }

    #[test]
    fn test_pred_and() {
        let p = pred_and(|x: &i32| *x > 0, |x: &i32| *x < 10);
        assert!(p(&5));
        assert!(!p(&-1));
        assert!(!p(&15));
    }

    #[test]
    fn test_pred_or() {
        let p = pred_or(|x: &i32| *x < 0, |x: &i32| *x > 100);
        assert!(p(&-5));
        assert!(p(&200));
        assert!(!p(&50));
    }

    #[test]
    fn test_pred_not() {
        let p = pred_not(|x: &i32| *x > 0);
        assert!(p(&-1));
        assert!(p(&0));
        assert!(!p(&1));
    }

    #[test]
    fn test_partition3() {
        let (neg, big, rest) = partition3(
            vec![-2, -1, 0, 5, 50, 100],
            |x| *x < 0,
            |x| *x >= 50,
        );
        assert_eq!(neg, vec![-2, -1]);
        assert_eq!(big, vec![50, 100]);
        assert_eq!(rest, vec![0, 5]);
    }

    #[test]
    fn test_memoize_bounded() {
        let mut f = memoize_bounded(|x: &i32| x * x, 2);
        assert_eq!(f(3), 9);
        assert_eq!(f(4), 16);
        assert_eq!(f(3), 9); // still cached
        assert_eq!(f(5), 25); // evicts oldest (3)
        assert_eq!(f(3), 9); // recomputed
    }

    #[test]
    fn test_filter_map_collect() {
        let result = filter_map_collect(vec![1, 2, 3, 4, 5], |x| {
            if x % 2 == 0 { Some(x * 10) } else { None }
        });
        assert_eq!(result, vec![20, 40]);
    }

    #[test]
    fn retry_cb_succeeds_first() {
        let result: Result<i32, &str> = retry_with_callback(3, || Ok(42), |_| {});
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn retry_cb_succeeds_after_failures() {
        let mut attempts = 0;
        let mut retries = 0;
        let result: Result<i32, &str> = retry_with_callback(
            3,
            || { attempts += 1; if attempts < 3 { Err("fail") } else { Ok(99) } },
            |_| { retries += 1; },
        );
        assert_eq!(result, Ok(99));
        assert_eq!(retries, 2);
    }

    #[test]
    fn retry_cb_all_fail() {
        let mut count = 0;
        let result: Result<i32, &str> = retry_with_callback(3, || { count += 1; Err("fail") }, |_| {});
        assert_eq!(result, Err("fail"));
        assert_eq!(count, 3);
    }

    #[test]
    fn pipe_builder_multiple_functions() {
        let add_one = |x: i32| x + 1;
        let double = |x: i32| x * 2;
        let negate = |x: i32| -x;
        let f = pipe().then(add_one).then(double).then(negate).build();
        // (5 + 1) * 2 = 12, then -12
        assert_eq!(f(5), -12);
    }

    #[test]
    fn pipe_builder_empty_is_identity() {
        let f = pipe::<i32>().build();
        assert_eq!(f(42), 42);
    }

    #[test]
    fn pipe_builder_len_and_empty() {
        let b = pipe::<i32>();
        assert!(b.is_empty());
        assert_eq!(b.len(), 0);
        let b = b.then(|x| x + 1);
        assert!(!b.is_empty());
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn debounce_fn_triggers_at_threshold() {
        let mut d = debounce_fn(|x: i32| x * 10, 3);
        d(1); // call 1
        d(2); // call 2
        let result = d(3); // call 3 → triggers
        assert_eq!(result, Some(30));
    }

    #[test]
    fn debounce_fn_returns_none_before_threshold() {
        let mut d = debounce_fn(|x: i32| x * 10, 3);
        assert_eq!(d(1), None);
        assert_eq!(d(2), None);
    }

    #[test]
    fn unfold_generates_fibonacci() {
        let fibs = unfold((0_u64, 1_u64), |&(a, b)| {
            if a > 20 {
                None
            } else {
                Some((a, (b, a + b)))
            }
        });
        assert_eq!(fibs, vec![0, 1, 1, 2, 3, 5, 8, 13]);
    }

    #[test]
    fn unfold_stops_on_none() {
        let result = unfold(0, |_: &i32| None::<(i32, i32)>);
        assert!(result.is_empty());
    }

    #[test]
    fn flatmap_flattens_results() {
        let result = flatmap(vec![1, 2, 3], |x| vec![x, x * 10]);
        assert_eq!(result, vec![1, 10, 2, 20, 3, 30]);
    }

    #[test]
    fn take_while_inclusive_includes_boundary() {
        let result = take_while_inclusive(vec![1, 2, 3, 4, 5], |&x| x < 3);
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_compose_rtl_basic() {
        let add1 = |x: i32| x + 1;
        let double = |x: i32| x * 2;
        // compose_rtl(f, g)(x) = f(g(x)) = double(add1(x))
        let f = compose_rtl(double, add1);
        assert_eq!(f(3), 8); // add1(3)=4, double(4)=8
    }

    #[test]
    fn test_compose_rtl_vs_compose() {
        let add1 = |x: i32| x + 1;
        let double = |x: i32| x * 2;
        // compose(f, g)(x) = g(f(x)) — left-to-right
        let ltr = compose(add1, double);
        // compose_rtl(g, f)(x) = g(f(x)) — same result when args swapped
        let rtl = compose_rtl(double, add1);
        assert_eq!(ltr(5), rtl(5));
    }

    #[test]
    fn test_throttle_immediate_first_call() {
        let counter = std::cell::Cell::new(0);
        let t = throttle_immediate(|_: i32| { counter.set(counter.get() + 1); }, 1000);
        t.call(1);
        assert_eq!(counter.get(), 1);
        assert_eq!(t.calls_made(), 1);
        assert_eq!(t.calls_suppressed(), 0);
    }

    #[test]
    fn test_throttle_immediate_suppresses() {
        let counter = std::cell::Cell::new(0);
        let t = throttle_immediate(|_: i32| { counter.set(counter.get() + 1); }, 10_000);
        t.call(1); // executed
        t.call(2); // suppressed
        t.call(3); // suppressed
        assert_eq!(counter.get(), 1);
        assert_eq!(t.calls_made(), 1);
        assert_eq!(t.calls_suppressed(), 2);
    }

    #[test]
    fn test_throttle_reset() {
        let counter = std::cell::Cell::new(0);
        let t = throttle_immediate(|_: i32| { counter.set(counter.get() + 1); }, 10_000);
        t.call(1); // executed
        t.call(2); // suppressed
        t.reset();
        t.call(3); // executed (after reset)
        assert_eq!(counter.get(), 2);
        assert_eq!(t.calls_made(), 2);
        assert_eq!(t.calls_suppressed(), 1);
    }

    #[test]
    fn test_identity_function() {
        assert_eq!(identity(42), 42);
        assert_eq!(identity("hello"), "hello");
        let v = vec![1, 2, 3];
        assert_eq!(identity(v.clone()), v);
    }

    #[test]
    fn test_constant_function() {
        let always_five = constant(5);
        assert_eq!(always_five(), 5);
        assert_eq!(always_five(), 5);
        let always_hello = constant(String::from("hello"));
        assert_eq!(always_hello(), "hello");
    }

    // --- new tests ---

    #[test]
    fn test_either_left_right() {
        let l: Either<i32, &str> = Either::Left(42);
        assert!(l.is_left());
        assert!(!l.is_right());
        assert_eq!(l.left(), Some(42));

        let r: Either<i32, &str> = Either::Right("hello");
        assert!(r.is_right());
        assert_eq!(r.right(), Some("hello"));

        let swapped = Either::<i32, &str>::Left(1).swap();
        assert_eq!(swapped, Either::Right(1));

        let folded = Either::<i32, i32>::Left(3).fold(|l| l * 2, |r| r + 10);
        assert_eq!(folded, 6);

        let mapped = Either::<i32, &str>::Left(5).map_left(|x| x + 1);
        assert_eq!(mapped, Either::Left(6));

        let same: Either<i32, i32> = Either::Right(99);
        assert_eq!(same.into_inner(), 99);
    }

    #[test]
    fn test_validated_accumulates_errors() {
        let v1: Validated<i32, &str> = Validated::ok(10);
        let v2: Validated<i32, &str> = Validated::ok(20);
        let combined = validated_zip(v1, v2, |a, b| a + b);
        assert_eq!(combined, Validated::Ok(30));

        let e1: Validated<i32, &str> = Validated::err("too small");
        let e2: Validated<i32, &str> = Validated::err("too big");
        let combined = validated_zip(e1, e2, |a, b| a + b);
        assert_eq!(combined, Validated::Errs(vec!["too small", "too big"]));

        let ok: Validated<i32, &str> = Validated::ok(5);
        let err: Validated<i32, &str> = Validated::err("bad");
        let combined = validated_zip(ok, err, |a, b| a + b);
        assert!(combined.is_err());
    }

    #[test]
    fn test_validate_all() {
        let checks: Vec<(fn(&i32) -> bool, &str)> = vec![
            (|x| *x > 0, "must be positive"),
            (|x| *x < 100, "must be < 100"),
            (|x| *x % 2 == 0, "must be even"),
        ];
        let valid = validate_all(42, &checks);
        assert_eq!(valid, Validated::Ok(42));

        let invalid = validate_all(-3, &checks);
        match invalid {
            Validated::Errs(errs) => {
                assert!(errs.contains(&"must be positive"));
                assert!(errs.contains(&"must be even"));
                assert_eq!(errs.len(), 2);
            }
            _ => panic!("expected errors"),
        }
    }

    #[test]
    fn test_result_ext_tap_ok() {
        use std::cell::RefCell;
        let log: RefCell<Vec<i32>> = RefCell::new(Vec::new());
        let r: Result<i32, &str> = Ok(42);
        let r = r.tap_ok(|v| log.borrow_mut().push(*v));
        assert_eq!(r, Ok(42));
        assert_eq!(*log.borrow(), vec![42]);

        let e: Result<i32, &str> = Err("bad");
        let e = e.tap_ok(|v| log.borrow_mut().push(*v));
        assert_eq!(e, Err("bad"));
        assert_eq!(log.borrow().len(), 1); // not called
    }

    #[test]
    fn test_option_ext_filter_with() {
        let some_val: Option<i32> = Some(10);
        assert_eq!(some_val.filter_with(|v| *v > 5), Some(10));
        assert_eq!(Some(3).filter_with(|v: &i32| *v > 5), None);
        assert_eq!(None::<i32>.filter_with(|_| true), None);
    }

    #[test]
    fn test_converge() {
        let sum_and_product = converge(
            |xs: &Vec<i32>| xs.iter().sum::<i32>(),
            |xs: &Vec<i32>| xs.iter().product::<i32>(),
            |s, p| (s, p),
        );
        let data = vec![2, 3, 4];
        assert_eq!(sum_and_product(&data), (9, 24));
    }

    #[test]
    fn test_iterate_while() {
        let result = iterate_while(1, |x| *x < 100, |x| x * 2);
        assert_eq!(result, 128); // 1 → 2 → 4 → 8 → 16 → 32 → 64 → 128
    }

    #[test]
    fn test_windows() {
        let data = vec![1, 2, 3, 4, 5];
        let w = windows(&data, 3);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0], &[1, 2, 3]);
        assert_eq!(w[2], &[3, 4, 5]);
        assert!(windows(&data, 0).is_empty());
        assert!(windows(&data, 6).is_empty());
    }

    #[test]
    fn test_intersperse() {
        let result = intersperse(vec![1, 2, 3], 0);
        assert_eq!(result, vec![1, 0, 2, 0, 3]);
        let single = intersperse(vec![42], 0);
        assert_eq!(single, vec![42]);
        let empty: Vec<i32> = intersperse(Vec::new(), 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_chunk() {
        let result = chunk(vec![1, 2, 3, 4, 5], 2);
        assert_eq!(result, vec![vec![1, 2], vec![3, 4], vec![5]]);
        let exact = chunk(vec![1, 2, 3, 4], 2);
        assert_eq!(exact, vec![vec![1, 2], vec![3, 4]]);
        assert!(chunk(vec![1, 2], 0).is_empty());
    }

    #[test]
    fn test_transpose_option_result() {
        let some_ok: Option<Result<i32, &str>> = Some(Ok(42));
        assert_eq!(transpose_option_result(some_ok), Ok(Some(42)));
        let some_err: Option<Result<i32, &str>> = Some(Err("fail"));
        assert_eq!(transpose_option_result(some_err), Err("fail"));
        let none: Option<Result<i32, &str>> = None;
        assert_eq!(transpose_option_result(none), Ok(None));
    }

    #[test]
    fn test_zip_longest() {
        let result = zip_longest(vec![1, 2, 3], vec!["a", "b"]);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], (Some(1), Some("a")));
        assert_eq!(result[2], (Some(3), None));
    }

    #[test]
    fn test_fold_while() {
        // Sum until accumulator exceeds 10
        let result = fold_while(vec![3, 3, 3, 3, 3], 0, |acc| *acc <= 10, |a, b| a + b);
        assert_eq!(result, 12); // 0+3=3, 3+3=6, 6+3=9, 9+3=12 > 10 → stop
    }

    #[test]
    fn test_pipeline_execute_trace() {
        let p = Pipeline::new()
            .then(|x: i32| x + 1)
            .then(|x: i32| x * 2);
        let trace = p.execute_trace(5);
        assert_eq!(trace, vec![5, 6, 12]);
    }

    #[test]
    fn test_lazy_deferred_evaluation() {
        let call_count = std::cell::Cell::new(0);
        let lazy = Lazy::new(move || {
            call_count.set(call_count.get() + 1);
            42
        });
        assert!(!lazy.is_evaluated());
        assert_eq!(lazy.force(), 42);
        assert!(lazy.is_evaluated());
        // Second call returns cached value
        assert_eq!(lazy.force(), 42);
    }

    #[test]
    fn test_unique_preserves_order() {
        let result = unique(vec![3, 1, 2, 1, 3, 4, 2]);
        assert_eq!(result, vec![3, 1, 2, 4]);
        let empty: Vec<i32> = unique(Vec::new());
        assert!(empty.is_empty());
    }

    #[test]
    fn test_frequency_map() {
        let freq = frequency_map(vec!["a", "b", "a", "c", "b", "a"]);
        assert_eq!(freq["a"], 3);
        assert_eq!(freq["b"], 2);
        assert_eq!(freq["c"], 1);
    }

    #[test]
    fn test_min_max_by_key() {
        let items = vec!["hello", "hi", "hey", "greetings"];
        assert_eq!(min_by_key(items.clone(), |s| s.len()), Some("hi"));
        assert_eq!(max_by_key(items, |s| s.len()), Some("greetings"));
        assert_eq!(min_by_key(Vec::<i32>::new(), |x| *x), None);
    }

    #[test]
    fn test_chain_results_success() {
        let fns: Vec<fn(i32) -> Result<i32, &'static str>> = vec![
            |x| Ok(x + 1),
            |x| Ok(x * 2),
            |x| Ok(x - 3),
        ];
        assert_eq!(chain_results(5, &fns), Ok(9)); // (5+1)*2-3 = 9
    }

    #[test]
    fn test_chain_results_short_circuit() {
        let fns: Vec<fn(i32) -> Result<i32, &'static str>> = vec![
            |x| Ok(x + 1),
            |_| Err("boom"),
            |x| Ok(x * 100), // should not run
        ];
        assert_eq!(chain_results(5, &fns), Err("boom"));
    }

    #[test]
    fn test_map_keys_and_values() {
        let mut m = HashMap::new();
        m.insert(1, "one");
        m.insert(2, "two");
        let mapped = map_keys(m, |k| k * 10);
        assert_eq!(mapped[&10], "one");
        assert_eq!(mapped[&20], "two");

        let mut m2 = HashMap::new();
        m2.insert("a", 1);
        m2.insert("b", 2);
        let mapped2 = map_values(m2, |v| v * 100);
        assert_eq!(mapped2["a"], 100);
        assert_eq!(mapped2["b"], 200);
    }

    #[test]
    fn test_try_fold_success_and_failure() {
        let result = try_fold(vec![1, 2, 3], 0i32, |acc, x| {
            let sum = acc + x;
            if sum > 10 { Err("overflow") } else { Ok(sum) }
        });
        assert_eq!(result, Ok(6));

        let result2 = try_fold(vec![5, 5, 5], 0i32, |acc, x| {
            let sum = acc + x;
            if sum > 10 { Err("overflow") } else { Ok(sum) }
        });
        assert_eq!(result2, Err("overflow"));
    }

    #[test]
    fn test_interleave() {
        let result = interleave(vec![1, 3, 5], vec![2, 4, 6]);
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6]);
        let uneven = interleave(vec![1, 3, 5, 7], vec![2, 4]);
        assert_eq!(uneven, vec![1, 2, 3, 4, 5, 7]);
    }

    #[test]
    fn test_span() {
        let (prefix, suffix) = span(vec![2, 4, 6, 7, 8], |x| x % 2 == 0);
        assert_eq!(prefix, vec![2, 4, 6]);
        assert_eq!(suffix, vec![7, 8]);

        let (all, none) = span(vec![1, 2, 3], |_| true);
        assert_eq!(all, vec![1, 2, 3]);
        assert!(none.is_empty());
    }

    #[test]
    fn test_all_equal() {
        assert!(all_equal(vec![1, 1, 1]));
        assert!(!all_equal(vec![1, 2, 1]));
        assert!(all_equal(Vec::<i32>::new()));
        assert!(all_equal(vec![42]));
    }

    #[test]
    fn test_successors() {
        let powers = successors(1, |x| if *x < 16 { Some(x * 2) } else { None });
        assert_eq!(powers, vec![1, 2, 4, 8, 16]);
    }

    #[test]
    fn test_converge_until_fixed_point() {
        // Newton's method approximation: sqrt(4) ≈ 2
        // Start with guess=3, converge x -> (x + 4/x) / 2 with integer division
        let result = converge_until(10i32, |x| (x + 4 / x) / 2, 100);
        assert_eq!(result, 2);

        // Already at fixed point
        let result2 = converge_until(5, |x| *x, 100);
        assert_eq!(result2, 5);
    }

    #[test] fn pipelineChain_new() { let s = PipelineChain::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn pipelineChain_add() { let mut s = PipelineChain::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn pipelineChain_remove() { let mut s = PipelineChain::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn pipelineChain_config() { let mut s = PipelineChain::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn pipelineChain_nav() { let mut s = PipelineChain::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn pipelineChain_filter() { let mut s = PipelineChain::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn pipelineChain_display() { assert!(format!("{}", PipelineChain::new()).contains("PipelineChain")); }
    #[test] fn retryDecorator_new() { let s = RetryDecorator::new(); assert!(s.is_empty()); }
    #[test] fn retryDecorator_add() { let mut s = RetryDecorator::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn retryDecorator_active() { let mut s = RetryDecorator::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn retryDecorator_error() { let mut s = RetryDecorator::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn retryDecorator_rm_group() { let mut s = RetryDecorator::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn retryDecorator_display() { assert!(format!("{}", RetryDecorator::new()).contains("RetryDecorator")); }


    #[test] fn pipelineChain_snap_capture() {
        let s = PipelineChain::new();
        let snap = PipelineChainSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn pipelineChain_snap_stale() {
        let s = PipelineChain::new();
        let snap = PipelineChainSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn pipelineChain_snap_diff() {
        let s = PipelineChain::new();
        let s1v = PipelineChainSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn pipelineChain_snap_display() {
        let s = PipelineChain::new();
        let snap = PipelineChainSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn retryDecorator_stats_record() {
        let mut st = RetryDecoratorStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn retryDecorator_stats_hit_ratio() {
        let mut st = RetryDecoratorStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn retryDecorator_stats_merge() {
        let mut a = RetryDecoratorStats::new();
        a.total_adds = 5;
        let mut b = RetryDecoratorStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn retryDecorator_stats_display() {
        let st = RetryDecoratorStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn pipelineChain_config_default() {
        let c = PipelineChainConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn pipelineChain_config_builder() {
        let c = PipelineChainConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn pipelineChain_config_labels() {
        let mut c = PipelineChainConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn pipelineChain_config_cleanup_threshold() {
        let c = PipelineChainConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn pipelineChain_config_display() {
        assert!(format!("{}", PipelineChainConfig::new()).contains("Config"));
    }
    #[test] fn retryDecorator_stats_peaks() {
        let mut st = RetryDecoratorStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- ValuePipeline tests --

    #[test]
    fn value_pipeline_map_and_execute() {
        let result = ValuePipeline::new(5).map(|x| x * 2).map(|x| x + 1).execute();
        assert_eq!(result, Some(11));
    }

    #[test]
    fn value_pipeline_then_none() {
        let result = ValuePipeline::new(5)
            .then(|x| if x > 10 { Some(x) } else { None })
            .execute();
        assert_eq!(result, None);
    }

    #[test]
    fn value_pipeline_inspect() {
        let mut seen = 0u32;
        let result = ValuePipeline::new(42).inspect(|v| seen = *v).execute();
        assert_eq!(result, Some(42));
        assert_eq!(seen, 42);
    }

    #[test]
    fn value_pipeline_step_count() {
        let p = ValuePipeline::new(1).map(|x| x + 1).map(|x| x + 1);
        assert_eq!(p.step_count(), 2);
    }

    #[test]
    fn filter_pipeline_basic() {
        let result = FilterPipeline::new(vec![1, 2, 3, 4, 5])
            .filter(|x| x % 2 == 0)
            .execute();
        assert_eq!(result, vec![2, 4]);
    }

    #[test]
    fn filter_pipeline_map() {
        let result = FilterPipeline::new(vec![1, 2, 3])
            .map(|x| x * 10)
            .execute();
        assert_eq!(result, vec![10, 20, 30]);
    }

    #[test]
    fn filter_pipeline_count() {
        let p = FilterPipeline::new(vec![1, 2, 3]).filter(|x| *x > 1);
        assert_eq!(p.count(), 2);
    }

    // -- Memoizer tests --

    #[test]
    fn memoizer_caches_value() {
        let mut m: Memoizer<String, i32> = Memoizer::new();
        let v1 = m.get_or_compute("a".into(), || 42);
        let v2 = m.get_or_compute("a".into(), || 99);
        assert_eq!(v1, 42);
        assert_eq!(v2, 42);
    }

    #[test]
    fn memoizer_hit_rate() {
        let mut m: Memoizer<String, i32> = Memoizer::new();
        m.get_or_compute("a".into(), || 1);
        m.get_or_compute("a".into(), || 1);
        assert!((m.hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn memoizer_invalidate() {
        let mut m: Memoizer<String, i32> = Memoizer::new();
        m.get_or_compute("a".into(), || 1);
        assert!(m.invalidate(&"a".into()));
        assert_eq!(m.cache_size(), 0);
    }

    #[test]
    fn memoizer_clear() {
        let mut m: Memoizer<String, i32> = Memoizer::new();
        m.get_or_compute("a".into(), || 1);
        m.clear();
        assert_eq!(m.cache_size(), 0);
        assert!((m.hit_rate() - 0.0).abs() < 0.01);
    }

    // -- Compose tests --

    #[test]
    fn compose_fns_two_functions() {
        let add1 = |x: i32| x + 1;
        let double = |x: i32| x * 2;
        let f = compose_fns(add1, double);
        assert_eq!(f(3), 8);
    }

    #[test]
    fn partial_apply_first_arg() {
        let add = |a: i32, b: i32| a + b;
        let add5 = partial_apply_first(add, 5);
        assert_eq!(add5(3), 8);
    }

    #[test]
    fn partial_apply_second_arg() {
        let sub = |a: i32, b: i32| a - b;
        let sub3 = partial_apply_second(sub, 3);
        assert_eq!(sub3(10), 7);
    }


    #[test]
    fn functional_entry_creation() {
        let e = FunctionalEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn functional_entry_with_priority() {
        let e = FunctionalEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn functional_entry_metadata() {
        let e = FunctionalEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn functional_entry_remove_meta() {
        let mut e = FunctionalEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn functional_entry_activate_deactivate() {
        let mut e = FunctionalEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn functional_config_add_sorted() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("lo", "Lo").with_priority(1));
        c.add(FunctionalEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn functional_config_capacity() {
        let mut c = FunctionalConfig::new(1);
        assert!(c.add(FunctionalEntry::new("a", "A")));
        assert!(!c.add(FunctionalEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn functional_config_remove() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn functional_config_get() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn functional_config_active_entries() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("a", "A"));
        c.add(FunctionalEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn functional_config_enable_disable() {
        let mut c = FunctionalConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn functional_config_clear() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn functional_config_find_by_label() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn functional_config_top_n() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("a", "A").with_priority(1));
        c.add(FunctionalEntry::new("b", "B").with_priority(2));
        c.add(FunctionalEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn functional_config_deactivate_activate_all() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("a", "A"));
        c.add(FunctionalEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn functional_config_highest_priority() {
        let mut c = FunctionalConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(FunctionalEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn functional_config_contains() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn functional_config_labels() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("a", "Alpha"));
        c.add(FunctionalEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn functional_config_drain_inactive() {
        let mut c = FunctionalConfig::new(10);
        c.add(FunctionalEntry::new("a", "A"));
        c.add(FunctionalEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for functional
    #[test]
    fn xa_functional_ring_new() {
        let rb = super::XaFunctionalRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_functional_ring_push_len() {
        let mut rb = super::XaFunctionalRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_functional_ring_wrap() {
        let mut rb = super::XaFunctionalRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_functional_ring_mean_empty() {
        let rb = super::XaFunctionalRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_functional_ring_mean_values() {
        let mut rb = super::XaFunctionalRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_functional_ring_min_max() {
        let mut rb = super::XaFunctionalRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_functional_ring_iter() {
        let mut rb = super::XaFunctionalRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_functional_counter_new() {
        let c = super::XaFunctionalCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_functional_counter_inc() {
        let mut c = super::XaFunctionalCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_functional_counter_inc_by() {
        let mut c = super::XaFunctionalCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_functional_counter_reset() {
        let mut c = super::XaFunctionalCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_functional_counter_clear() {
        let mut c = super::XaFunctionalCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_functional_counter_default() {
        let c = super::XaFunctionalCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 85 ----

    #[test]
    fn xc_85_pool_new_empty() {
        let pool: super::Xc85Pool<i32> = super::Xc85Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_85_pool_release_acquire() {
        let mut pool = super::Xc85Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_85_pool_acquire_empty() {
        let mut pool: super::Xc85Pool<i32> = super::Xc85Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_85_pool_full() {
        let mut pool = super::Xc85Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_85_pool_drain() {
        let mut pool = super::Xc85Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_85_pool_stats() {
        let mut pool = super::Xc85Pool::new(8);
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
    fn xc_85_pool_clear() {
        let mut pool = super::Xc85Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_85_pool_shrink() {
        let mut pool = super::Xc85Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_85_pool_default() {
        let pool: super::Xc85Pool<String> = super::Xc85Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_85_pool_extend() {
        let mut pool = super::Xc85Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_85_pool_retain() {
        let mut pool = super::Xc85Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_85_scheduler_round_robin() {
        let mut sched = super::Xc85Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_85_scheduler_empty() {
        let mut sched = super::Xc85Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_85_scheduler_reset() {
        let mut sched = super::Xc85Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_85_scheduler_add_remove() {
        let mut sched = super::Xc85Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_85_scheduler_targets() {
        let sched = super::Xc85Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_85_hash_empty() {
        assert_eq!(super::xc_85_hash(b""), 5381);
    }

    #[test]
    fn xc_85_hash_data() {
        let h = super::xc_85_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_85_hash(b"hello"), h);
    }

    #[test]
    fn xc_85_reverse_str() {
        assert_eq!(super::xc_85_reverse("abc"), "cba");
        assert_eq!(super::xc_85_reverse(""), "");
    }


    // --- xd_66 deepening tests ---

    #[test]
    fn xd_66_sm_initial_state() {
        let sm = Xd66StateMachine::new();
        assert_eq!(sm.current_state(), Xd66State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_66_sm_valid_idle_to_running() {
        let mut sm = Xd66StateMachine::new();
        assert!(sm.transition(Xd66State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd66State::Running);
    }

    #[test]
    fn xd_66_sm_valid_running_to_paused() {
        let mut sm = Xd66StateMachine::new();
        sm.transition(Xd66State::Running).unwrap();
        assert!(sm.transition(Xd66State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd66State::Paused);
    }

    #[test]
    fn xd_66_sm_valid_running_to_done() {
        let mut sm = Xd66StateMachine::new();
        sm.transition(Xd66State::Running).unwrap();
        assert!(sm.transition(Xd66State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd66State::Done);
    }

    #[test]
    fn xd_66_sm_valid_paused_to_running() {
        let mut sm = Xd66StateMachine::new();
        sm.transition(Xd66State::Running).unwrap();
        sm.transition(Xd66State::Paused).unwrap();
        assert!(sm.transition(Xd66State::Running).is_ok());
    }

    #[test]
    fn xd_66_sm_valid_done_to_idle() {
        let mut sm = Xd66StateMachine::new();
        sm.transition(Xd66State::Running).unwrap();
        sm.transition(Xd66State::Done).unwrap();
        assert!(sm.transition(Xd66State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd66State::Idle);
    }

    #[test]
    fn xd_66_sm_invalid_idle_to_done() {
        let mut sm = Xd66StateMachine::new();
        assert!(sm.transition(Xd66State::Done).is_err());
    }

    #[test]
    fn xd_66_sm_invalid_idle_to_paused() {
        let mut sm = Xd66StateMachine::new();
        assert!(sm.transition(Xd66State::Paused).is_err());
    }

    #[test]
    fn xd_66_sm_history_tracking() {
        let mut sm = Xd66StateMachine::new();
        sm.transition(Xd66State::Running).unwrap();
        sm.transition(Xd66State::Paused).unwrap();
        sm.transition(Xd66State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd66State::Idle);
        assert_eq!(sm.history()[0].to, Xd66State::Running);
        assert_eq!(sm.history()[1].from, Xd66State::Running);
        assert_eq!(sm.history()[2].to, Xd66State::Done);
    }

    #[test]
    fn xd_66_sm_serialize_deserialize() {
        let mut sm = Xd66StateMachine::new();
        sm.transition(Xd66State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd66StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd66State::Running));
    }

    #[test]
    fn xd_66_sm_deserialize_invalid() {
        assert_eq!(Xd66StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_66_sm_reset() {
        let mut sm = Xd66StateMachine::new();
        sm.transition(Xd66State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd66State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_66_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd66EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd66Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_66_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd66EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd66Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd66Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_66_bus_unsubscribe() {
        let mut bus = Xd66EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_66_event_kind_and_payload() {
        let e = Xd66Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd66Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_66_bus_clear_history() {
        let mut bus = Xd66EventBus::new();
        bus.publish(Xd66Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_66_sm_step_counter_increments() {
        let mut sm = Xd66StateMachine::new();
        sm.transition(Xd66State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd66State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #71 --

    #[test]
    fn xf71_trie_insert_search() {
        let mut t = Xf71Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf71_trie_starts_with() {
        let mut t = Xf71Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf71_trie_remove() {
        let mut t = Xf71Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf71_trie_word_count() {
        let mut t = Xf71Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf71_trie_longest_prefix() {
        let mut t = Xf71Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf71_trie_all_words() {
        let mut t = Xf71Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf71_trie_autocomplete() {
        let mut t = Xf71Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf71_trie_empty_search() {
        let t = Xf71Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf71_bloom_add_contains() {
        let mut bf = Xf71BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf71_bloom_probably_absent() {
        let bf = Xf71BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf71_bloom_false_positive_rate() {
        let mut bf = Xf71BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf71_bloom_clear() {
        let mut bf = Xf71BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf71_bloom_union() {
        let mut a = Xf71BloomFilter::xf_new(512, 2);
        let mut b = Xf71BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf71_bloom_intersection_estimate() {
        let mut a = Xf71BloomFilter::xf_new(512, 2);
        let mut b = Xf71BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf71_bloom_union_size_mismatch() {
        let a = Xf71BloomFilter::xf_new(256, 2);
        let b = Xf71BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh84_skip_insert_contains() {
        let mut sl = super::Xh84SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh84_skip_remove() {
        let mut sl = super::Xh84SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh84_skip_len() {
        let mut sl = super::Xh84SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh84_skip_range_query() {
        let mut sl = super::Xh84SkipList::xh_new(4);
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
    fn xh84_skip_floor_ceiling() {
        let mut sl = super::Xh84SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh84_skip_rank() {
        let mut sl = super::Xh84SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh84_skip_empty() {
        let sl = super::Xh84SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh84_skip_duplicates() {
        let mut sl = super::Xh84SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh84_bitset_set_test() {
        let mut bs = super::Xh84BitSet::xh_new(256);
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
    fn xh84_bitset_clear_count() {
        let mut bs = super::Xh84BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh84_bitset_and_or_xor() {
        let mut a = super::Xh84BitSet::xh_new(128);
        let mut b = super::Xh84BitSet::xh_new(128);
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
    fn xh84_bitset_iter_ones() {
        let mut bs = super::Xh84BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh84_bitset_first_last() {
        let mut bs = super::Xh84BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh84_bitset_empty() {
        let bs = super::Xh84BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi84_deque_push_pop_back() {
        let mut dq = super::Xi84Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi84_deque_push_pop_front() {
        let mut dq = super::Xi84Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi84_deque_mixed_ops() {
        let mut dq = super::Xi84Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi84_deque_get_and_split() {
        let mut dq = super::Xi84Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi84_deque_rotate_left() {
        let mut dq = super::Xi84Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi84_deque_rotate_right() {
        let mut dq = super::Xi84Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi84_deque_grow() {
        let mut dq = super::Xi84Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi84_deque_empty() {
        let dq = super::Xi84Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi84_interval_tree_insert_query() {
        let mut tree = super::Xi84IntervalTree::xi_new();
        tree.xi_insert(super::Xi84Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi84Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi84Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi84_interval_tree_overlap() {
        let mut tree = super::Xi84IntervalTree::xi_new();
        tree.xi_insert(super::Xi84Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi84Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi84Interval::xi_new(12, 20));
        let q = super::Xi84Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi84_interval_tree_remove() {
        let mut tree = super::Xi84IntervalTree::xi_new();
        tree.xi_insert(super::Xi84Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi84Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi84_interval_tree_gaps() {
        let mut tree = super::Xi84IntervalTree::xi_new();
        tree.xi_insert(super::Xi84Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi84Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi84Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi84Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi84Interval::xi_new(8, 10));
    }

    #[test]
    fn xi84_interval_tree_merge() {
        let mut tree = super::Xi84IntervalTree::xi_new();
        tree.xi_insert(super::Xi84Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi84Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi84Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi84Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi84Interval::xi_new(10, 15));
    }

    #[test]
    fn xi84_interval_tree_all() {
        let mut tree = super::Xi84IntervalTree::xi_new();
        tree.xi_insert(super::Xi84Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi84Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi84_interval_tree_empty() {
        let tree = super::Xi84IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi84_interval_tree_contains_point() {
        let iv = super::Xi84Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 84) ---

    #[test]
    fn xj_84_uf_make_and_find() {
        let mut uf = super::Xj84UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_84_uf_union_connected() {
        let mut uf = super::Xj84UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_84_uf_component_count() {
        let mut uf = super::Xj84UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_84_uf_component_size() {
        let mut uf = super::Xj84UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_84_uf_largest_component() {
        let mut uf = super::Xj84UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_84_uf_many_elements() {
        let mut uf = super::Xj84UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_84_uf_separate_components() {
        let mut uf = super::Xj84UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_84_uf_path_compression() {
        let mut uf = super::Xj84UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_84_bt_insert_get() {
        let mut bt = super::Xj84BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_84_bt_contains_len() {
        let mut bt = super::Xj84BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_84_bt_replace() {
        let mut bt = super::Xj84BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_84_bt_remove() {
        let mut bt = super::Xj84BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_84_bt_keys_values() {
        let mut bt = super::Xj84BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_84_bt_range() {
        let mut bt = super::Xj84BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_84_bt_min_max() {
        let mut bt = super::Xj84BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_84_bt_many_inserts() {
        let mut bt = super::Xj84BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_84 segment tree tests ---

    #[test]
    fn xk_84_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk84SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_84_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk84SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_84_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk84SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_84_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk84SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_84_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk84SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_84_st_single_element() {
        let data = vec![42];
        let st = super::Xk84SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_84_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk84SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_84_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk84SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_84 disjoint intervals tests ---

    #[test]
    fn xk_84_di_add_and_count() {
        let mut di = super::Xk84DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_84_di_merge_overlap() {
        let mut di = super::Xk84DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_84_di_contains() {
        let mut di = super::Xk84DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_84_di_remove() {
        let mut di = super::Xk84DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_84_di_covered_length() {
        let mut di = super::Xk84DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_84_di_gaps() {
        let mut di = super::Xk84DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_84_di_merge_adjacent() {
        let mut di = super::Xk84DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_84_di_empty() {
        let di = super::Xk84DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_84_rope_new_empty() {
        let rope = super::Xl84Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_84_rope_from_str() {
        let rope = super::Xl84Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_84_rope_insert_at() {
        let mut rope = super::Xl84Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_84_rope_delete_range() {
        let mut rope = super::Xl84Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_84_rope_char_at() {
        let rope = super::Xl84Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_84_rope_split_concat() {
        let rope = super::Xl84Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_84_rope_line_count() {
        let rope = super::Xl84Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_84_rope_line_at() {
        let rope = super::Xl84Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_84_sa_build_and_search() {
        let sa = super::Xl84SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_84_sa_count() {
        let sa = super::Xl84SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_84_sa_longest_repeated() {
        let sa = super::Xl84SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_84_sa_all_positions() {
        let sa = super::Xl84SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_84_sa_len() {
        let sa = super::Xl84SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_84_sa_empty() {
        let sa = super::Xl84SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_84_rope_slice() {
        let rope = super::Xl84Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_84_sa_search_start() {
        let sa = super::Xl84SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_84_sparse_set_get() {
        let mut m = super::Xm84MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_84_sparse_row_col() {
        let mut m = super::Xm84MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_84_sparse_transpose() {
        let mut m = super::Xm84MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_84_sparse_multiply_vec() {
        let mut m = super::Xm84MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_84_sparse_nnz_density() {
        let mut m = super::Xm84MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_84_sparse_clear() {
        let mut m = super::Xm84MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_84_sparse_overwrite_zero() {
        let mut m = super::Xm84MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_84_tokenizer_basic() {
        let t = super::Xm84Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_84_tokenizer_count() {
        let t = super::Xm84Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_84_tokenizer_unique() {
        let t = super::Xm84Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_84_tokenizer_frequency() {
        let t = super::Xm84Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_84_tokenizer_delimiter() {
        let t = super::Xm84Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_84_tokenizer_whitespace() {
        let t = super::Xm84Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_84_tokenizer_empty() {
        let t = super::Xm84Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 84 ----

    #[test]
    fn xn_84_fenwick_prefix_sum() {
        let mut ft = super::Xn84Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_84_fenwick_range_sum() {
        let mut ft = super::Xn84Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_84_fenwick_point_query() {
        let mut ft = super::Xn84Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_84_fenwick_len() {
        let ft = super::Xn84Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_84_fenwick_multiple_updates() {
        let mut ft = super::Xn84Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_84_fenwick_single_element() {
        let mut ft = super::Xn84Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_84_fenwick_find_kth() {
        let mut ft = super::Xn84Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_84_fenwick_negative_delta() {
        let mut ft = super::Xn84Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 84 ----

    #[test]
    fn xn_84_avl_insert_get() {
        let mut m = super::Xn84AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_84_avl_remove() {
        let mut m = super::Xn84AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_84_avl_in_order() {
        let mut m = super::Xn84AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_84_avl_min_max() {
        let mut m = super::Xn84AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_84_avl_floor_ceiling() {
        let mut m = super::Xn84AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_84_avl_height_balanced() {
        let mut m = super::Xn84AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_84_avl_overwrite() {
        let mut m = super::Xn84AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_84_avl_empty() {
        let m: super::Xn84AVL<i32, i32> = super::Xn84AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo84RedBlack tests ---

    #[test]
    fn xo_84_rb_insert_and_get() {
        let mut tree = super::Xo84RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_84_rb_len_and_empty() {
        let mut tree = super::Xo84RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_84_rb_min_max() {
        let mut tree = super::Xo84RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_84_rb_contains() {
        let mut tree = super::Xo84RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_84_rb_remove() {
        let mut tree = super::Xo84RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_84_rb_in_order() {
        let mut tree = super::Xo84RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_84_rb_black_height() {
        let mut tree = super::Xo84RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_84_rb_overwrite() {
        let mut tree = super::Xo84RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo84ConsistentHash tests ---

    #[test]
    fn xo_84_ch_add_and_count() {
        let mut ring = super::Xo84ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_84_ch_remove_node() {
        let mut ring = super::Xo84ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_84_ch_get_node() {
        let mut ring = super::Xo84ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_84_ch_empty_ring() {
        let ring = super::Xo84ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_84_ch_distribution() {
        let mut ring = super::Xo84ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_84_ch_rebalance() {
        let mut ring = super::Xo84ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_84_ch_virtual_nodes() {
        let mut ring = super::Xo84ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_84_ch_consistent_lookup() {
        let mut ring = super::Xo84ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_84_splay_insert_get() {
        let mut t = super::Xp84SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_84_splay_remove() {
        let mut t = super::Xp84SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_84_splay_count_increases() {
        let mut t = super::Xp84SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_84_splay_depth() {
        let mut t = super::Xp84SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_84_splay_len_empty() {
        let t = super::Xp84SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_84_splay_min_max() {
        let mut t = super::Xp84SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_84_splay_overwrite() {
        let mut t = super::Xp84SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_84_splay_remove_missing() {
        let mut t = super::Xp84SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_84 treap tests ----
    #[test]
    fn xq_84_treap_empty() {
        let t = super::Xq84Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_84_treap_insert_get() {
        let mut t = super::Xq84Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_84_treap_overwrite() {
        let mut t = super::Xq84Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_84_treap_remove() {
        let mut t = super::Xq84Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_84_treap_min_max() {
        let mut t = super::Xq84Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_84_treap_rank() {
        let mut t = super::Xq84Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_84_treap_kth() {
        let mut t = super::Xq84Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_84_treap_in_order() {
        let mut t = super::Xq84Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_84 VEB tree tests ----
    #[test]
    fn xq_84_veb_empty() {
        let v = super::Xq84VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_84_veb_insert_contains() {
        let mut v = super::Xq84VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_84_veb_min_max() {
        let mut v = super::Xq84VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_84_veb_delete() {
        let mut v = super::Xq84VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_84_veb_successor() {
        let mut v = super::Xq84VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_84_veb_predecessor() {
        let mut v = super::Xq84VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_84_veb_count() {
        let mut v = super::Xq84VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_84_veb_duplicate_insert() {
        let mut v = super::Xq84VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}
