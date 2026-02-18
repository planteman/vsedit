//! Specialized data structures for vsedit.
//!
//! Provides collection types equivalent to VS Code's `vs/base/common/collections.ts`,
//! `linkedList.ts`, and `map.ts`.
//!
//! # Key types
//!
//! - [`LinkedList`] — doubly linked list with O(1) push/remove via arena allocation.
//! - [`ResourceMap`] — string-keyed map with optional case-insensitive lookup.
//! - [`SetMap`] — a map of sets.
//! - [`BidirectionalMap`] — maps values in both directions.
//!
//! # Utility functions
//!
//! - [`group_by`] — group items by a key function.
//! - [`diff_sets`] — compute added/removed items between two sets.
//! - [`diff_maps`] — compute added/removed/changed entries between two maps.
//! - [`coalesce`] — merge adjacent matching items.

use std::fmt;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use vsedit_lifecycle::Disposable;

// ---------------------------------------------------------------------------
// NodeId — opaque handle for linked-list nodes
// ---------------------------------------------------------------------------

/// Opaque handle returned by [`LinkedList::push_front`] and [`LinkedList::push_back`].
///
/// Used to remove a node in O(1) via [`LinkedList::remove`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

// ---------------------------------------------------------------------------
// LinkedList<T>
// ---------------------------------------------------------------------------

/// A doubly linked list backed by an arena for O(1) push and remove.
///
/// Nodes are allocated inside an internal `Vec` and linked with indices rather than pointers,
/// keeping the implementation entirely safe.
#[derive(Debug)]
pub struct LinkedList<T> {
    nodes: Vec<Node<T>>,
    head: Option<usize>,
    tail: Option<usize>,
    len: usize,
    /// Indices of freed slots available for reuse.
    free: Vec<usize>,
}

#[derive(Debug)]
struct Node<T> {
    value: Option<T>,
    prev: Option<usize>,
    next: Option<usize>,
    alive: bool,
}

impl<T> LinkedList<T> {
    /// Creates an empty `LinkedList`.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            head: None,
            tail: None,
            len: 0,
            free: Vec::new(),
        }
    }

    /// Pushes a value to the front of the list and returns a [`NodeId`] handle.
    pub fn push_front(&mut self, value: T) -> NodeId {
        let idx = self.alloc_node(value);
        self.nodes[idx].next = self.head;
        if let Some(old_head) = self.head {
            self.nodes[old_head].prev = Some(idx);
        }
        self.head = Some(idx);
        if self.tail.is_none() {
            self.tail = Some(idx);
        }
        self.len += 1;
        NodeId(idx)
    }

    /// Pushes a value to the back of the list and returns a [`NodeId`] handle.
    pub fn push_back(&mut self, value: T) -> NodeId {
        let idx = self.alloc_node(value);
        self.nodes[idx].prev = self.tail;
        if let Some(old_tail) = self.tail {
            self.nodes[old_tail].next = Some(idx);
        }
        self.tail = Some(idx);
        if self.head.is_none() {
            self.head = Some(idx);
        }
        self.len += 1;
        NodeId(idx)
    }

    /// Removes the node identified by `id` and returns its value.
    ///
    /// Returns `None` if the node was already removed.
    pub fn remove(&mut self, id: NodeId) -> Option<T> {
        let idx = id.0;
        if idx >= self.nodes.len() || !self.nodes[idx].alive {
            return None;
        }
        let prev = self.nodes[idx].prev;
        let next = self.nodes[idx].next;
        match prev {
            Some(p) => self.nodes[p].next = next,
            None => self.head = next,
        }
        match next {
            Some(n) => self.nodes[n].prev = prev,
            None => self.tail = prev,
        }
        self.nodes[idx].alive = false;
        self.nodes[idx].prev = None;
        self.nodes[idx].next = None;
        self.len -= 1;
        self.free.push(idx);
        self.nodes[idx].value.take()
    }

    /// Returns the number of live elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the list contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Removes all elements from the list.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.head = None;
        self.tail = None;
        self.len = 0;
        self.free.clear();
    }

    /// Iterates over references to the values in order (front to back).
    pub fn iter(&self) -> LinkedListIter<'_, T> {
        LinkedListIter {
            list: self,
            current: self.head,
        }
    }

    fn alloc_node(&mut self, value: T) -> usize {
        if let Some(idx) = self.free.pop() {
            self.nodes[idx] = Node {
                value: Some(value),
                prev: None,
                next: None,
                alive: true,
            };
            idx
        } else {
            let idx = self.nodes.len();
            self.nodes.push(Node {
                value: Some(value),
                prev: None,
                next: None,
                alive: true,
            });
            idx
        }
    }
}

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over [`LinkedList`] values.
pub struct LinkedListIter<'a, T> {
    list: &'a LinkedList<T>,
    current: Option<usize>,
}

impl<'a, T> Iterator for LinkedListIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let idx = self.current?;
        let node = &self.list.nodes[idx];
        self.current = node.next;
        node.value.as_ref()
    }
}

// ---------------------------------------------------------------------------
// ResourceMap<T>
// ---------------------------------------------------------------------------

/// A string-keyed map with optional case-insensitive lookup.
///
/// Equivalent to VS Code's `ResourceMap` — keys can represent URIs or file paths
/// where case-sensitivity depends on the platform.
#[derive(Debug, Clone)]
pub struct ResourceMap<T> {
    map: HashMap<String, T>,
    ignore_case: bool,
}

impl<T> ResourceMap<T> {
    /// Creates a new `ResourceMap`.
    ///
    /// If `ignore_case` is `true`, all key lookups are normalized to lowercase.
    pub fn new(ignore_case: bool) -> Self {
        Self {
            map: HashMap::new(),
            ignore_case,
        }
    }

    /// Inserts a key-value pair, returning the previous value if the key existed.
    pub fn set(&mut self, key: impl Into<String>, value: T) -> Option<T> {
        self.map.insert(self.normalize(key), value)
    }

    /// Returns a reference to the value for `key`, or `None`.
    pub fn get(&self, key: &str) -> Option<&T> {
        self.map.get(&self.normalize_ref(key))
    }

