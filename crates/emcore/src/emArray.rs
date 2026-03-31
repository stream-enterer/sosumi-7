// emArray.rs — COW dynamic array, ported from emArray.h
//
// C++ emArray is a copy-on-write dynamic array. Copy is O(1) shallow;
// mutation deep-copies if shared. Rust equivalent wraps Vec<T> in Rc
// with clone-on-mutate via Rc::make_mut.
//
// DIVERGED: TuningLevel — omitted entirely. Rust ownership handles
// the optimization concerns that TuningLevel addressed in C++.
//
// DIVERGED: C++ overloaded methods are split into distinct names
// (Add_one, Add_fill, Add_slice, etc.) because Rust has no overloading.
//
// DIVERGED: BinarySearch returns Result<usize, usize> instead of
// C++ int (negative = bitwise-inverted insertion point). Rust's
// slice::binary_search convention is used.
//
// DIVERGED: Iterator inner class — omitted from this file. Will be
// ported separately as emArrayIterator (Task 2).
//
// DIVERGED: PointerToIndex — omitted from initial port.
//
// DIVERGED: AddNew, InsertNew, ReplaceByNew (default-value insertion
// variants) — omitted. Rust callers use Default::default() explicitly.
//
// DIVERGED: Sort() skips COW clone if array is already sorted. C++ calls
// MakeWritable() unconditionally. No behavioral difference for callers.

use std::rc::Rc;

/// Stable cursor for emArray. Tracks position by index.
///
/// DIVERGED: C++ emArray::Iterator auto-adjusts index when elements are
/// inserted/removed before the cursor position. This Rust cursor does NOT
/// auto-adjust — it maintains the original index. Full auto-adjustment
/// requires the array to track all live cursors, which adds overhead.
/// Will be implemented when a consumer requires it.
pub struct Cursor {
    /// `None` means the cursor is in the "invalid / off-the-end" state.
    index: Option<usize>,
}

impl Cursor {
    pub fn IsValid<T: Clone>(&self, array: &emArray<T>) -> bool {
        match self.index {
            Some(i) => i < array.GetCount(),
            None => false,
        }
    }

    pub fn Get<'a, T: Clone>(&self, array: &'a emArray<T>) -> Option<&'a T> {
        self.index.and_then(|i| array.data.get(i))
    }

    pub fn SetNext<T: Clone>(&mut self, array: &emArray<T>) {
        match self.index {
            Some(i) if i < array.GetCount() => self.index = Some(i + 1),
            _ => {}
        }
    }

    pub fn SetPrev<T: Clone>(&mut self, _array: &emArray<T>) {
        match self.index {
            Some(0) => self.index = None,
            Some(i) => self.index = Some(i - 1),
            None => {}
        }
    }

    pub fn SetIndex(&mut self, index: usize) {
        self.index = Some(index);
    }
}

/// COW dynamic array backed by `Rc<Vec<T>>`.
///
/// Clone is O(1) shallow (Rc::clone). Mutation triggers deep copy
/// if the Rc is shared (Rc::make_mut).
pub struct emArray<T: Clone> {
    data: Rc<Vec<T>>,
}

impl<T: Clone> Clone for emArray<T> {
    fn clone(&self) -> Self {
        emArray {
            data: Rc::clone(&self.data),
        }
    }
}

impl<T: Clone> Default for emArray<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> emArray<T> {
    // ---------------------------------------------------------------
    // Construction
    // ---------------------------------------------------------------

    pub fn new() -> Self {
        emArray {
            data: Rc::new(Vec::new()),
        }
    }

    pub fn from_slice(slice: &[T]) -> Self {
        emArray {
            data: Rc::new(slice.to_vec()),
        }
    }

    pub fn filled(obj: T, count: usize) -> Self {
        emArray {
            data: Rc::new(vec![obj; count]),
        }
    }

    // ---------------------------------------------------------------
    // Internal: COW write access
    // ---------------------------------------------------------------

    fn make_writable(&mut self) -> &mut Vec<T> {
        Rc::make_mut(&mut self.data)
    }

    // ---------------------------------------------------------------
    // Read-only methods
    // ---------------------------------------------------------------

    pub fn GetCount(&self) -> usize {
        self.data.len()
    }

    pub fn IsEmpty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get a reference to the element at `index`.
    pub fn Get_at(&self, index: usize) -> &T {
        &self.data[index]
    }

    /// Get the backing slice.
    pub fn Get(&self) -> &[T] {
        &self.data
    }

    /// Number of Rc references to the underlying data.
    pub fn GetDataRefCount(&self) -> usize {
        Rc::strong_count(&self.data)
    }

    /// Create a sub-array from elements `[index..index+count]`.
    pub fn GetSubArray(&self, index: usize, count: usize) -> emArray<T> {
        emArray {
            data: Rc::new(self.data[index..index + count].to_vec()),
        }
    }

    // ---------------------------------------------------------------
    // Mutating methods (trigger COW)
    // ---------------------------------------------------------------

    /// Set the element at `index` to `obj`.
    pub fn Set(&mut self, index: usize, obj: T) {
        self.make_writable()[index] = obj;
    }

