// emArray.rs — COW dynamic array, ported from emArray.h
//
// C++ emArray is a copy-on-write dynamic array. Copy is O(1) shallow;
// mutation deep-copies if shared. Rust equivalent wraps Vec<T> in Rc
// with clone-on-mutate via Rc::make_mut.
//
// DIVERGED: (language-forced) TuningLevel — stored for API correspondence but has no effect
// on behavior. Rust ownership handles the optimization concerns that
// TuningLevel addressed in C++.
//
// DIVERGED: (language-forced) C++ overloaded methods are split into distinct names
// (Add_one, Add_fill, Add_slice, etc.) because Rust has no overloading.
//
// DIVERGED: (language-forced) BinarySearch returns Result<usize, usize> instead of
// C++ int (negative = bitwise-inverted insertion point). Rust's
// slice::binary_search convention is used.
//
// DIVERGED: (language-forced) PointerToIndex — requires pointer arithmetic into Vec backing.
//
// AddNew, InsertNew, ReplaceByNew require T: Default (separate impl block).
//
// DIVERGED: (language-forced) Sort() skips COW clone if array is already sorted. C++ calls
// MakeWritable() unconditionally. No behavioral difference for callers.

use std::cell::RefCell;
use std::rc::Rc;

/// Stable cursor for emArray. Tracks position by index.
///
/// Auto-adjusts when elements are inserted or removed before the cursor
/// position, matching C++ emArray::Iterator behavior.
pub struct Cursor {
    /// `None` means the cursor is in the "invalid / off-the-end" state.
    index: Option<usize>,
    /// Shared adjustment log from the owning emArray.
    adjustments: Rc<RefCell<Vec<(isize, usize)>>>,
    /// Number of adjustments already applied.
    last_adj_len: usize,
}

impl Cursor {
    /// Apply any pending adjustments from the array's adjustment log.
    fn apply_adjustments(&mut self) {
        let adjs = self.adjustments.borrow();
        for i in self.last_adj_len..adjs.len() {
            let (delta, at_index) = adjs[i];
            if let Some(ref mut idx) = self.index {
                if *idx >= at_index {
                    *idx = (*idx as isize + delta).max(0) as usize;
                }
            }
        }
        self.last_adj_len = adjs.len();
    }

    pub fn IsValid<T: Clone>(&mut self, array: &emArray<T>) -> bool {
        self.apply_adjustments();
        match self.index {
            Some(i) => i < array.GetCount(),
            None => false,
        }
    }

    pub fn Get<'a, T: Clone>(&mut self, array: &'a emArray<T>) -> Option<&'a T> {
        self.apply_adjustments();
        self.index.and_then(|i| array.data.get(i))
    }

    pub fn SetNext<T: Clone>(&mut self, array: &emArray<T>) {
        self.apply_adjustments();
        match self.index {
            Some(i) if i < array.GetCount() => self.index = Some(i + 1),
            _ => {}
        }
    }

    pub fn SetPrev<T: Clone>(&mut self, _array: &emArray<T>) {
        self.apply_adjustments();
        match self.index {
            Some(0) => self.index = None,
            Some(i) => self.index = Some(i - 1),
            None => {}
        }
    }

    pub fn SetIndex(&mut self, index: usize) {
        self.apply_adjustments();
        self.index = Some(index);
    }
}

/// COW dynamic array backed by `Rc<Vec<T>>`.
///
/// Clone is O(1) shallow (Rc::clone). Mutation triggers deep copy
/// if the Rc is shared (Rc::make_mut).
pub struct emArray<T: Clone> {
    data: Rc<Vec<T>>,
    /// C++ TuningLevel — stored for API correspondence, no effect on behavior.
    /// DIVERGED: (language-forced) Rust ownership model makes COW tuning unnecessary; field
    /// exists for API correspondence only.
    tuning_level: u8,
    /// Adjustment log for cursor auto-adjustment.
    /// Each entry: (delta, at_index) — cursor indices >= at_index shift by delta.
    adjustments: Rc<RefCell<Vec<(isize, usize)>>>,
}

