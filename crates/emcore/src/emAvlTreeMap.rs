// emAvlTreeMap.rs — COW ordered map, ported from emAvlTreeMap.h
//
// C++ emAvlTreeMap is a copy-on-write sorted map backed by an intrusive AVL
// tree. Rust wraps BTreeMap in Rc for COW + ordered access.
//
// DIVERGED: (language-forced) Element struct — C++ exposes `Element { Key, Value, AvlNode }`.
// Rust returns `(&K, &V)` tuples or `Option<&V>` directly since there is no
// intrusive AVL node to expose.
//
// DIVERGED: (language-forced) Iterator inner class — C++ Iterator is a stable cursor with AVL
// node stack and auto-adjustment on mutation (auto-advances past removed
// elements). Rust provides MapCursor, which tracks position by key clone and
// returns None (not auto-advance) when the pointed-to element is removed.
//
// DIVERGED: (language-forced) GetKeyWritable — omitted. Mutating keys in a sorted map is
// inherently dangerous (can break ordering). C++ documents "must not disturb
// order". Rust prevents this at the API level.
//
// DIVERGED: (language-forced) Element-pointer overloads of Get, GetKey, GetValue, GetValueWritable,
// SetValue, Remove — omitted. C++ uses raw pointers to elements; Rust API
// uses key references instead.

use std::collections::BTreeMap;
use std::ops::Bound;
use std::ops::Index;
use std::rc::Rc;

/// Copy-on-write ordered map matching C++ `emAvlTreeMap<KEY, VALUE>`.
pub struct emAvlTreeMap<K: Ord + Clone, V: Clone> {
    data: Rc<BTreeMap<K, V>>,
}

impl<K: Ord + Clone, V: Clone> emAvlTreeMap<K, V> {
    // --- Construction ---

    /// Construct an empty map.
    pub fn new() -> Self {
        Self {
            data: Rc::new(BTreeMap::new()),
        }
    }

    /// Construct a map with one element.
    pub fn from_entry(key: K, value: V) -> Self {
        let mut map = BTreeMap::new();
        map.insert(key, value);
        Self { data: Rc::new(map) }
    }

    // --- Read ---

    /// Ask whether the map contains an element whose key equals the given key.
    pub fn Contains(&self, key: &K) -> bool {
        self.data.contains_key(key)
    }

    /// Get a reference to the value of the element whose key equals the given key.
    pub fn GetValue(&self, key: &K) -> Option<&V> {
        self.data.get(key)
    }

    /// Get a reference to the key matching the given key (useful when K has
    /// data beyond what Ord compares).
    pub fn GetKey(&self, key: &K) -> Option<&K> {
        self.data.get_key_value(key).map(|(k, _)| k)
    }

    /// Ask whether this map has no elements.
    pub fn IsEmpty(&self) -> bool {
        self.data.is_empty()
    }

    /// Compute the number of elements.
    pub fn GetCount(&self) -> usize {
        self.data.len()
    }

    /// Get number of references to the data behind this map.
    pub fn GetDataRefCount(&self) -> usize {
        Rc::strong_count(&self.data)
    }

    // --- Ordered access ---

    /// Get the element with the smallest key.
    pub fn GetFirst(&self) -> Option<(&K, &V)> {
        self.data.iter().next()
    }

    /// Get the element with the largest key.
    pub fn GetLast(&self) -> Option<(&K, &V)> {
        self.data.iter().next_back()
    }

    /// Get the nearest element whose key is strictly greater than the given key.
    pub fn GetNearestGreater(&self, key: &K) -> Option<(&K, &V)> {
        self.data
            .range((Bound::Excluded(key.clone()), Bound::Unbounded))
            .next()
    }

    /// Get the nearest element whose key is greater than or equal to the given key.
    pub fn GetNearestGreaterOrEqual(&self, key: &K) -> Option<(&K, &V)> {
        self.data.range(key..).next()
    }

    /// Get the nearest element whose key is strictly less than the given key.
    pub fn GetNearestLess(&self, key: &K) -> Option<(&K, &V)> {
        self.data.range(..key).next_back()
    }

    /// Get the nearest element whose key is less than or equal to the given key.
    pub fn GetNearestLessOrEqual(&self, key: &K) -> Option<(&K, &V)> {
        self.data
            .range((Bound::Unbounded, Bound::Included(key.clone())))
            .next_back()
    }

    // --- Mutate ---

    /// Insert or update an element. Same as `SetValue(key, value, true)`.
    pub fn Insert(&mut self, key: K, value: V) {
        self.SetValue(key, value, true);
    }

    /// Set the value of an element. If `insert_if_new` is true, the element
    /// is created if it is not found. If false, only updates existing keys.
    pub fn SetValue(&mut self, key: K, value: V, insert_if_new: bool) {
        if !insert_if_new && !self.data.contains_key(&key) {
            return;
        }
        Rc::make_mut(&mut self.data).insert(key, value);
    }