    /// Returns a mutable reference to the value for `key`, or `None`.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut T> {
        let k = self.normalize_ref(key);
        self.map.get_mut(&k)
    }

    /// Removes a key, returning the value if it existed.
    pub fn delete(&mut self, key: &str) -> Option<T> {
        self.map.remove(&self.normalize_ref(key))
    }

    /// Returns `true` if the map contains `key`.
    pub fn has(&self, key: &str) -> bool {
        self.map.contains_key(&self.normalize_ref(key))
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterates over `(key, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> {
        self.map.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Iterates over keys.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }

    /// Iterates over values.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.map.values()
    }

    fn normalize(&self, key: impl Into<String>) -> String {
        let k = key.into();
        if self.ignore_case { k.to_lowercase() } else { k }
    }

    fn normalize_ref(&self, key: &str) -> String {
        if self.ignore_case {
            key.to_lowercase()
        } else {
            key.to_owned()
        }
    }
}

// ---------------------------------------------------------------------------
// SetMap<K, V>
// ---------------------------------------------------------------------------

/// A map whose values are sets of `V`.
///
/// Useful for one-to-many relationships (e.g. mapping a category to all items in it).
#[derive(Debug, Clone)]
pub struct SetMap<K, V> {
    map: HashMap<K, HashSet<V>>,
}

impl<K, V> SetMap<K, V>
where
    K: Eq + Hash,
    V: Eq + Hash,
{
    /// Creates an empty `SetMap`.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Adds `value` to the set associated with `key`.
    pub fn add(&mut self, key: K, value: V) {
        self.map.entry(key).or_default().insert(value);
    }

    /// Removes `value` from the set associated with `key`.
    ///
    /// If the set becomes empty it is removed from the map entirely.
    pub fn delete(&mut self, key: &K, value: &V) {
        if let Some(set) = self.map.get_mut(key) {
            set.remove(value);
            if set.is_empty() {
                self.map.remove(key);
            }
        }
    }

    /// Returns the set associated with `key`, or `None`.
    pub fn get(&self, key: &K) -> Option<&HashSet<V>> {
        self.map.get(key)
    }

    /// Calls `f` for each value in the set associated with `key`.
    pub fn for_each(&self, key: &K, mut f: impl FnMut(&V)) {
        if let Some(set) = self.map.get(key) {
            for v in set {
                f(v);
            }
        }
    }
}

impl<K, V> Default for SetMap<K, V>
where
    K: Eq + Hash,
    V: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BidirectionalMap<L, R>
// ---------------------------------------------------------------------------

/// A map that allows O(1) lookup in both directions.
///
/// Every `(left, right)` pair is stored in two internal maps so that
/// [`get_by_left`](Self::get_by_left) and [`get_by_right`](Self::get_by_right) are
/// both constant-time.
#[derive(Debug, Clone)]
pub struct BidirectionalMap<L, R> {
    left_to_right: HashMap<L, R>,
    right_to_left: HashMap<R, L>,
}

impl<L, R> BidirectionalMap<L, R>
where
    L: Eq + Hash + Clone,
    R: Eq + Hash + Clone,
{
    /// Creates an empty `BidirectionalMap`.
    pub fn new() -> Self {
        Self {
            left_to_right: HashMap::new(),
            right_to_left: HashMap::new(),
        }
    }

    /// Inserts a bidirectional mapping between `left` and `right`.
    ///
    /// Any previous mappings involving either value are removed first.
    pub fn set(&mut self, left: L, right: R) {
        // Remove any existing mapping for this left value.
        if let Some(old_right) = self.left_to_right.remove(&left) {
            self.right_to_left.remove(&old_right);
        }
        // Remove any existing mapping for this right value.
        if let Some(old_left) = self.right_to_left.remove(&right) {
            self.left_to_right.remove(&old_left);
        }
        self.left_to_right.insert(left.clone(), right.clone());
        self.right_to_left.insert(right, left);
    }

    /// Returns the right value mapped to `left`, if any.
    pub fn get_by_left(&self, left: &L) -> Option<&R> {
        self.left_to_right.get(left)
    }

    /// Returns the left value mapped to `right`, if any.
    pub fn get_by_right(&self, right: &R) -> Option<&L> {
        self.right_to_left.get(right)
    }

    /// Removes the mapping associated with `left`, returning the right value.
    pub fn delete_by_left(&mut self, left: &L) -> Option<R> {
        if let Some(right) = self.left_to_right.remove(left) {
            self.right_to_left.remove(&right);
            Some(right)
        } else {
            None
        }
    }

    /// Removes the mapping associated with `right`, returning the left value.
    pub fn delete_by_right(&mut self, right: &R) -> Option<L> {
        if let Some(left) = self.right_to_left.remove(right) {
            self.left_to_right.remove(&left);
            Some(left)
        } else {
            None
        }
    }

    /// Returns the number of mappings.
    pub fn len(&self) -> usize {
        self.left_to_right.len()
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.left_to_right.is_empty()
    }
}

impl<L, R> Default for BidirectionalMap<L, R>
where
    L: Eq + Hash + Clone,
    R: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LRUCache<K, V>
// ---------------------------------------------------------------------------

/// A least-recently-used cache with configurable capacity.
///
/// When the cache exceeds capacity, the least-recently accessed entry is evicted.
/// Both `get` and `set` count as access.
#[derive(Debug)]
pub struct LruCache<K, V> {
    /// Entries in order from least-recently used (front) to most-recently used (back).
    entries: Vec<(K, V)>,
    capacity: usize,
}

impl<K: Eq + Hash, V> LruCache<K, V> {
    /// Create a new LRU cache with the given capacity (clamped to at least 1).
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    /// Get a reference to the value for `key`, marking it as recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v)
        } else {
            None
        }
    }

    /// Peek at a value without updating its position.
    pub fn peek(&self, key: &K) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Insert or update a key-value pair. Returns the evicted entry if the
    /// cache was at capacity and a new key was inserted.
    pub fn set(&mut self, key: K, value: V) -> Option<(K, V)> {
        // Remove existing entry for this key.
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
        }

        // Evict if at capacity.
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.remove(0))
        } else {
            None
        };

        self.entries.push((key, value));
        evicted
    }

    /// Remove a key from the cache, returning its value if present.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }

    /// Returns the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns `true` if the cache contains `key`.
    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    /// Returns the cache capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return keys in order from least-recently used to most-recently used.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|(k, _)| k)
    }
}

// ---------------------------------------------------------------------------
// PriorityQueue<T> — min-heap
// ---------------------------------------------------------------------------

/// A min-priority queue backed by a binary heap.
///
/// The smallest element (according to `Ord`) is dequeued first.
#[derive(Debug, Clone)]
pub struct PriorityQueue<T> {
    heap: Vec<T>,
}

impl<T: Ord> PriorityQueue<T> {
    /// Create an empty priority queue.
    pub fn new() -> Self {
        Self { heap: Vec::new() }
    }

    /// Push an item onto the queue.
    pub fn push(&mut self, item: T) {
        self.heap.push(item);
        self.sift_up(self.heap.len() - 1);
    }

    /// Remove and return the smallest item, or `None` if empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.heap.is_empty() {
            return None;
        }
        let last = self.heap.len() - 1;
        self.heap.swap(0, last);
        let item = self.heap.pop();
        if !self.heap.is_empty() {
            self.sift_down(0);
        }
        item
    }

    /// Peek at the smallest item without removing it.
    pub fn peek(&self) -> Option<&T> {
        self.heap.first()
    }

    /// Returns the number of items in the queue.
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Clear all items from the queue.
    pub fn clear(&mut self) {
        self.heap.clear();
    }

    /// Drain all items in sorted (ascending) order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.heap.len());
        while let Some(item) = self.pop() {
            result.push(item);
        }
        result
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.heap[idx] < self.heap[parent] {
                self.heap.swap(idx, parent);
                idx = parent;
            } else {
                break;
            }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.heap.len();
        loop {
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            let mut smallest = idx;
            if left < len && self.heap[left] < self.heap[smallest] {
                smallest = left;
            }
            if right < len && self.heap[right] < self.heap[smallest] {
                smallest = right;
            }
            if smallest != idx {
                self.heap.swap(idx, smallest);
                idx = smallest;
            } else {
                break;
            }
        }
    }
}

impl<T: Ord> Default for PriorityQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Groups items by a key produced by `key_fn`.
///
/// # Examples
///
/// ```
/// use vsedit_collections::group_by;
///
/// let words = vec!["apple", "avocado", "banana", "blueberry"];
/// let groups = group_by(words, |w| w.chars().next().unwrap());
/// assert_eq!(groups[&'a'].len(), 2);
/// ```
pub fn group_by<T, K, F>(items: impl IntoIterator<Item = T>, mut key_fn: F) -> HashMap<K, Vec<T>>
where
    K: Eq + Hash,
    F: FnMut(&T) -> K,
{
    let mut map: HashMap<K, Vec<T>> = HashMap::new();
    for item in items {
        let key = key_fn(&item);
        map.entry(key).or_default().push(item);
    }
    map
}

/// Result of [`diff_sets`].
#[derive(Debug, Clone)]
pub struct SetDiff<T> {
    /// Items present in `after` but not in `before`.
    pub added: HashSet<T>,
    /// Items present in `before` but not in `after`.
    pub removed: HashSet<T>,
}

impl<T: Eq + Hash> PartialEq for SetDiff<T> {
    fn eq(&self, other: &Self) -> bool {
        self.added == other.added && self.removed == other.removed
    }
}

impl<T: Eq + Hash> Eq for SetDiff<T> {}

/// Computes the items that were added and removed between two sets.
pub fn diff_sets<T: Eq + Hash + Clone>(before: &HashSet<T>, after: &HashSet<T>) -> SetDiff<T> {
    SetDiff {
        added: after.difference(before).cloned().collect(),
        removed: before.difference(after).cloned().collect(),
    }
}

/// Result of [`diff_maps`].
#[derive(Debug, Clone)]
pub struct MapDiff<K, V> {
    /// Keys present in `after` but not in `before`.
    pub added: HashMap<K, V>,
    /// Keys present in `before` but not in `after`.
    pub removed: HashMap<K, V>,
    /// Keys present in both but with different values (contains the *new* value).
    pub changed: HashMap<K, V>,
}

impl<K: Eq + Hash, V: PartialEq> PartialEq for MapDiff<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.added == other.added
            && self.removed == other.removed
            && self.changed == other.changed
    }
}

impl<K: Eq + Hash, V: Eq> Eq for MapDiff<K, V> {}

/// Computes added, removed, and changed entries between two maps.
pub fn diff_maps<K, V>(before: &HashMap<K, V>, after: &HashMap<K, V>) -> MapDiff<K, V>
where
    K: Eq + Hash + Clone,
    V: Eq + Clone,
{
    let mut added = HashMap::new();
    let mut removed = HashMap::new();
    let mut changed = HashMap::new();

    for (k, v) in after {
        match before.get(k) {
            None => {
                added.insert(k.clone(), v.clone());
            }
            Some(old_v) if old_v != v => {
                changed.insert(k.clone(), v.clone());
            }
            _ => {}
        }
    }
    for (k, v) in before {
        if !after.contains_key(k) {
            removed.insert(k.clone(), v.clone());
        }
    }

    MapDiff {
        added,
        removed,
        changed,
    }
}

/// Merges adjacent items in a sequence when `merge_fn` returns `Some`.
///
/// Iterates left-to-right. For each pair of adjacent items `(a, b)`, `merge_fn(&a, &b)` is
/// called. If it returns `Some(merged)`, the pair is replaced by `merged` and merging
/// continues with the next item. If it returns `None`, `a` is emitted and `b` becomes the
/// new candidate.
pub fn coalesce<T>(
    items: impl IntoIterator<Item = T>,
    merge_fn: impl Fn(&T, &T) -> Option<T>,
) -> Vec<T> {
    let mut result: Vec<T> = Vec::new();
    for item in items {
        if let Some(last) = result.last() {
            if let Some(merged) = merge_fn(last, &item) {
                *result.last_mut().unwrap() = merged;
                continue;
            }
        }
        result.push(item);
    }
    result
}

// ---------------------------------------------------------------------------
// DisposableLinkedList — LinkedList that disposes removed values
// ---------------------------------------------------------------------------

/// A [`LinkedList`] whose values implement [`Disposable`].
///
/// When a node is removed, its value is automatically disposed.
#[derive(Debug)]
pub struct DisposableLinkedList<T: Disposable> {
    inner: LinkedList<T>,
}

impl<T: Disposable> DisposableLinkedList<T> {
    /// Creates an empty `DisposableLinkedList`.
    pub fn new() -> Self {
        Self {
            inner: LinkedList::new(),
        }
    }

    /// Pushes a value to the front.
    pub fn push_front(&mut self, value: T) -> NodeId {
        self.inner.push_front(value)
    }

    /// Pushes a value to the back.
    pub fn push_back(&mut self, value: T) -> NodeId {
        self.inner.push_back(value)
    }

    /// Removes and disposes the value at `id`.
    pub fn remove(&mut self, id: NodeId) {
        if let Some(val) = self.inner.remove(id) {
            val.dispose();
        }
    }

