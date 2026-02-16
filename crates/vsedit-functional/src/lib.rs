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
}