    /// Remove the element that matches a key. If the element does not exist,
    /// nothing is removed.
    pub fn Remove(&mut self, key: &K) {
        if self.data.contains_key(key) {
            Rc::make_mut(&mut self.data).remove(key);
        }
    }

    /// Remove the first (smallest key) element.
    pub fn RemoveFirst(&mut self) {
        if let Some(key) = self.data.keys().next().cloned() {
            Rc::make_mut(&mut self.data).remove(&key);
        }
    }

    /// Remove the last (largest key) element.
    pub fn RemoveLast(&mut self) {
        if let Some(key) = self.data.keys().next_back().cloned() {
            Rc::make_mut(&mut self.data).remove(&key);
        }
    }

    /// Remove all elements.
    pub fn Clear(&mut self) {
        if !self.data.is_empty() {
            self.data = Rc::new(BTreeMap::new());
        }
    }

    /// Ensure data is not shared. Call before handing to another context.
    pub fn MakeNonShared(&mut self) {
        Rc::make_mut(&mut self.data);
    }
}

impl<K: Ord + Clone, V: Clone + Default> emAvlTreeMap<K, V> {
    /// Get a mutable reference to the value of an element. If `insert_if_new`
    /// is true, the element is created with `Default::default()` if not found.
    pub fn GetValueWritable(&mut self, key: &K, insert_if_new: bool) -> Option<&mut V> {
        if insert_if_new {
            let map = Rc::make_mut(&mut self.data);
            Some(map.entry(key.clone()).or_default())
        } else if self.data.contains_key(key) {
            let map = Rc::make_mut(&mut self.data);
            map.get_mut(key)
        } else {
            None
        }
    }
}

/// Stable cursor for emAvlTreeMap. Tracks position by cloned key.
///
/// DIVERGED: (language-forced) C++ `Iterator` auto-advances when the pointed-to element is
/// removed. This cursor returns `None` from `Get` instead. C++ `Iterator` is
/// nullified on `operator=`; this cursor holds a key copy independent of map
/// identity so it can be compared against any map with the same key type.
pub struct MapCursor<K: Clone> {
    key: Option<K>,
}

impl<K: Ord + Clone> MapCursor<K> {
    /// Return the key/value pair the cursor is pointing at, or `None` if the
    /// cursor is detached or the key no longer exists in `map`.
    pub fn Get<'a, V: Clone>(&self, map: &'a emAvlTreeMap<K, V>) -> Option<(&'a K, &'a V)> {
        self.key.as_ref().and_then(|k| map.data.get_key_value(k))
    }

    /// Advance the cursor to the next (larger) key in `map`.
    pub fn SetNext<V: Clone>(&mut self, map: &emAvlTreeMap<K, V>) {
        if let Some(ref k) = self.key {
            self.key = map
                .data
                .range((Bound::Excluded(k.clone()), Bound::Unbounded))
                .next()
                .map(|(k, _)| k.clone());
        }
    }

    /// Retreat the cursor to the previous (smaller) key in `map`.
    pub fn SetPrev<V: Clone>(&mut self, map: &emAvlTreeMap<K, V>) {
        if let Some(ref k) = self.key {
            self.key = map.data.range(..k).next_back().map(|(k, _)| k.clone());
        }
    }

    /// Detach the cursor so that `Get` returns `None`.
    pub fn Detach(&mut self) {
        self.key = None;
    }
}

impl<K: Ord + Clone, V: Clone> emAvlTreeMap<K, V> {
    /// Return a cursor pointing at the first (smallest key) element, or a
    /// detached cursor if the map is empty.
    pub fn cursor_first(&self) -> MapCursor<K> {
        MapCursor {
            key: self.data.keys().next().cloned(),
        }
    }

    /// Return a cursor pointing at the last (largest key) element, or a
    /// detached cursor if the map is empty.
    pub fn cursor_last(&self) -> MapCursor<K> {
        MapCursor {
            key: self.data.keys().next_back().cloned(),
        }
    }

    /// Return a cursor pointing at `key`, or a detached cursor if `key` is
    /// not present in the map.
    pub fn cursor_at(&self, key: &K) -> MapCursor<K> {
        MapCursor {
            key: if self.data.contains_key(key) {
                Some(key.clone())
            } else {
                None
            },
        }
    }
}

impl<K: Ord + Clone, V: Clone> Clone for emAvlTreeMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            data: Rc::clone(&self.data),
        }
    }
}

impl<K: Ord + Clone, V: Clone> Default for emAvlTreeMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// DIVERGED: (language-forced) C++ `operator[]` creates a default entry if missing.
/// Rust panics if key not found (no `Default` bound on `V`).
impl<K: Clone + Ord, V: Clone> Index<&K> for emAvlTreeMap<K, V> {
    type Output = V;
    fn index(&self, key: &K) -> &V {
        self.GetValue(key).expect("emAvlTreeMap: key not found")
    }
}