    /// Get a mutable reference to the element at `index`.
    pub fn GetWritable(&mut self, index: usize) -> &mut T {
        &mut self.make_writable()[index]
    }

    /// Get a mutable slice of all elements.
    pub fn GetWritableSlice(&mut self) -> &mut [T] {
        self.make_writable().as_mut_slice()
    }

    /// Append a single element.
    pub fn Add_one(&mut self, obj: T) {
        self.make_writable().push(obj);
    }

    /// Append `count` copies of `obj`.
    pub fn Add_fill(&mut self, obj: T, count: usize) {
        let v = self.make_writable();
        v.reserve(count);
        for _ in 0..count {
            v.push(obj.clone());
        }
    }

    /// Append all elements from another emArray.
    pub fn Add(&mut self, array: &emArray<T>) {
        self.make_writable().extend_from_slice(&array.data);
    }

    /// Append all elements from a slice.
    pub fn Add_slice(&mut self, slice: &[T]) {
        self.make_writable().extend_from_slice(slice);
    }

    /// Set the number of elements. New elements are default-initialized.
    pub fn SetCount(&mut self, count: usize)
    where
        T: Default,
    {
        let v = self.make_writable();
        if count > v.len() {
            v.resize_with(count, T::default);
        } else {
            v.truncate(count);
        }
    }

    /// Shrink capacity to match length.
    pub fn Compact(&mut self) {
        self.make_writable().shrink_to_fit();
    }

    /// Insert a single element at `index`.
    pub fn Insert(&mut self, index: usize, obj: T) {
        self.make_writable().insert(index, obj);
    }

    /// Insert `count` copies of `obj` at `index`.
    pub fn Insert_fill(&mut self, index: usize, obj: T, count: usize) {
        let items: Vec<T> = (0..count).map(|_| obj.clone()).collect();
        self.make_writable().splice(index..index, items);
    }

    /// Insert all elements from a slice at `index`.
    pub fn Insert_slice(&mut self, index: usize, slice: &[T]) {
        self.make_writable().splice(index..index, slice.iter().cloned());
    }

    /// Insert all elements from another emArray at `index`.
    pub fn Insert_array(&mut self, index: usize, array: &emArray<T>) {
        self.Insert_slice(index, &array.data);
    }

    /// Remove `count` elements starting at `index`.
    pub fn Remove(&mut self, index: usize, count: usize) {
        let v = self.make_writable();
        v.drain(index..index + count);
    }

    /// Replace `rem_count` elements at `index` with `ins_count` copies of `obj`.
    pub fn Replace(&mut self, index: usize, rem_count: usize, obj: T, ins_count: usize) {
        let items: Vec<T> = (0..ins_count).map(|_| obj.clone()).collect();
        let v = self.make_writable();
        let end = (index + rem_count).min(v.len());
        v.splice(index..end, items);
    }

    /// Replace `rem_count` elements at `index` with elements from `slice`.
    pub fn Replace_slice(&mut self, index: usize, rem_count: usize, slice: &[T]) {
        let v = self.make_writable();
        let end = (index + rem_count).min(v.len());
        v.splice(index..end, slice.iter().cloned());
    }

    /// Extract `count` elements starting at `index`, removing them.
    pub fn Extract(&mut self, index: usize, count: usize) -> emArray<T> {
        let v = self.make_writable();
        let extracted: Vec<T> = v.drain(index..index + count).collect();
        emArray {
            data: Rc::new(extracted),
        }
    }

    /// Remove all elements.
    pub fn Clear(&mut self) {
        self.make_writable().clear();
    }

    /// Ensure the data is not shared (deep-copy if needed).
    pub fn MakeNonShared(&mut self) {
        // Rc::make_mut will clone the inner Vec if shared.
        let _ = self.make_writable();
    }
}

// ---------------------------------------------------------------
// Cursor factory methods
// ---------------------------------------------------------------

impl<T: Clone> emArray<T> {
    pub fn cursor(&self, index: usize) -> Cursor {
        Cursor { index: Some(index) }
    }

    pub fn cursor_first(&self) -> Cursor {
        Cursor { index: Some(0) }
    }

    pub fn cursor_last(&self) -> Cursor {
        Cursor {
            index: if self.data.is_empty() { None } else { Some(self.data.len() - 1) },
        }
    }
}

// ---------------------------------------------------------------
// Sort and binary search (require Ord)
// ---------------------------------------------------------------

impl<T: Clone + Ord> emArray<T> {
    /// Sort the array. Returns `true` if order changed.
    /// Uses stable sort (preserves order of equal elements, matching C++).
    pub fn Sort(&mut self) -> bool {
        // Check if already sorted.
        let is_sorted = self.data.windows(2).all(|w| w[0] <= w[1]);
        if is_sorted {
            return false;
        }
        self.make_writable().sort();
        true
    }

