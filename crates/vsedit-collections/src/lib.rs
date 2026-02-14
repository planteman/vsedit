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
}
