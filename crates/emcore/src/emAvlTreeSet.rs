// emAvlTreeSet.rs — COW ordered set, ported from emAvlTreeSet.h
//
// C++ emAvlTreeSet is a copy-on-write sorted set backed by an intrusive AVL
// tree. Rust wraps BTreeSet in Rc for COW + ordered access.
//
// DIVERGED: (language-forced) Element struct — C++ exposes `Element { Obj, AvlNode }`.
// Rust returns `&T` references or `Option<&T>` directly since there is no
// intrusive AVL node to expose.
//
// DIVERGED: (language-forced) Iterator inner class — C++ Iterator is a stable cursor with AVL
// node stack and auto-adjustment on mutation (auto-advances past removed
// elements). Rust provides SetCursor, which tracks position by value clone and
// returns None (not auto-advance) when the pointed-to element is removed.
//
// DIVERGED: (language-forced) GetWritable — omitted. C++ returns a mutable pointer with the
// caveat "must not disturb order". Rust prevents this at the API level since
// mutating elements in a sorted set can break ordering invariants.
//
// DIVERGED: (language-forced) Element-pointer overloads of Remove — omitted. C++ uses raw
// pointers to elements; Rust API uses value references instead.
//
// DIVERGED: (language-forced) begin/end — omitted. C++ provides range-based for loop support
// via begin/end returning Iterator. Rust uses SetCursor for iteration.

use std::collections::BTreeSet;
use std::ops::Add;
use std::ops::AddAssign;
use std::ops::BitAnd;
use std::ops::BitAndAssign;
use std::ops::BitOr;
use std::ops::BitOrAssign;
use std::ops::Bound;
use std::ops::Sub;
use std::ops::SubAssign;
use std::rc::Rc;

/// Copy-on-write ordered set matching C++ `emAvlTreeSet<OBJ>`.
#[derive(Debug)]
pub struct emAvlTreeSet<T: Ord + Clone> {
    data: Rc<BTreeSet<T>>,
}

impl<T: Ord + Clone> emAvlTreeSet<T> {
    // --- Construction ---

    /// Construct an empty set.
    pub fn new() -> Self {
        Self {
            data: Rc::new(BTreeSet::new()),
        }
    }

    /// Construct a set with one element.
    pub fn from_element(obj: T) -> Self {
        let mut set = BTreeSet::new();
        set.insert(obj);
        Self { data: Rc::new(set) }
    }

    // --- Read ---

    /// Ask whether this set contains an element which equals the given object.
    pub fn Contains(&self, obj: &T) -> bool {
        self.data.contains(obj)
    }

    /// Get a reference to the element which equals the given object.
    pub fn Get(&self, obj: &T) -> Option<&T> {
        self.data.get(obj)
    }

    /// Get the smallest element, or None if the set is empty.
    pub fn GetFirst(&self) -> Option<&T> {
        self.data.iter().next()
    }

    /// Get the largest element, or None if the set is empty.
    pub fn GetLast(&self) -> Option<&T> {
        self.data.iter().next_back()
    }

    /// Get the nearest element strictly greater than the given object.
    pub fn GetNearestGreater(&self, obj: &T) -> Option<&T> {
        self.data
            .range((Bound::Excluded(obj.clone()), Bound::Unbounded))
            .next()
    }

    /// Get the nearest element greater than or equal to the given object.
    pub fn GetNearestGreaterOrEqual(&self, obj: &T) -> Option<&T> {
        self.data.range(obj..).next()
    }

    /// Get the nearest element strictly less than the given object.
    pub fn GetNearestLess(&self, obj: &T) -> Option<&T> {
        self.data.range(..obj).next_back()
    }

    /// Get the nearest element less than or equal to the given object.
    pub fn GetNearestLessOrEqual(&self, obj: &T) -> Option<&T> {
        self.data
            .range((Bound::Unbounded, Bound::Included(obj.clone())))
            .next_back()
    }