    /// Binary search for `obj`. Returns `Ok(index)` if found,
    /// `Err(index)` with the insertion point if not found.
    ///
    /// DIVERGED: C++ returns int (negative = ~insertion_index).
    /// Rust uses `Result<usize, usize>` matching slice::binary_search.
    pub fn BinarySearch(&self, obj: &T) -> Result<usize, usize> {
        self.data.binary_search(obj)
    }

    /// Insert `obj` maintaining sorted order (always inserts, even if duplicate).
    pub fn BinaryInsert(&mut self, obj: T) {
        let pos = match self.data.binary_search(&obj) {
            Ok(i) | Err(i) => i,
        };
        self.make_writable().insert(pos, obj);
    }

    /// Insert `obj` only if no equal element exists. Returns `true` if inserted.
    pub fn BinaryInsertIfNew(&mut self, obj: T) -> bool {
        match self.data.binary_search(&obj) {
            Ok(_) => false,
            Err(i) => {
                self.make_writable().insert(i, obj);
                true
            }
        }
    }

    /// Insert `obj` if not present, or replace existing equal element.
    pub fn BinaryInsertOrReplace(&mut self, obj: T) {
        match self.data.binary_search(&obj) {
            Ok(i) => {
                self.make_writable()[i] = obj;
            }
            Err(i) => {
                self.make_writable().insert(i, obj);
            }
        }
    }

    /// Remove the element equal to `obj`. Returns `true` if found and removed.
    pub fn BinaryRemove(&mut self, obj: &T) -> bool {
        match self.data.binary_search(obj) {
            Ok(i) => {
                self.make_writable().remove(i);
                true
            }
            Err(_) => false,
        }
    }
}

// ---------------------------------------------------------------
// Sort and binary search with custom comparators
// ---------------------------------------------------------------

impl<T: Clone> emArray<T> {
    /// Sort with a custom comparator.
    /// C++ `emArray::Sort(int(*)(const OBJ*,const OBJ*,void*), void*)`.
    pub fn Sort_by(&mut self, compare: impl FnMut(&T, &T) -> std::cmp::Ordering) {
        self.make_writable().sort_by(compare);
    }

    /// Binary search with a custom comparator.
    /// C++ custom comparator overload of `BinarySearch`.
    pub fn BinarySearch_by(
        &self,
        compare: impl FnMut(&T) -> std::cmp::Ordering,
    ) -> Result<usize, usize> {
        self.data.binary_search_by(compare)
    }

    /// Insert maintaining order defined by `compare`.
    pub fn BinaryInsert_by(
        &mut self,
        obj: T,
        mut compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
    ) {
        let pos = match self.data.binary_search_by(|probe| compare(probe, &obj)) {
            Ok(i) | Err(i) => i,
        };
        self.make_writable().insert(pos, obj);
    }

    /// Insert only if no equal element exists (per `compare`). Returns `true` if inserted.
    pub fn BinaryInsertIfNew_by(
        &mut self,
        obj: T,
        mut compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
    ) -> bool {
        match self.data.binary_search_by(|probe| compare(probe, &obj)) {
            Ok(_) => false,
            Err(i) => {
                self.make_writable().insert(i, obj);
                true
            }
        }
    }

    /// Insert or replace the matching element (per `compare`).
    pub fn BinaryInsertOrReplace_by(
        &mut self,
        obj: T,
        mut compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
    ) {
        match self.data.binary_search_by(|probe| compare(probe, &obj)) {
            Ok(i) => {
                self.make_writable()[i] = obj;
            }
            Err(i) => {
                self.make_writable().insert(i, obj);
            }
        }
    }

    /// Remove the matching element (per `compare`). Returns `true` if found.
    pub fn BinaryRemove_by(
        &mut self,
        compare: impl FnMut(&T) -> std::cmp::Ordering,
    ) -> bool {
        match self.data.binary_search_by(compare) {
            Ok(i) => {
                self.make_writable().remove(i);
                true
            }
            Err(_) => false,
        }
    }

    /// Binary search by extracted key.
    /// C++ `BinarySearchByKey`.
    pub fn BinarySearchByKey<K: Ord>(
        &self,
        key: &K,
        extract: impl Fn(&T) -> K,
    ) -> Result<usize, usize> {
        self.data.binary_search_by(|probe| extract(probe).cmp(key))
    }

    /// Find and replace the matching element (per `compare`). Returns `true` if found.
    /// C++ `BinaryReplace`.
    pub fn BinaryReplace(
        &mut self,
        obj: T,
        mut compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
    ) -> bool {
        match self.data.binary_search_by(|probe| compare(probe, &obj)) {
            Ok(i) => {
                self.make_writable()[i] = obj;
                true
            }
            Err(_) => false,
        }
    }

    /// Remove by extracted key. Returns `true` if found.
    /// C++ `BinaryRemoveByKey`.
    pub fn BinaryRemoveByKey<K: Ord>(
        &mut self,
        key: &K,
        extract: impl Fn(&T) -> K,
    ) -> bool {
        match self.data.binary_search_by(|probe| extract(probe).cmp(key)) {
            Ok(i) => {
                self.make_writable().remove(i);
                true
            }
            Err(_) => false,
        }
    }
}