    /// Returns the number of live elements.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clears and disposes all elements.
    pub fn clear(&mut self) {
        // Dispose each live value before clearing.
        for val in self.inner.iter() {
            val.dispose();
        }
        self.inner.clear();
    }

    /// Iterates over references to the values.
    pub fn iter(&self) -> LinkedListIter<'_, T> {
        self.inner.iter()
    }
}

impl<T: Disposable> Default for DisposableLinkedList<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// IntervalMap — range-based lookups
// ---------------------------------------------------------------------------

/// A non-overlapping interval map that maps half-open ranges `[start, end)`
/// to values.
#[derive(Debug, Clone)]
pub struct IntervalMap<V> {
    /// Sorted by start. Intervals must not overlap.
    intervals: Vec<(u64, u64, V)>,
}

impl<V: Clone> IntervalMap<V> {
    /// Create a new empty interval map.
    pub fn new() -> Self {
        Self {
            intervals: Vec::new(),
        }
    }

    /// Insert a half-open interval `[start, end)` with a value.
    /// Returns `false` if the interval overlaps an existing one.
    pub fn insert(&mut self, start: u64, end: u64, value: V) -> bool {
        if start >= end {
            return false;
        }
        // Check for overlap
        for &(s, e, _) in &self.intervals {
            if start < e && end > s {
                return false;
            }
        }
        self.intervals.push((start, end, value));
        self.intervals.sort_by_key(|&(s, _, _)| s);
        true
    }

    /// Query the value at a point. Returns `None` if no interval contains it.
    pub fn query(&self, point: u64) -> Option<&V> {
        for &(s, e, ref v) in &self.intervals {
            if point >= s && point < e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the interval containing the given point.
    /// Returns the removed value if found.
    pub fn remove_at(&mut self, point: u64) -> Option<V> {
        if let Some(idx) = self.intervals.iter().position(|&(s, e, _)| point >= s && point < e) {
            let (_, _, v) = self.intervals.remove(idx);
            Some(v)
        } else {
            None
        }
    }

    /// Number of intervals in the map.
    pub fn len(&self) -> usize {
        self.intervals.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }

    /// Return all intervals as `(start, end, &value)` triples.
    pub fn iter(&self) -> impl Iterator<Item = (u64, u64, &V)> {
        self.intervals.iter().map(|&(s, e, ref v)| (s, e, v))
    }

    /// Clear all intervals.
    pub fn clear(&mut self) {
        self.intervals.clear();
    }

    /// Find all intervals that overlap the query range `[start, end)`.
    pub fn query_range(&self, start: u64, end: u64) -> Vec<(u64, u64, &V)> {
        self.intervals
            .iter()
            .filter(|&&(s, e, _)| start < e && end > s)
            .map(|&(s, e, ref v)| (s, e, v))
            .collect()
    }
}

impl<V: Clone> Default for IntervalMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Trie — prefix searching
// ---------------------------------------------------------------------------

/// A node in the trie.
#[derive(Debug, Clone)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    is_terminal: bool,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            is_terminal: false,
        }
    }
}

/// A trie (prefix tree) for efficient prefix-based string lookups.
#[derive(Debug, Clone)]
pub struct Trie {
    root: TrieNode,
    size: usize,
}

impl Trie {
    /// Create a new empty trie.
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
            size: 0,
        }
    }

    /// Insert a word into the trie.
    pub fn insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_insert_with(TrieNode::new);
        }
        if !node.is_terminal {
            node.is_terminal = true;
            self.size += 1;
        }
    }

    /// Check if a word exists in the trie.
    pub fn contains(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(next) => node = next,
                None => return false,
            }
        }
        node.is_terminal
    }

    /// Check if any word in the trie starts with the given prefix.
    pub fn has_prefix(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(next) => node = next,
                None => return false,
            }
        }
        true
    }

    /// Collect all words that start with the given prefix.
    pub fn words_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(next) => node = next,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        self.collect_words(node, &mut prefix.to_string(), &mut results);
        results
    }

    fn collect_words(&self, node: &TrieNode, current: &mut String, results: &mut Vec<String>) {
        if node.is_terminal {
            results.push(current.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            current.push(ch);
            self.collect_words(&node.children[&ch], current, results);
            current.pop();
        }
    }

    /// Number of words in the trie.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Whether the trie is empty.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

impl Default for Trie {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// sorted_merge — merge two sorted iterators
// ---------------------------------------------------------------------------

/// Merge two sorted iterators into a single sorted iterator.
///
/// Both input iterators must yield elements in ascending order.
/// The resulting iterator yields all elements from both in sorted order.
pub fn sorted_merge<I, J, T>(a: I, b: J) -> SortedMerge<I::IntoIter, J::IntoIter, T>
where
    I: IntoIterator<Item = T>,
    J: IntoIterator<Item = T>,
    T: Ord,
{
    SortedMerge {
        a: a.into_iter().peekable(),
        b: b.into_iter().peekable(),
    }
}

/// Iterator adapter that merges two sorted iterators.
pub struct SortedMerge<A, B, T>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
{
    a: std::iter::Peekable<A>,
    b: std::iter::Peekable<B>,
}

impl<A, B, T> Iterator for SortedMerge<A, B, T>
where
    A: Iterator<Item = T>,
    B: Iterator<Item = T>,
    T: Ord,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match (self.a.peek(), self.b.peek()) {
            (Some(a), Some(b)) => {
                if a <= b {
                    self.a.next()
                } else {
                    self.b.next()
                }
            }
            (Some(_), None) => self.a.next(),
            (None, Some(_)) => self.b.next(),
            (None, None) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// LinkedList — additional methods
// ---------------------------------------------------------------------------

impl<T> LinkedList<T> {
    /// Returns a reference to the front element, or `None` if empty.
    pub fn front(&self) -> Option<&T> {
        self.head.and_then(|idx| self.nodes[idx].value.as_ref())
    }

    /// Returns a reference to the back element, or `None` if empty.
    pub fn back(&self) -> Option<&T> {
        self.tail.and_then(|idx| self.nodes[idx].value.as_ref())
    }

    /// Removes and returns the front element.
    pub fn pop_front(&mut self) -> Option<T> {
        let head = self.head?;
        self.remove(NodeId(head))
    }

    /// Removes and returns the back element.
    pub fn pop_back(&mut self) -> Option<T> {
        let tail = self.tail?;
        self.remove(NodeId(tail))
    }

    /// Collects all values into a `Vec`, consuming the list.
    pub fn into_vec(mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.len);
        while let Some(val) = self.pop_front() {
            result.push(val);
        }
        result
    }

    /// Returns the number of arena slots currently allocated (including freed).
    pub fn arena_capacity(&self) -> usize {
        self.nodes.len()
    }
}

impl<T: PartialEq> LinkedList<T> {
    /// Returns `true` if the list contains the given value.
    pub fn contains(&self, value: &T) -> bool {
        self.iter().any(|v| v == value)
    }

    /// Finds the `NodeId` of the first element equal to `value`.
    pub fn find(&self, value: &T) -> Option<NodeId> {
        let mut idx = self.head;
        while let Some(i) = idx {
            if self.nodes[i].alive {
                if let Some(ref v) = self.nodes[i].value {
                    if v == value {
                        return Some(NodeId(i));
                    }
                }
            }
            idx = self.nodes[i].next;
        }
        None
    }
}

// ---------------------------------------------------------------------------
// SetMap — additional methods
// ---------------------------------------------------------------------------

impl<K, V> SetMap<K, V>
where
    K: Eq + Hash,
    V: Eq + Hash,
{
    /// Returns the total number of values across all keys.
    pub fn total_values(&self) -> usize {
        self.map.values().map(|s| s.len()).sum()
    }

    /// Returns the number of keys in the map.
    pub fn key_count(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the key exists and its set contains `value`.
    pub fn contains(&self, key: &K, value: &V) -> bool {
        self.map.get(key).map_or(false, |s| s.contains(value))
    }

    /// Removes all values for a key, returning the removed set.
    pub fn remove_key(&mut self, key: &K) -> Option<HashSet<V>> {
        self.map.remove(key)
    }
}

// ---------------------------------------------------------------------------
// BidirectionalMap — additional methods
// ---------------------------------------------------------------------------

impl<L, R> BidirectionalMap<L, R>
where
    L: Eq + Hash + Clone,
    R: Eq + Hash + Clone,
{
    /// Clears all mappings.
    pub fn clear(&mut self) {
        self.left_to_right.clear();
        self.right_to_left.clear();
    }

    /// Returns `true` if a mapping exists for the given left value.
    pub fn contains_left(&self, left: &L) -> bool {
        self.left_to_right.contains_key(left)
    }

    /// Returns `true` if a mapping exists for the given right value.
    pub fn contains_right(&self, right: &R) -> bool {
        self.right_to_left.contains_key(right)
    }

    /// Returns an iterator over all `(left, right)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&L, &R)> {
        self.left_to_right.iter()
    }
}

// ---------------------------------------------------------------------------
// OrderedMap – maintains insertion order
// ---------------------------------------------------------------------------

/// A map that maintains insertion order.
///
/// Like [`HashMap`] but iterates in the order keys were first inserted.
#[derive(Debug, Clone)]
pub struct OrderedMap<K: Eq + Hash + Clone, V> {
    map: HashMap<K, V>,
    order: Vec<K>,
}

impl<K: Eq + Hash + Clone, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
        }
    }
}