    /// Ask whether this set has no elements.
    pub fn IsEmpty(&self) -> bool {
        self.data.is_empty()
    }

    /// Compute the number of elements.
    pub fn GetCount(&self) -> usize {
        self.data.len()
    }

    /// Get number of references to the data behind this set.
    pub fn GetDataRefCount(&self) -> usize {
        Rc::strong_count(&self.data)
    }

    // --- Mutate ---

    /// Insert an element. If an equal element already exists, it is not replaced.
    pub fn Insert(&mut self, obj: T) {
        Rc::make_mut(&mut self.data).insert(obj);
    }

    /// Insert all elements from another set (union).
    pub fn InsertSet(&mut self, set: &emAvlTreeSet<T>) {
        if set.IsEmpty() {
            return;
        }
        let inner = Rc::make_mut(&mut self.data);
        for item in set.data.iter() {
            inner.insert(item.clone());
        }
    }

    /// Remove the first (smallest) element.
    pub fn RemoveFirst(&mut self) {
        if let Some(obj) = self.data.iter().next().cloned() {
            Rc::make_mut(&mut self.data).remove(&obj);
        }
    }

    /// Remove the last (largest) element.
    pub fn RemoveLast(&mut self) {
        if let Some(obj) = self.data.iter().next_back().cloned() {
            Rc::make_mut(&mut self.data).remove(&obj);
        }
    }

    /// Remove the element that equals the given object. No-op if not found.
    pub fn Remove(&mut self, obj: &T) {
        if self.data.contains(obj) {
            Rc::make_mut(&mut self.data).remove(obj);
        }
    }

    /// Remove all elements that are in the given set (subtraction).
    pub fn RemoveSet(&mut self, set: &emAvlTreeSet<T>) {
        if Rc::ptr_eq(&self.data, &set.data) {
            self.Clear();
            return;
        }
        if set.IsEmpty() {
            return;
        }
        let inner = Rc::make_mut(&mut self.data);
        for item in set.data.iter() {
            inner.remove(item);
        }
    }

    /// Remove all elements not contained in the given set (intersection).
    pub fn Intersect(&mut self, set: &emAvlTreeSet<T>) {
        if Rc::ptr_eq(&self.data, &set.data) {
            return;
        }
        let inner = Rc::make_mut(&mut self.data);
        inner.retain(|item| set.data.contains(item));
    }

    /// Remove all elements.
    pub fn Clear(&mut self) {
        if !self.data.is_empty() {
            self.data = Rc::new(BTreeSet::new());
        }
    }

    /// Ensure data is not shared. Call before handing to another context.
    pub fn MakeNonShared(&mut self) {
        Rc::make_mut(&mut self.data);
    }
}

// --- Cursor ---

/// Stable cursor for emAvlTreeSet. Tracks position by cloned value.
///
/// DIVERGED: (language-forced) C++ `Iterator` auto-advances when the pointed-to element is
/// removed. This cursor returns `None` from `Get` instead. C++ `Iterator` is
/// nullified on `operator=`; this cursor holds a value copy independent of set
/// identity so it can be compared against any set with the same element type.
pub struct SetCursor<T: Clone> {
    value: Option<T>,
}

impl<T: Ord + Clone> SetCursor<T> {
    /// Return the element the cursor is pointing at, or `None` if the cursor
    /// is detached or the value no longer exists in `set`.
    pub fn Get<'a>(&self, set: &'a emAvlTreeSet<T>) -> Option<&'a T> {
        self.value.as_ref().and_then(|v| set.data.get(v))
    }

    /// Advance the cursor to the next (larger) element in `set`.
    pub fn SetNext(&mut self, set: &emAvlTreeSet<T>) {
        if let Some(ref v) = self.value {
            self.value = set
                .data
                .range((Bound::Excluded(v.clone()), Bound::Unbounded))
                .next()
                .cloned();
        }
    }

    /// Retreat the cursor to the previous (smaller) element in `set`.
    pub fn SetPrev(&mut self, set: &emAvlTreeSet<T>) {
        if let Some(ref v) = self.value {
            self.value = set.data.range(..v).next_back().cloned();
        }
    }

    /// Detach the cursor so that `Get` returns `None`.
    pub fn Detach(&mut self) {
        self.value = None;
    }
}

