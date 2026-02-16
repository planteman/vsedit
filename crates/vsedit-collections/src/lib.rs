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
// Tests
// ---------------------------------------------------------------------------

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
}