impl<K: Eq + Hash + Clone, V> OrderedMap<K, V> {
    /// Create an empty ordered map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a key-value pair. If the key exists, updates the value but
    /// preserves insertion order.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let old = self.map.insert(key.clone(), value);
        if old.is_none() {
            self.order.push(key);
        }
        old
    }

    /// Get a reference to the value for a key.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    /// Remove a key, also removing it from the ordering.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let val = self.map.remove(key)?;
        self.order.retain(|k| k != key);
        Some(val)
    }

    /// Iterate in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.order.iter().filter_map(move |k| self.map.get(k).map(|v| (k, v)))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Get keys in insertion order.
    pub fn keys(&self) -> &[K] {
        &self.order
    }

    /// Whether the map contains the key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }
}

// ---------------------------------------------------------------------------
// BiMap – bidirectional lookup
// ---------------------------------------------------------------------------

/// A bidirectional map allowing O(1) lookup in both directions.
///
/// Wraps [`BidirectionalMap`] with a simpler API.
#[derive(Debug, Clone)]
pub struct BiMap<L: Eq + Hash + Clone, R: Eq + Hash + Clone> {
    inner: BidirectionalMap<L, R>,
}

impl<L: Eq + Hash + Clone, R: Eq + Hash + Clone> Default for BiMap<L, R> {
    fn default() -> Self {
        Self {
            inner: BidirectionalMap::new(),
        }
    }
}

impl<L: Eq + Hash + Clone, R: Eq + Hash + Clone> BiMap<L, R> {
    /// Create an empty bimap.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a bidirectional mapping.
    pub fn insert(&mut self, left: L, right: R) {
        self.inner.set(left, right);
    }

    /// Look up by left key.
    pub fn get_left(&self, left: &L) -> Option<&R> {
        self.inner.get_by_left(left)
    }

    /// Look up by right key.
    pub fn get_right(&self, right: &R) -> Option<&L> {
        self.inner.get_by_right(right)
    }