impl<T: Ord + Clone> emAvlTreeSet<T> {
    /// Return a cursor pointing at the first (smallest) element, or a
    /// detached cursor if the set is empty.
    pub fn cursor_first(&self) -> SetCursor<T> {
        SetCursor {
            value: self.data.iter().next().cloned(),
        }
    }

    /// Return a cursor pointing at the last (largest) element, or a
    /// detached cursor if the set is empty.
    pub fn cursor_last(&self) -> SetCursor<T> {
        SetCursor {
            value: self.data.iter().next_back().cloned(),
        }
    }

    /// Return a cursor pointing at `value`, or a detached cursor if `value`
    /// is not present in the set.
    pub fn cursor_at(&self, value: &T) -> SetCursor<T> {
        SetCursor {
            value: if self.data.contains(value) {
                Some(value.clone())
            } else {
                None
            },
        }
    }
}

// --- Trait impls ---

impl<T: Ord + Clone> Clone for emAvlTreeSet<T> {
    fn clone(&self) -> Self {
        Self {
            data: Rc::clone(&self.data),
        }
    }
}

impl<T: Ord + Clone> Default for emAvlTreeSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord + Clone> PartialEq for emAvlTreeSet<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data) || *self.data == *other.data
    }
}

impl<T: Ord + Clone> Eq for emAvlTreeSet<T> {}

impl<T: Clone + Ord> BitOr<&emAvlTreeSet<T>> for &emAvlTreeSet<T> {
    type Output = emAvlTreeSet<T>;
    fn bitor(self, rhs: &emAvlTreeSet<T>) -> emAvlTreeSet<T> {
        let mut result = self.clone();
        result.InsertSet(rhs);
        result
    }
}

impl<T: Clone + Ord> BitOrAssign<&emAvlTreeSet<T>> for emAvlTreeSet<T> {
    fn bitor_assign(&mut self, rhs: &emAvlTreeSet<T>) {
        self.InsertSet(rhs);
    }
}

impl<T: Clone + Ord> BitAnd<&emAvlTreeSet<T>> for &emAvlTreeSet<T> {
    type Output = emAvlTreeSet<T>;
    fn bitand(self, rhs: &emAvlTreeSet<T>) -> emAvlTreeSet<T> {
        let mut result = self.clone();
        result.Intersect(rhs);
        result
    }
}

impl<T: Clone + Ord> BitAndAssign<&emAvlTreeSet<T>> for emAvlTreeSet<T> {
    fn bitand_assign(&mut self, rhs: &emAvlTreeSet<T>) {
        self.Intersect(rhs);
    }
}

impl<T: Clone + Ord> Sub<&emAvlTreeSet<T>> for &emAvlTreeSet<T> {
    type Output = emAvlTreeSet<T>;
    fn sub(self, rhs: &emAvlTreeSet<T>) -> emAvlTreeSet<T> {
        let mut result = self.clone();
        result.RemoveSet(rhs);
        result
    }
}

impl<T: Clone + Ord> SubAssign<&emAvlTreeSet<T>> for emAvlTreeSet<T> {
    fn sub_assign(&mut self, rhs: &emAvlTreeSet<T>) {
        self.RemoveSet(rhs);
    }
}

impl<T: Clone + Ord> Add<T> for &emAvlTreeSet<T> {
    type Output = emAvlTreeSet<T>;
    fn add(self, rhs: T) -> emAvlTreeSet<T> {
        let mut result = self.clone();
        result.Insert(rhs);
        result
    }
}

impl<T: Clone + Ord> AddAssign<T> for emAvlTreeSet<T> {
    fn add_assign(&mut self, rhs: T) {
        self.Insert(rhs);
    }
}