impl<T: Clone> Clone for emArray<T> {
    fn clone(&self) -> Self {
        emArray {
            data: Rc::clone(&self.data),
            tuning_level: self.tuning_level,
            adjustments: Rc::new(RefCell::new(Vec::new())),
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
            tuning_level: 0,
            adjustments: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn from_slice(slice: &[T]) -> Self {
        emArray {
            data: Rc::new(slice.to_vec()),
            tuning_level: 0,
            adjustments: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn filled(obj: T, count: usize) -> Self {
        emArray {
            data: Rc::new(vec![obj; count]),
            tuning_level: 0,
            adjustments: Rc::new(RefCell::new(Vec::new())),
        }
    }

    // ---------------------------------------------------------------
    // Internal: COW write access
    // ---------------------------------------------------------------

    fn make_writable(&mut self) -> &mut Vec<T> {
        Rc::make_mut(&mut self.data)
    }

    /// Log a cursor adjustment. Clears the log if no cursors are alive
    /// (strong_count == 1 means only the array itself holds a reference).
    fn log_adjustment(&self, delta: isize, at_index: usize) {
        if Rc::strong_count(&self.adjustments) == 1 {
            self.adjustments.borrow_mut().clear();
        } else {
            self.adjustments.borrow_mut().push((delta, at_index));
        }
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
            tuning_level: self.tuning_level,
            adjustments: Rc::new(RefCell::new(Vec::new())),
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
        let index = self.data.len();
        self.make_writable().push(obj);
        self.log_adjustment(1, index);
    }

    /// Append `count` copies of `obj`.
    pub fn Add_fill(&mut self, obj: T, count: usize) {
        let index = self.data.len();
        let v = self.make_writable();
        v.reserve(count);
        for _ in 0..count {
            v.push(obj.clone());
        }
        self.adjustments.borrow_mut().push((count as isize, index));
    }

    /// Append all elements from another emArray.
    pub fn Add(&mut self, array: &emArray<T>) {
        let index = self.data.len();
        let count = array.data.len();
        self.make_writable().extend_from_slice(&array.data);
        self.adjustments.borrow_mut().push((count as isize, index));
    }

    /// Append all elements from a slice.
    pub fn Add_slice(&mut self, slice: &[T]) {
        let index = self.data.len();
        let count = slice.len();
        self.make_writable().extend_from_slice(slice);
        self.adjustments.borrow_mut().push((count as isize, index));
    }

    /// Set the number of elements. New elements are default-initialized.
    pub fn SetCount(&mut self, count: usize)
    where
        T: Default,
    {
        let old_len = self.data.len();
        let v = self.make_writable();
        if count > v.len() {
            v.resize_with(count, T::default);
            self.adjustments
                .borrow_mut()
                .push(((count - old_len) as isize, old_len));
        } else if count < v.len() {
            v.truncate(count);
            self.adjustments
                .borrow_mut()
                .push((-((old_len - count) as isize), count));
        }
    }

    /// Shrink capacity to match length.
    pub fn Compact(&mut self) {
        self.make_writable().shrink_to_fit();
    }

    /// Insert a single element at `index`.
    pub fn Insert(&mut self, index: usize, obj: T) {
        self.make_writable().insert(index, obj);
        self.log_adjustment(1, index);
    }

    /// Insert `count` copies of `obj` at `index`.
    pub fn Insert_fill(&mut self, index: usize, obj: T, count: usize) {
        let items: Vec<T> = (0..count).map(|_| obj.clone()).collect();
        self.make_writable().splice(index..index, items);
        self.adjustments.borrow_mut().push((count as isize, index));
    }

    /// Insert all elements from a slice at `index`.
    pub fn Insert_slice(&mut self, index: usize, slice: &[T]) {
        let count = slice.len();
        self.make_writable()
            .splice(index..index, slice.iter().cloned());
        self.adjustments.borrow_mut().push((count as isize, index));
    }

    /// Insert all elements from another emArray at `index`.
    pub fn Insert_array(&mut self, index: usize, array: &emArray<T>) {
        let count = array.data.len();
        let slice = &array.data;
        self.make_writable()
            .splice(index..index, slice.iter().cloned());
        self.adjustments.borrow_mut().push((count as isize, index));
    }

    /// Remove `count` elements starting at `index`.
    pub fn Remove(&mut self, index: usize, count: usize) {
        let v = self.make_writable();
        v.drain(index..index + count);
        self.adjustments
            .borrow_mut()
            .push((-(count as isize), index));
    }

    /// Replace `rem_count` elements at `index` with `ins_count` copies of `obj`.
    pub fn Replace(&mut self, index: usize, rem_count: usize, obj: T, ins_count: usize) {
        let items: Vec<T> = (0..ins_count).map(|_| obj.clone()).collect();
        let v = self.make_writable();
        let end = (index + rem_count).min(v.len());
        let actual_rem = end - index;
        v.splice(index..end, items);
        let delta = ins_count as isize - actual_rem as isize;
        if delta != 0 {
            self.log_adjustment(delta, index);
        }
    }

    /// Replace `rem_count` elements at `index` with elements from `slice`.
    pub fn Replace_slice(&mut self, index: usize, rem_count: usize, slice: &[T]) {
        let v = self.make_writable();
        let end = (index + rem_count).min(v.len());
        let actual_rem = end - index;
        v.splice(index..end, slice.iter().cloned());
        let delta = slice.len() as isize - actual_rem as isize;
        if delta != 0 {
            self.log_adjustment(delta, index);
        }
    }

    /// Extract `count` elements starting at `index`, removing them.
    pub fn Extract(&mut self, index: usize, count: usize) -> emArray<T> {
        let v = self.make_writable();
        let extracted: Vec<T> = v.drain(index..index + count).collect();
        self.adjustments
            .borrow_mut()
            .push((-(count as isize), index));
        emArray {
            data: Rc::new(extracted),
            tuning_level: self.tuning_level,
            adjustments: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Remove all elements.
    pub fn Clear(&mut self) {
        let count = self.data.len();
        self.make_writable().clear();
        if count > 0 {
            self.adjustments.borrow_mut().push((-(count as isize), 0));
        }
    }

    /// Ensure the data is not shared (deep-copy if needed).
    pub fn MakeNonShared(&mut self) {
        // Rc::make_mut will clone the inner Vec if shared.
        let _ = self.make_writable();
    }

    // ---------------------------------------------------------------
    // TuningLevel (API correspondence only)
    // ---------------------------------------------------------------

    /// Get the tuning level. C++ `GetTuningLevel`.
    pub fn GetTuningLevel(&self) -> u8 {
        self.tuning_level
    }

    /// Set the tuning level. No effect on behavior. C++ `SetTuningLevel`.
    pub fn SetTuningLevel(&mut self, level: u8) {
        self.tuning_level = level;
    }
}

// ---------------------------------------------------------------
// Default-insertion methods (require T: Default)
// ---------------------------------------------------------------

impl<T: Clone + Default> emArray<T> {
    /// Append a default-constructed element. C++ `AddNew`.
    pub fn AddNew(&mut self) {
        let index = self.data.len();
        self.make_writable().push(T::default());
        self.log_adjustment(1, index);
    }

    /// Insert a default-constructed element at `index`. C++ `InsertNew`.
    pub fn InsertNew(&mut self, index: usize) {
        self.make_writable().insert(index, T::default());
        self.log_adjustment(1, index);
    }

    /// Replace `count` elements starting at `index` with one default element.
    /// C++ `ReplaceByNew`.
    pub fn ReplaceByNew(&mut self, index: usize, count: usize) {
        let v = self.make_writable();
        v.drain(index..index + count);
        v.insert(index, T::default());
        let delta = 1isize - count as isize;
        if delta != 0 {
            self.log_adjustment(delta, index);
        }
    }
}

// ---------------------------------------------------------------
// Cursor factory methods
// ---------------------------------------------------------------

impl<T: Clone> emArray<T> {
    pub fn cursor(&self, index: usize) -> Cursor {
        let last_adj_len = self.adjustments.borrow().len();
        Cursor {
            index: Some(index),
            adjustments: Rc::clone(&self.adjustments),
            last_adj_len,
        }
    }

    pub fn cursor_first(&self) -> Cursor {
        let last_adj_len = self.adjustments.borrow().len();
        Cursor {
            index: Some(0),
            adjustments: Rc::clone(&self.adjustments),
            last_adj_len,
        }
    }

    pub fn cursor_last(&self) -> Cursor {
        let last_adj_len = self.adjustments.borrow().len();
        Cursor {
            index: if self.data.is_empty() {
                None
            } else {
                Some(self.data.len() - 1)
            },
            adjustments: Rc::clone(&self.adjustments),
            last_adj_len,
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
    /// DIVERGED: (language-forced) C++ returns int (negative = ~insertion_index).
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
        self.log_adjustment(1, pos);
    }

    /// Insert `obj` only if no equal element exists. Returns `true` if inserted.
    pub fn BinaryInsertIfNew(&mut self, obj: T) -> bool {
        match self.data.binary_search(&obj) {
            Ok(_) => false,
            Err(i) => {
                self.make_writable().insert(i, obj);
                self.log_adjustment(1, i);
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
                self.log_adjustment(1, i);
            }
        }
    }

    /// Remove the element equal to `obj`. Returns `true` if found and removed.
    pub fn BinaryRemove(&mut self, obj: &T) -> bool {
        match self.data.binary_search(obj) {
            Ok(i) => {
                self.make_writable().remove(i);
                self.log_adjustment(-1, i);
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
        self.log_adjustment(1, pos);
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
                self.log_adjustment(1, i);
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
                self.log_adjustment(1, i);
            }
        }
    }

    /// Remove the matching element (per `compare`). Returns `true` if found.
    pub fn BinaryRemove_by(&mut self, compare: impl FnMut(&T) -> std::cmp::Ordering) -> bool {
        match self.data.binary_search_by(compare) {
            Ok(i) => {
                self.make_writable().remove(i);
                self.log_adjustment(-1, i);
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
    pub fn BinaryRemoveByKey<K: Ord>(&mut self, key: &K, extract: impl Fn(&T) -> K) -> bool {
        match self.data.binary_search_by(|probe| extract(probe).cmp(key)) {
            Ok(i) => {
                self.make_writable().remove(i);
                self.log_adjustment(-1, i);
                true
            }
            Err(_) => false,
        }
    }
}