    /// Remove by left key.
    pub fn remove_left(&mut self, left: &L) -> Option<R> {
        self.inner.delete_by_left(left)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// ---------------------------------------------------------------------------
// BoundedStack – stack with capacity limit
// ---------------------------------------------------------------------------

/// A stack with a maximum capacity. When full, the oldest item is dropped.
#[derive(Debug, Clone)]
pub struct BoundedStack<T> {
    items: Vec<T>,
    capacity: usize,
}

impl<T> BoundedStack<T> {
    /// Create a bounded stack with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Push an item. If at capacity, the bottom item is dropped.
    pub fn push(&mut self, item: T) {
        if self.items.len() >= self.capacity {
            self.items.remove(0);
        }
        self.items.push(item);
    }

    /// Pop the top item.
    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    /// Peek at the top item.
    pub fn peek(&self) -> Option<&T> {
        self.items.last()
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether the stack is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// The capacity of the stack.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

// ---------------------------------------------------------------------------
// CountingSet – track element frequency
// ---------------------------------------------------------------------------

/// A set that tracks how many times each element has been added.
#[derive(Debug, Clone)]
pub struct CountingSet<T: Eq + Hash> {
    counts: HashMap<T, usize>,
}

impl<T: Eq + Hash> Default for CountingSet<T> {
    fn default() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }
}

impl<T: Eq + Hash> CountingSet<T> {
    /// Create an empty counting set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an element, incrementing its count.
    pub fn add(&mut self, item: T) {
        *self.counts.entry(item).or_insert(0) += 1;
    }

    /// Remove one occurrence. Returns `true` if the element was present.
    pub fn remove(&mut self, item: &T) -> bool {
        if let Some(count) = self.counts.get_mut(item) {
            *count -= 1;
            if *count == 0 {
                self.counts.remove(item);
            }
            true
        } else {
            false
        }
    }

    /// Get the count for an element.
    pub fn count(&self, item: &T) -> usize {
        self.counts.get(item).copied().unwrap_or(0)
    }

    /// Number of distinct elements.
    pub fn distinct_count(&self) -> usize {
        self.counts.len()
    }

    /// Total count across all elements.
    pub fn total_count(&self) -> usize {
        self.counts.values().sum()
    }

    /// Whether the set contains the element.
    pub fn contains(&self, item: &T) -> bool {
        self.counts.contains_key(item)
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Get the most frequent element.
    pub fn most_frequent(&self) -> Option<(&T, usize)> {
        self.counts.iter().max_by_key(|(_, c)| *c).map(|(k, &v)| (k, v))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// WeightedRandomSampler - weighted random sampler
// ---------------------------------------------------------------------------

/// Severity level for weighted random sampler issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeightedRandomSamplerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for WeightedRandomSamplerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [WeightedRandomSampler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedRandomSamplerEntry {
    pub id: String,
    pub label: String,
    pub severity: WeightedRandomSamplerSeverity,
    pub detail: Option<String>,
    pub item_count: usize,
    enabled: bool,
}

impl WeightedRandomSamplerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: WeightedRandomSamplerSeverity::Low,
            detail: None,
            item_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: WeightedRandomSamplerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_item_count(mut self, val: usize) -> Self {
        self.item_count = val;
        self
    }

    pub fn total_weight(&self) -> bool {
        self.enabled && self.severity >= WeightedRandomSamplerSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.item_count, det)
    }
}

impl fmt::Display for WeightedRandomSamplerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [WeightedRandomSamplerEntry] items.
#[derive(Debug, Clone)]
pub struct WeightedRandomSampler {
    entries: Vec<WeightedRandomSamplerEntry>,
    name: String,
    capacity: usize,
}

impl WeightedRandomSampler {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: WeightedRandomSamplerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<WeightedRandomSamplerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&WeightedRandomSamplerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn item_count(&self) -> usize { self.entries.len() }

    pub fn total_weight(&self) -> bool {
        self.entries.iter().any(|e| e.total_weight())
    }

    pub fn entries_by_severity(&self, severity: WeightedRandomSamplerSeverity) -> Vec<&WeightedRandomSamplerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= WeightedRandomSamplerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&WeightedRandomSamplerEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&WeightedRandomSamplerEntry> {
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
// SlidingWindowIterator - sliding window iterator
// ---------------------------------------------------------------------------

/// Configuration for [SlidingWindowIterator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlidingWindowIteratorConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub window_size: usize,
}

impl SlidingWindowIteratorConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, window_size: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_window_size(mut self, val: usize) -> Self { self.window_size = val; self }
}

impl Default for SlidingWindowIteratorConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [SlidingWindowIterator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlidingWindowIteratorItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl SlidingWindowIteratorItem {
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

    pub fn has_next_window(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for SlidingWindowIteratorItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [SlidingWindowIteratorItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct SlidingWindowIterator {
    config: SlidingWindowIteratorConfig,
    items: Vec<SlidingWindowIteratorItem>,
}

impl SlidingWindowIterator {
    pub fn new(config: SlidingWindowIteratorConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: SlidingWindowIteratorItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<SlidingWindowIteratorItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&SlidingWindowIteratorItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn window_size(&self) -> usize { self.items.len() }

    pub fn has_next_window(&self) -> bool {
        self.items.iter().any(|i| i.has_next_window())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&SlidingWindowIteratorItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&SlidingWindowIteratorItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &SlidingWindowIteratorConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// vsedit-collections: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionsXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl CollectionsXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for CollectionsXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct CollectionsXRegistry {
    entries: Vec<CollectionsXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl CollectionsXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: CollectionsXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&CollectionsXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut CollectionsXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<CollectionsXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&CollectionsXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&CollectionsXConfig> {
        let mut sorted: Vec<&CollectionsXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&CollectionsXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> CollectionsXIterator<'_> {
        CollectionsXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct CollectionsXIterator<'a> {
    inner: std::slice::Iter<'a, CollectionsXConfig>,
}

impl<'a> Iterator for CollectionsXIterator<'a> {
    type Item = &'a CollectionsXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct CollectionsXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl CollectionsXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct CollectionsXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl CollectionsXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &CollectionsXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &CollectionsXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &CollectionsXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for CollectionsXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct CollectionsXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl CollectionsXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &CollectionsXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &CollectionsXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for CollectionsXValidator {
    fn default() -> Self {
        Self::new()
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
// xb_ utilities – batch 37
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer37 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer37 {
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
pub fn xb_fnv1a_37(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_37<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_37<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_37(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_37(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 17
// ---------------------------------------------------------------------------

/// Generic object pool `Xc17Pool<T>`.
pub struct Xc17Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc17Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc17PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc17Pool<T> {
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
    pub fn stats(&self) -> Xc17PoolStats {
        Xc17PoolStats {
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

impl<T> Default for Xc17Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc17Scheduler`.
pub struct Xc17Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc17Scheduler {
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

impl Default for Xc17Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_17 hash for the given byte slice.
pub fn xc_17_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_17 convention.
pub fn xc_17_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe50 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe50Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe50PipelineError {
    pub stage: Xe50Stage,
    pub message: String,
}

impl std::fmt::Display for Xe50PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe50Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe50Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError>>>,
    stage_names: Vec<Xe50Stage>,
}

impl Xe50Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe50Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe50Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe50Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe50Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> {
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

    pub fn compose(mut self, other: Xe50Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe50CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe50CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe50Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe50CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe50CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe50Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe50CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_50_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe50CacheEntry {
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

    fn xe_50_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe50CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_50_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> {
    Ok(data)
}

pub fn xe_50_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_50_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_50_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_50_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe50PipelineError> {
    Err(Xe50PipelineError {
        stage: Xe50Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_43: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg43Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg43Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg43Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_43: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg43Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg43Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg43Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg43Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 16).
pub struct Xh16SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh16SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 58 as u64,
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

/// A compact bit set supporting boolean operations (variant 16).
pub struct Xh16BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh16BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 16).
pub struct Xi16Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi16Deque<T> {
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
pub struct Xi16Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi16Interval {
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

/// A simple interval tree (variant 16).
pub struct Xi16IntervalTree {
    xi_intervals: Vec<Xi16Interval>,
}

impl Xi16IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi16Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi16Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi16Interval) -> Vec<&Xi16Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi16Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi16Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi16Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi16Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi16Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi16Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 16) ---

/// Disjoint set / union-find for crate 16.
pub struct Xj16UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj16UnionFind {
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

const XJ16_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 16.
pub struct Xj16BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj16BTreeNode<K, V>>>,
    len: usize,
}

struct Xj16BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj16BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj16BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ16_BTREE_ORDER - 1
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
        let mid = XJ16_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj16BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj16BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj16BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj16BTreeNode::xj_new_leaf();
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

#[cfg(test)]
mod tests {
    use super::*;

    // -- LinkedList ----------------------------------------------------------

    #[test]
    fn linked_list_push_front_and_iter() {
        let mut list = LinkedList::new();
        list.push_front(3);
        list.push_front(2);
        list.push_front(1);
        let vals: Vec<_> = list.iter().copied().collect();
        assert_eq!(vals, vec![1, 2, 3]);
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn linked_list_push_back_and_iter() {
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        let vals: Vec<_> = list.iter().copied().collect();
        assert_eq!(vals, vec![1, 2, 3]);
    }

    #[test]
    fn linked_list_remove_middle() {
        let mut list = LinkedList::new();
        list.push_back(1);
        let mid = list.push_back(2);
        list.push_back(3);
        assert_eq!(list.remove(mid), Some(2));
        assert_eq!(list.len(), 2);
        let vals: Vec<_> = list.iter().copied().collect();
        assert_eq!(vals, vec![1, 3]);
    }

    #[test]
    fn linked_list_remove_head() {
        let mut list = LinkedList::new();
        let head = list.push_back(1);
        list.push_back(2);
        assert_eq!(list.remove(head), Some(1));
        let vals: Vec<_> = list.iter().copied().collect();
        assert_eq!(vals, vec![2]);
    }

    #[test]
    fn linked_list_remove_tail() {
        let mut list = LinkedList::new();
        list.push_back(1);
        let tail = list.push_back(2);
        assert_eq!(list.remove(tail), Some(2));
        let vals: Vec<_> = list.iter().copied().collect();
        assert_eq!(vals, vec![1]);
    }

    #[test]
    fn linked_list_double_remove_returns_none() {
        let mut list = LinkedList::new();
        let id = list.push_back(42);
        assert!(list.remove(id).is_some());
        assert!(list.remove(id).is_none());
    }

    #[test]
    fn linked_list_clear() {
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.iter().count(), 0);
    }

    #[test]
    fn linked_list_slot_reuse() {
        let mut list = LinkedList::new();
        let a = list.push_back(10);
        list.push_back(20);
        list.remove(a);
        // The freed slot should be reused.
        let c = list.push_back(30);
        assert_eq!(c, NodeId(0)); // reuses slot 0
        let vals: Vec<_> = list.iter().copied().collect();
        assert_eq!(vals, vec![20, 30]);
    }

    // -- ResourceMap ---------------------------------------------------------

    #[test]
    fn resource_map_case_sensitive() {
        let mut m = ResourceMap::new(false);
        m.set("Hello", 1);
        assert!(m.has("Hello"));
        assert!(!m.has("hello"));
        assert_eq!(m.get("Hello"), Some(&1));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn resource_map_case_insensitive() {
        let mut m = ResourceMap::new(true);
        m.set("Hello", 1);
        assert!(m.has("hello"));
        assert!(m.has("HELLO"));
        assert_eq!(m.get("hElLo"), Some(&1));
    }

    #[test]
    fn resource_map_delete() {
        let mut m = ResourceMap::new(false);
        m.set("key", 42);
        assert_eq!(m.delete("key"), Some(42));
        assert!(!m.has("key"));
        assert!(m.is_empty());
    }

    #[test]
    fn resource_map_keys_values() {
        let mut m = ResourceMap::new(false);
        m.set("a", 1);
        m.set("b", 2);
        let mut keys: Vec<_> = m.keys().collect();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
        let mut vals: Vec<_> = m.values().copied().collect();
        vals.sort();
        assert_eq!(vals, vec![1, 2]);
    }

    // -- SetMap --------------------------------------------------------------

    #[test]
    fn set_map_add_and_get() {
        let mut sm = SetMap::new();
        sm.add("fruits", "apple");
        sm.add("fruits", "banana");
        sm.add("fruits", "apple"); // duplicate
        let set = sm.get(&"fruits").unwrap();
        assert_eq!(set.len(), 2);
        assert!(set.contains(&"apple"));
        assert!(set.contains(&"banana"));
    }

    #[test]
    fn set_map_delete_removes_empty_set() {
        let mut sm = SetMap::new();
        sm.add("k", 1);
        sm.delete(&"k", &1);
        assert!(sm.get(&"k").is_none());
    }

    #[test]
    fn set_map_for_each() {
        let mut sm = SetMap::new();
        sm.add(1, 10);
        sm.add(1, 20);
        let mut collected = Vec::new();
        sm.for_each(&1, |v| collected.push(*v));
        collected.sort();
        assert_eq!(collected, vec![10, 20]);
    }

    // -- BidirectionalMap ----------------------------------------------------

    #[test]
    fn bidir_map_set_and_get() {
        let mut bm = BidirectionalMap::new();
        bm.set("a", 1);
        bm.set("b", 2);
        assert_eq!(bm.get_by_left(&"a"), Some(&1));
        assert_eq!(bm.get_by_right(&2), Some(&"b"));
        assert_eq!(bm.len(), 2);
    }

    #[test]
    fn bidir_map_overwrite_left() {
        let mut bm = BidirectionalMap::new();
        bm.set("a", 1);
        bm.set("a", 2);
        assert_eq!(bm.get_by_left(&"a"), Some(&2));
        assert!(bm.get_by_right(&1).is_none());
        assert_eq!(bm.len(), 1);
    }

    #[test]
    fn bidir_map_overwrite_right() {
        let mut bm = BidirectionalMap::new();
        bm.set("a", 1);
        bm.set("b", 1);
        assert_eq!(bm.get_by_right(&1), Some(&"b"));
        assert!(bm.get_by_left(&"a").is_none());
        assert_eq!(bm.len(), 1);
    }

    #[test]
    fn bidir_map_delete_by_left() {
        let mut bm = BidirectionalMap::new();
        bm.set("a", 1);
        assert_eq!(bm.delete_by_left(&"a"), Some(1));
        assert!(bm.is_empty());
    }

    #[test]
    fn bidir_map_delete_by_right() {
        let mut bm = BidirectionalMap::new();
        bm.set("a", 1);
        assert_eq!(bm.delete_by_right(&1), Some("a"));
        assert!(bm.is_empty());
    }

    // -- group_by ------------------------------------------------------------

    #[test]
    fn group_by_first_char() {
        let words = vec!["apple", "avocado", "banana", "blueberry", "cherry"];
        let groups = group_by(words, |w| w.chars().next().unwrap());
        assert_eq!(groups[&'a'], vec!["apple", "avocado"]);
        assert_eq!(groups[&'b'], vec!["banana", "blueberry"]);
        assert_eq!(groups[&'c'], vec!["cherry"]);
    }

    // -- diff_sets -----------------------------------------------------------

    #[test]
    fn diff_sets_added_and_removed() {
        let before: HashSet<i32> = [1, 2, 3].into_iter().collect();
        let after: HashSet<i32> = [2, 3, 4].into_iter().collect();
        let diff = diff_sets(&before, &after);
        assert_eq!(diff.added, [4].into_iter().collect());
        assert_eq!(diff.removed, [1].into_iter().collect());
    }

    #[test]
    fn diff_sets_identical() {
        let s: HashSet<i32> = [1, 2].into_iter().collect();
        let diff = diff_sets(&s, &s);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
    }

    // -- diff_maps -----------------------------------------------------------

    #[test]
    fn diff_maps_all_cases() {
        let mut before = HashMap::new();
        before.insert("a", 1);
        before.insert("b", 2);
        before.insert("c", 3);

        let mut after = HashMap::new();
        after.insert("b", 2);
        after.insert("c", 30);
        after.insert("d", 4);

        let diff = diff_maps(&before, &after);
        assert_eq!(diff.added, [("d", 4)].into_iter().collect());
        assert_eq!(diff.removed, [("a", 1)].into_iter().collect());
        assert_eq!(diff.changed, [("c", 30)].into_iter().collect());
    }

    // -- coalesce ------------------------------------------------------------

    #[test]
    fn coalesce_merges_adjacent() {
        // Merge adjacent intervals: (start, end)
        let intervals = vec![(0, 3), (3, 5), (5, 8), (10, 12)];
        let merged = coalesce(intervals, |a, b| {
            if a.1 == b.0 {
                Some((a.0, b.1))
            } else {
                None
            }
        });
        assert_eq!(merged, vec![(0, 8), (10, 12)]);
    }

    #[test]
    fn coalesce_empty() {
        let items: Vec<i32> = vec![];
        let result = coalesce(items, |_, _| None);
        assert!(result.is_empty());
    }

    #[test]
    fn coalesce_no_merges() {
        let items = vec![1, 3, 5];
        let result = coalesce(items, |_, _| None);
        assert_eq!(result, vec![1, 3, 5]);
    }

    // -- LruCache -----------------------------------------------------------

    #[test]
    fn lru_cache_basic_get_set() {
        let mut cache = LruCache::new(3);
        cache.set("a", 1);
        cache.set("b", 2);
        cache.set("c", 3);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&"a"), Some(&1));
        assert_eq!(cache.get(&"b"), Some(&2));
        assert!(cache.contains_key(&"c"));
    }

    #[test]
    fn lru_cache_eviction() {
        let mut cache = LruCache::new(2);
        cache.set("a", 1);
        cache.set("b", 2);
        let evicted = cache.set("c", 3); // evicts "a"
        assert_eq!(evicted, Some(("a", 1)));
        assert!(!cache.contains_key(&"a"));
        assert!(cache.contains_key(&"b"));
        assert!(cache.contains_key(&"c"));
    }

    #[test]
    fn lru_cache_access_updates_order() {
        let mut cache = LruCache::new(2);
        cache.set("a", 1);
        cache.set("b", 2);
        cache.get(&"a"); // "a" is now most recent
        let evicted = cache.set("c", 3); // evicts "b" (least recent)
        assert_eq!(evicted, Some(("b", 2)));
        assert!(cache.contains_key(&"a"));
    }

    #[test]
    fn lru_cache_update_existing() {
        let mut cache = LruCache::new(2);
        cache.set("a", 1);
        cache.set("b", 2);
        cache.set("a", 10); // update, no eviction
        assert_eq!(cache.get(&"a"), Some(&10));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn lru_cache_remove() {
        let mut cache = LruCache::new(3);
        cache.set("a", 1);
        cache.set("b", 2);
        assert_eq!(cache.remove(&"a"), Some(1));
        assert_eq!(cache.len(), 1);
        assert!(cache.remove(&"z").is_none());
    }

    #[test]
    fn lru_cache_peek_no_reorder() {
        let mut cache = LruCache::new(2);
        cache.set("a", 1);
        cache.set("b", 2);
        assert_eq!(cache.peek(&"a"), Some(&1));
        // Peek shouldn't change order, so "a" should still be LRU
        let evicted = cache.set("c", 3);
        assert_eq!(evicted, Some(("a", 1)));
    }

    #[test]
    fn lru_cache_clear() {
        let mut cache = LruCache::new(3);
        cache.set("a", 1);
        cache.set("b", 2);
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.capacity(), 3);
    }

    #[test]
    fn lru_cache_keys_order() {
        let mut cache = LruCache::new(3);
        cache.set("a", 1);
        cache.set("b", 2);
        cache.set("c", 3);
        cache.get(&"a"); // move "a" to most recent
        let keys: Vec<_> = cache.keys().collect();
        assert_eq!(keys, vec![&"b", &"c", &"a"]); // LRU → MRU
    }

    // -- PriorityQueue ------------------------------------------------------

    #[test]
    fn priority_queue_min_heap() {
        let mut pq = PriorityQueue::new();
        pq.push(5);
        pq.push(1);
        pq.push(3);
        pq.push(2);
        pq.push(4);
        assert_eq!(pq.len(), 5);
        assert_eq!(pq.peek(), Some(&1));
        assert_eq!(pq.pop(), Some(1));
        assert_eq!(pq.pop(), Some(2));
        assert_eq!(pq.pop(), Some(3));
        assert_eq!(pq.pop(), Some(4));
        assert_eq!(pq.pop(), Some(5));
        assert!(pq.is_empty());
    }

    #[test]
    fn priority_queue_drain_sorted() {
        let mut pq = PriorityQueue::new();
        pq.push(10);
        pq.push(3);
        pq.push(7);
        pq.push(1);
        let sorted = pq.drain_sorted();
        assert_eq!(sorted, vec![1, 3, 7, 10]);
        assert!(pq.is_empty());
    }

    #[test]
    fn priority_queue_single_element() {
        let mut pq = PriorityQueue::new();
        pq.push(42);
        assert_eq!(pq.peek(), Some(&42));
        assert_eq!(pq.pop(), Some(42));
        assert!(pq.pop().is_none());
    }

    #[test]
    fn priority_queue_duplicates() {
        let mut pq = PriorityQueue::new();
        pq.push(3);
        pq.push(1);
        pq.push(3);
        pq.push(1);
        assert_eq!(pq.drain_sorted(), vec![1, 1, 3, 3]);
    }

    #[test]
    fn priority_queue_clear() {
        let mut pq = PriorityQueue::new();
        pq.push(1);
        pq.push(2);
        pq.clear();
        assert!(pq.is_empty());
    }

    #[test]
    fn priority_queue_with_strings() {
        let mut pq: PriorityQueue<String> = PriorityQueue::new();
        pq.push("banana".into());
        pq.push("apple".into());
        pq.push("cherry".into());
        assert_eq!(pq.pop().as_deref(), Some("apple"));
        assert_eq!(pq.pop().as_deref(), Some("banana"));
        assert_eq!(pq.pop().as_deref(), Some("cherry"));
    }

    // -- IntervalMap tests ---------------------------------------------------

    #[test]
    fn interval_map_insert_and_query() {
        let mut map = IntervalMap::new();
        assert!(map.insert(0, 10, "a"));
        assert!(map.insert(20, 30, "b"));
        assert!(!map.insert(5, 15, "c")); // overlaps with [0,10)
        assert_eq!(map.query(5), Some(&"a"));
        assert_eq!(map.query(15), None);
        assert_eq!(map.query(25), Some(&"b"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn interval_map_remove_and_range_query() {
        let mut map = IntervalMap::new();
        map.insert(0, 10, "x");
        map.insert(20, 30, "y");
        let overlapping = map.query_range(5, 25);
        assert_eq!(overlapping.len(), 2);
        assert_eq!(map.remove_at(5), Some("x"));
        assert_eq!(map.len(), 1);
    }

    // -- Trie tests ----------------------------------------------------------

    #[test]
    fn trie_insert_contains_prefix() {
        let mut trie = Trie::new();
        trie.insert("hello");
        trie.insert("help");
        trie.insert("world");
        assert!(trie.contains("hello"));
        assert!(!trie.contains("hel"));
        assert!(trie.has_prefix("hel"));
        assert!(!trie.has_prefix("xyz"));
        assert_eq!(trie.len(), 3);
    }

    #[test]
    fn trie_words_with_prefix() {
        let mut trie = Trie::new();
        trie.insert("apple");
        trie.insert("app");
        trie.insert("application");
        trie.insert("banana");
        let words = trie.words_with_prefix("app");
        assert_eq!(words, vec!["app", "apple", "application"]);
    }

    // -- sorted_merge tests --------------------------------------------------

    #[test]
    fn sorted_merge_two_sorted_lists() {
        let a = vec![1, 3, 5, 7];
        let b = vec![2, 4, 6, 8];
        let merged: Vec<i32> = sorted_merge(a, b).collect();
        assert_eq!(merged, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn sorted_merge_one_empty() {
        let a: Vec<i32> = vec![1, 2, 3];
        let b: Vec<i32> = vec![];
        let merged: Vec<i32> = sorted_merge(a, b).collect();
        assert_eq!(merged, vec![1, 2, 3]);
    }

    #[test]
    fn sorted_merge_with_duplicates() {
        let a = vec![1, 3, 3];
        let b = vec![2, 3, 4];
        let merged: Vec<i32> = sorted_merge(a, b).collect();
        assert_eq!(merged, vec![1, 2, 3, 3, 3, 4]);
    }

    // -- LinkedList front/back/pop/contains/find ----------------------------

    #[test]
    fn linked_list_front_and_back() {
        let mut list = LinkedList::new();
        assert!(list.front().is_none());
        assert!(list.back().is_none());
        list.push_back(10);
        list.push_back(20);
        list.push_back(30);
        assert_eq!(list.front(), Some(&10));
        assert_eq!(list.back(), Some(&30));
    }

    #[test]
    fn linked_list_pop_front_and_pop_back() {
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        assert_eq!(list.pop_front(), Some(1));
        assert_eq!(list.pop_back(), Some(3));
        assert_eq!(list.len(), 1);
        assert_eq!(list.front(), Some(&2));
    }

    #[test]
    fn linked_list_into_vec() {
        let mut list = LinkedList::new();
        list.push_back(10);
        list.push_back(20);
        list.push_back(30);
        assert_eq!(list.into_vec(), vec![10, 20, 30]);
    }

    #[test]
    fn linked_list_contains_and_find() {
        let mut list = LinkedList::new();
        list.push_back(5);
        list.push_back(10);
        list.push_back(15);
        assert!(list.contains(&10));
        assert!(!list.contains(&99));
        let id = list.find(&10).unwrap();
        assert_eq!(list.remove(id), Some(10));
        assert!(!list.contains(&10));
    }

    #[test]
    fn linked_list_arena_capacity() {
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        assert!(list.arena_capacity() >= 2);
    }

    // -- SetMap additional ---------------------------------------------------

    #[test]
    fn set_map_total_values_and_key_count() {
        let mut sm = SetMap::new();
        sm.add("a", 1);
        sm.add("a", 2);
        sm.add("b", 3);
        assert_eq!(sm.key_count(), 2);
        assert_eq!(sm.total_values(), 3);
    }

    #[test]
    fn set_map_contains_and_remove_key() {
        let mut sm = SetMap::new();
        sm.add("k", 10);
        sm.add("k", 20);
        assert!(sm.contains(&"k", &10));
        assert!(!sm.contains(&"k", &99));
        let removed = sm.remove_key(&"k").unwrap();
        assert_eq!(removed.len(), 2);
        assert!(sm.get(&"k").is_none());
    }

    // -- BidirectionalMap additional -----------------------------------------

    #[test]
    fn bidir_map_clear_and_contains() {
        let mut bm = BidirectionalMap::new();
        bm.set("x", 1);
        bm.set("y", 2);
        assert!(bm.contains_left(&"x"));
        assert!(bm.contains_right(&2));
        assert!(!bm.contains_left(&"z"));
        bm.clear();
        assert!(bm.is_empty());
        assert!(!bm.contains_left(&"x"));
    }

    #[test]
    fn bidir_map_iter() {
        let mut bm = BidirectionalMap::new();
        bm.set("a", 1);
        bm.set("b", 2);
        let mut pairs: Vec<_> = bm.iter().map(|(l, r)| (*l, *r)).collect();
        pairs.sort();
        assert_eq!(pairs, vec![("a", 1), ("b", 2)]);
    }

    // -- OrderedMap tests --

    #[test]
    fn ordered_map_insertion_order() {
        let mut m = OrderedMap::new();
        m.insert("c", 3);
        m.insert("a", 1);
        m.insert("b", 2);
        let keys: Vec<&&str> = m.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec![&"c", &"a", &"b"]);
    }

    #[test]
    fn ordered_map_update_preserves_order() {
        let mut m = OrderedMap::new();
        m.insert("a", 1);
        m.insert("b", 2);
        m.insert("a", 10);
        let keys: Vec<&&str> = m.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec![&"a", &"b"]);
        assert_eq!(m.get(&"a"), Some(&10));
    }

    #[test]
    fn ordered_map_remove() {
        let mut m = OrderedMap::new();
        m.insert("a", 1);
        m.insert("b", 2);
        m.remove(&"a");
        assert_eq!(m.len(), 1);
        assert!(!m.contains_key(&"a"));
    }

    // -- BiMap tests --

    #[test]
    fn bimap_bidirectional() {
        let mut bm = BiMap::new();
        bm.insert("hello", 42);
        assert_eq!(bm.get_left(&"hello"), Some(&42));
        assert_eq!(bm.get_right(&42), Some(&"hello"));
    }

    #[test]
    fn bimap_remove() {
        let mut bm = BiMap::new();
        bm.insert("a", 1);
        assert_eq!(bm.remove_left(&"a"), Some(1));
        assert!(bm.is_empty());
    }

    // -- BoundedStack tests --

    #[test]
    fn bounded_stack_push_pop() {
        let mut s = BoundedStack::new(3);
        s.push(1);
        s.push(2);
        s.push(3);
        assert!(s.is_full());
        assert_eq!(s.pop(), Some(3));
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn bounded_stack_overflow() {
        let mut s = BoundedStack::new(2);
        s.push(1);
        s.push(2);
        s.push(3); // drops 1
        assert_eq!(s.len(), 2);
        assert_eq!(s.pop(), Some(3));
        assert_eq!(s.pop(), Some(2));
        assert!(s.is_empty());
    }

    #[test]
    fn bounded_stack_peek() {
        let mut s = BoundedStack::new(5);
        s.push(10);
        assert_eq!(s.peek(), Some(&10));
    }

    // -- CountingSet tests --

    #[test]
    fn counting_set_add_count() {
        let mut cs = CountingSet::new();
        cs.add("a");
        cs.add("a");
        cs.add("b");
        assert_eq!(cs.count(&"a"), 2);
        assert_eq!(cs.count(&"b"), 1);
        assert_eq!(cs.distinct_count(), 2);
        assert_eq!(cs.total_count(), 3);
    }

    #[test]
    fn counting_set_remove() {
        let mut cs = CountingSet::new();
        cs.add("a");
        cs.add("a");
        cs.remove(&"a");
        assert_eq!(cs.count(&"a"), 1);
        cs.remove(&"a");
        assert!(!cs.contains(&"a"));
    }

    #[test]
    fn counting_set_most_frequent() {
        let mut cs = CountingSet::new();
        cs.add("a");
        cs.add("b");
        cs.add("b");
        cs.add("b");
        let (item, count) = cs.most_frequent().unwrap();
        assert_eq!(*item, "b");
        assert_eq!(count, 3);
    }

#[test]
    fn weightedrandomsampler_severity_ordering() {
        assert!(WeightedRandomSamplerSeverity::Critical > WeightedRandomSamplerSeverity::High);
        assert!(WeightedRandomSamplerSeverity::High > WeightedRandomSamplerSeverity::Medium);
        assert!(WeightedRandomSamplerSeverity::Medium > WeightedRandomSamplerSeverity::Low);
    }

    #[test]
    fn weightedrandomsampler_severity_display() {
        assert_eq!(WeightedRandomSamplerSeverity::Low.to_string(), "low");
        assert_eq!(WeightedRandomSamplerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn weightedrandomsampler_entry_creation() {
        let e = WeightedRandomSamplerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, WeightedRandomSamplerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn weightedrandomsampler_entry_builder() {
        let e = WeightedRandomSamplerEntry::new("e2", "Entry 2")
            .with_severity(WeightedRandomSamplerSeverity::High)
            .with_detail("some detail")
            .with_item_count(42);
        assert_eq!(e.severity, WeightedRandomSamplerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.item_count, 42);
    }

    #[test]
    fn weightedrandomsampler_entry_enable_disable() {
        let mut e = WeightedRandomSamplerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn weightedrandomsampler_add_and_count() {
        let mut mgr = WeightedRandomSampler::new("test");
        mgr.add(WeightedRandomSamplerEntry::new("a", "A"));
        mgr.add(WeightedRandomSamplerEntry::new("b", "B").with_severity(WeightedRandomSamplerSeverity::High));
        assert_eq!(mgr.item_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn weightedrandomsampler_remove() {
        let mut mgr = WeightedRandomSampler::new("test");
        mgr.add(WeightedRandomSamplerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn weightedrandomsampler_capacity() {
        let mut mgr = WeightedRandomSampler::new("test").with_capacity(1);
        assert!(mgr.add(WeightedRandomSamplerEntry::new("a", "A")));
        assert!(!mgr.add(WeightedRandomSamplerEntry::new("b", "B")));
    }

    #[test]
    fn weightedrandomsampler_sorted_by_severity() {
        let mut mgr = WeightedRandomSampler::new("test");
        mgr.add(WeightedRandomSamplerEntry::new("lo", "Low"));
        mgr.add(WeightedRandomSamplerEntry::new("hi", "High").with_severity(WeightedRandomSamplerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, WeightedRandomSamplerSeverity::Critical);
    }

    #[test]
    fn weightedrandomsampler_summary() {
        let mgr = WeightedRandomSampler::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn slidingwindowiterator_config_defaults() {
        let cfg = SlidingWindowIteratorConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn slidingwindowiterator_item_creation() {
        let item = SlidingWindowIteratorItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn slidingwindowiterator_add_and_get() {
        let mut mgr = SlidingWindowIterator::new(SlidingWindowIteratorConfig::new("test"));
        mgr.add(SlidingWindowIteratorItem::new("k1", "v1"));
        assert_eq!(mgr.window_size(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn slidingwindowiterator_remove_item() {
        let mut mgr = SlidingWindowIterator::new(SlidingWindowIteratorConfig::new("test"));
        mgr.add(SlidingWindowIteratorItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn slidingwindowiterator_sorted_by_priority() {
        let mut mgr = SlidingWindowIterator::new(SlidingWindowIteratorConfig::new("test"));
        mgr.add(SlidingWindowIteratorItem::new("lo", "low").with_priority(1));
        mgr.add(SlidingWindowIteratorItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn slidingwindowiterator_items_with_tag() {
        let mut mgr = SlidingWindowIterator::new(SlidingWindowIteratorConfig::new("test"));
        mgr.add(SlidingWindowIteratorItem::new("a", "1").with_tag("x"));
        mgr.add(SlidingWindowIteratorItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn slidingwindowiterator_report() {
        let mgr = SlidingWindowIterator::new(SlidingWindowIteratorConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn collections_x_config_new() {
        let c = CollectionsXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn collections_x_config_builder() {
        let c = CollectionsXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn collections_x_config_display() {
        let c = CollectionsXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn collections_x_registry_insert_get() {
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn collections_x_registry_duplicate() {
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("a")).unwrap();
        assert!(reg.insert(CollectionsXConfig::new("a")).is_err());
    }

    #[test]
    fn collections_x_registry_remove() {
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("a")).unwrap();
        reg.insert(CollectionsXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn collections_x_registry_active_entries() {
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("a")).unwrap();
        reg.insert(CollectionsXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn collections_x_registry_by_weight() {
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(CollectionsXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn collections_x_registry_tags() {
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(CollectionsXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn collections_x_registry_total_weight() {
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(CollectionsXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn collections_x_registry_iterator() {
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("a")).unwrap();
        reg.insert(CollectionsXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn collections_x_cache_put_get() {
        let mut cache = CollectionsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn collections_x_cache_eviction() {
        let mut cache = CollectionsXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn collections_x_cache_lru_order() {
        let mut cache = CollectionsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn collections_x_cache_most_least_recent() {
        let mut cache = CollectionsXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn collections_x_formatter_entry() {
        let e = CollectionsXConfig::new("k").with_value("v");
        let fmt = CollectionsXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn collections_x_formatter_summary() {
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("a").with_weight(5)).unwrap();
        let fmt = CollectionsXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn collections_x_validator_valid() {
        let v = CollectionsXValidator::new();
        let c = CollectionsXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn collections_x_validator_empty_key() {
        let v = CollectionsXValidator::new();
        let c = CollectionsXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn collections_x_validator_require_value() {
        let v = CollectionsXValidator::new().require_value(true);
        let c = CollectionsXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn collections_x_validator_allowed_tags() {
        let v = CollectionsXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = CollectionsXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn collections_x_validator_validate_all() {
        let v = CollectionsXValidator::new();
        let mut reg = CollectionsXRegistry::new();
        reg.insert(CollectionsXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
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


    #[test]
    fn xb_ring_buffer_37_push_and_len() {
        let mut rb = super::XbRingBuffer37::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_37_overwrite() {
        let mut rb = super::XbRingBuffer37::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_37_get_out_of_bounds() {
        let rb = super::XbRingBuffer37::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_37_drain_all() {
        let mut rb = super::XbRingBuffer37::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_37_peek_front_back() {
        let mut rb = super::XbRingBuffer37::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_37_clear() {
        let mut rb = super::XbRingBuffer37::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_37_capacity() {
        let rb = super::XbRingBuffer37::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_37_basic() {
        let h = super::xb_fnv1a_37(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_37(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_37_different_inputs() {
        let h1 = super::xb_fnv1a_37(b"abc");
        let h2 = super::xb_fnv1a_37(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_37_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_37(&data);
        let dec = super::xb_rle_decode_37(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_37_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_37(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_37(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_37_values() {
        assert!((super::xb_clamp_37(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_37(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_37(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_37_values() {
        assert!((super::xb_lerp_37(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_37(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_37(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_37_wrap_around_twice() {
        let mut rb = super::XbRingBuffer37::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 17 ----

    #[test]
    fn xc_17_pool_new_empty() {
        let pool: super::Xc17Pool<i32> = super::Xc17Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_17_pool_release_acquire() {
        let mut pool = super::Xc17Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_17_pool_acquire_empty() {
        let mut pool: super::Xc17Pool<i32> = super::Xc17Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_17_pool_full() {
        let mut pool = super::Xc17Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_17_pool_drain() {
        let mut pool = super::Xc17Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_17_pool_stats() {
        let mut pool = super::Xc17Pool::new(8);
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
    fn xc_17_pool_clear() {
        let mut pool = super::Xc17Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_17_pool_shrink() {
        let mut pool = super::Xc17Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_17_pool_default() {
        let pool: super::Xc17Pool<String> = super::Xc17Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_17_pool_extend() {
        let mut pool = super::Xc17Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_17_pool_retain() {
        let mut pool = super::Xc17Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_17_scheduler_round_robin() {
        let mut sched = super::Xc17Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_17_scheduler_empty() {
        let mut sched = super::Xc17Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_17_scheduler_reset() {
        let mut sched = super::Xc17Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_17_scheduler_add_remove() {
        let mut sched = super::Xc17Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_17_scheduler_targets() {
        let sched = super::Xc17Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_17_hash_empty() {
        assert_eq!(super::xc_17_hash(b""), 5381);
    }

    #[test]
    fn xc_17_hash_data() {
        let h = super::xc_17_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_17_hash(b"hello"), h);
    }

    #[test]
    fn xc_17_reverse_str() {
        assert_eq!(super::xc_17_reverse("abc"), "cba");
        assert_eq!(super::xc_17_reverse(""), "");
    }


    #[test]
    fn xe_50_pipeline_empty() {
        let p = super::Xe50Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_50_pipeline_parse_stage() {
        let p = super::Xe50Pipeline::new()
            .add_parse(super::xe_50_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_50_pipeline_transform_double() {
        let p = super::Xe50Pipeline::new()
            .add_transform(super::xe_50_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_50_pipeline_validate_reverse() {
        let p = super::Xe50Pipeline::new()
            .add_validate(super::xe_50_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_50_pipeline_emit_filter() {
        let p = super::Xe50Pipeline::new()
            .add_emit(super::xe_50_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_50_pipeline_multi_stage() {
        let p = super::Xe50Pipeline::new()
            .add_parse(super::xe_50_pipeline_identity)
            .add_transform(super::xe_50_pipeline_double)
            .add_validate(super::xe_50_pipeline_reverse)
            .add_emit(super::xe_50_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_50_pipeline_error_propagation() {
        let p = super::Xe50Pipeline::new()
            .add_parse(super::xe_50_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe50Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_50_pipeline_compose() {
        let p1 = super::Xe50Pipeline::new()
            .add_parse(super::xe_50_pipeline_identity);
        let p2 = super::Xe50Pipeline::new()
            .add_transform(super::xe_50_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_50_pipeline_error_display() {
        let e = super::Xe50PipelineError {
            stage: super::Xe50Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_50_cache_put_get() {
        let mut c = super::Xe50Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_50_cache_miss() {
        let mut c: super::Xe50Cache<&str, i32> = super::Xe50Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_50_cache_ttl_expiry() {
        let mut c = super::Xe50Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_50_cache_evict() {
        let mut c = super::Xe50Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_50_cache_capacity() {
        let mut c = super::Xe50Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_50_cache_stats() {
        let mut c = super::Xe50Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_50_cache_clear() {
        let mut c = super::Xe50Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_43 graph tests ------------------------------------------------

    #[test]
    fn xg_43_graph_empty() {
        let g = super::Xg43Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_43_graph_add_node() {
        let mut g = super::Xg43Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_43_graph_add_edge() {
        let mut g = super::Xg43Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_43_graph_neighbors() {
        let mut g = super::Xg43Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_43_graph_has_path() {
        let mut g = super::Xg43Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_43_graph_self_path() {
        let g = super::Xg43Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_43_graph_topo_sort() {
        let mut g = super::Xg43Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_43_graph_cycle_detect_false() {
        let mut g = super::Xg43Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_43_graph_cycle_detect_true() {
        let mut g = super::Xg43Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_43 heap tests -------------------------------------------------

    #[test]
    fn xg_43_heap_empty() {
        let h: super::Xg43Heap<i32> = super::Xg43Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_43_heap_push_pop() {
        let mut h = super::Xg43Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_43_heap_peek() {
        let mut h = super::Xg43Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_43_heap_drain_sorted() {
        let mut h = super::Xg43Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_43_heap_merge() {
        let mut a = super::Xg43Heap::new();
        let mut b = super::Xg43Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_43_heap_default() {
        let h: super::Xg43Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_43_graph_default() {
        let g: super::Xg43Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh16_skip_insert_contains() {
        let mut sl = super::Xh16SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh16_skip_remove() {
        let mut sl = super::Xh16SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh16_skip_len() {
        let mut sl = super::Xh16SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh16_skip_range_query() {
        let mut sl = super::Xh16SkipList::xh_new(4);
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
    fn xh16_skip_floor_ceiling() {
        let mut sl = super::Xh16SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh16_skip_rank() {
        let mut sl = super::Xh16SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh16_skip_empty() {
        let sl = super::Xh16SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh16_skip_duplicates() {
        let mut sl = super::Xh16SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh16_bitset_set_test() {
        let mut bs = super::Xh16BitSet::xh_new(256);
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
    fn xh16_bitset_clear_count() {
        let mut bs = super::Xh16BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh16_bitset_and_or_xor() {
        let mut a = super::Xh16BitSet::xh_new(128);
        let mut b = super::Xh16BitSet::xh_new(128);
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
    fn xh16_bitset_iter_ones() {
        let mut bs = super::Xh16BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh16_bitset_first_last() {
        let mut bs = super::Xh16BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh16_bitset_empty() {
        let bs = super::Xh16BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi16_deque_push_pop_back() {
        let mut dq = super::Xi16Deque::xi_new(4);
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
    fn xi16_deque_push_pop_front() {
        let mut dq = super::Xi16Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi16_deque_mixed_ops() {
        let mut dq = super::Xi16Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi16_deque_get_and_split() {
        let mut dq = super::Xi16Deque::xi_new(8);
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
    fn xi16_deque_rotate_left() {
        let mut dq = super::Xi16Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi16_deque_rotate_right() {
        let mut dq = super::Xi16Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi16_deque_grow() {
        let mut dq = super::Xi16Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi16_deque_empty() {
        let dq = super::Xi16Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi16_interval_tree_insert_query() {
        let mut tree = super::Xi16IntervalTree::xi_new();
        tree.xi_insert(super::Xi16Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi16Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi16Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi16_interval_tree_overlap() {
        let mut tree = super::Xi16IntervalTree::xi_new();
        tree.xi_insert(super::Xi16Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi16Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi16Interval::xi_new(12, 20));
        let q = super::Xi16Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi16_interval_tree_remove() {
        let mut tree = super::Xi16IntervalTree::xi_new();
        tree.xi_insert(super::Xi16Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi16Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi16_interval_tree_gaps() {
        let mut tree = super::Xi16IntervalTree::xi_new();
        tree.xi_insert(super::Xi16Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi16Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi16Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi16Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi16Interval::xi_new(8, 10));
    }

    #[test]
    fn xi16_interval_tree_merge() {
        let mut tree = super::Xi16IntervalTree::xi_new();
        tree.xi_insert(super::Xi16Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi16Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi16Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi16Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi16Interval::xi_new(10, 15));
    }

    #[test]
    fn xi16_interval_tree_all() {
        let mut tree = super::Xi16IntervalTree::xi_new();
        tree.xi_insert(super::Xi16Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi16Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi16_interval_tree_empty() {
        let tree = super::Xi16IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi16_interval_tree_contains_point() {
        let iv = super::Xi16Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 16) ---

    #[test]
    fn xj_16_uf_make_and_find() {
        let mut uf = super::Xj16UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_16_uf_union_connected() {
        let mut uf = super::Xj16UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_16_uf_component_count() {
        let mut uf = super::Xj16UnionFind::xj_new();
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
    fn xj_16_uf_component_size() {
        let mut uf = super::Xj16UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_16_uf_largest_component() {
        let mut uf = super::Xj16UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_16_uf_many_elements() {
        let mut uf = super::Xj16UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_16_uf_separate_components() {
        let mut uf = super::Xj16UnionFind::xj_new();
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
    fn xj_16_uf_path_compression() {
        let mut uf = super::Xj16UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_16_bt_insert_get() {
        let mut bt = super::Xj16BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_16_bt_contains_len() {
        let mut bt = super::Xj16BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_16_bt_replace() {
        let mut bt = super::Xj16BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_16_bt_remove() {
        let mut bt = super::Xj16BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_16_bt_keys_values() {
        let mut bt = super::Xj16BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_16_bt_range() {
        let mut bt = super::Xj16BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_16_bt_min_max() {
        let mut bt = super::Xj16BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_16_bt_many_inserts() {
        let mut bt = super::Xj16BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
