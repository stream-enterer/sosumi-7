// emCrossPtr.rs — Port of emCrossPtr.h / emCrossPtr.cpp
//
// C++ uses an intrusive linked list (emCrossPtrPrivate) so that
// BreakCrossPtrs() can walk the list and NULL every pointer in O(n).
//
// Rust design: replace the intrusive list with a shared invalidation flag
// (`Rc<Cell<bool>>`). All emCrossPtrs linked to the same emCrossPtrList
// share a clone of that flag. BreakCrossPtrs() sets the flag to false,
// which every pointer sees immediately on the next is_valid() / Get() call.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// Shared flag that an `emCrossPtrList` uses to invalidate all linked pointers
/// in one shot. When the flag is `true`, pointers are considered valid (subject
/// to the target still being alive). `BreakCrossPtrs()` sets it to `false`.
struct InvalidationFlag(Cell<bool>);

/// A weak cross-pointer that is automatically invalidated when
/// `emCrossPtrList::BreakCrossPtrs()` is called (or the list is dropped),
/// even if the target object is still alive.
///
/// Corresponds to C++ `emCrossPtr<T>`.
#[allow(non_camel_case_types)]
pub struct emCrossPtr<T> {
    target: Weak<RefCell<T>>,
    flag: Option<Rc<InvalidationFlag>>,
}

/// A list of cross-pointers targeting an object. Embedding this in a type
/// allows `emCrossPtr`s to reference that object with explicit invalidation.
///
/// Dropping the list automatically calls `BreakCrossPtrs()`.
///
/// Corresponds to C++ `emCrossPtrList`.
#[allow(non_camel_case_types)]
pub struct emCrossPtrList {
    flag: Rc<InvalidationFlag>,
}

// ---------------------------------------------------------------------------
// emCrossPtr
// ---------------------------------------------------------------------------

impl<T> emCrossPtr<T> {
    /// Construct a null pointer. Corresponds to C++ `emCrossPtr()`.
    pub fn new() -> Self {
        Self {
            target: Weak::new(),
            flag: None,
        }
    }

    /// Construct a cross-pointer linked to `target` via `list`.
    /// Corresponds to C++ `emCrossPtr(CLS * obj)` + `LinkCrossPtr`.
    pub fn from_target(target: &Rc<RefCell<T>>, list: &mut emCrossPtrList) -> Self {
        Self {
            target: Rc::downgrade(target),
            flag: Some(Rc::clone(&list.flag)),
        }
    }

    /// Return the target if the pointer is valid, `None` otherwise.
    /// Corresponds to C++ `Get()`.
    pub fn Get(&self) -> Option<Rc<RefCell<T>>> {
        if self.is_valid() {
            self.target.upgrade()
        } else {
            None
        }
    }

    /// Rebind to a different target and list.
    // DIVERGED: (language-forced) C++ Set(CLS* obj = NULL) allows null. Rust splits into
    // Set() for non-null and Reset() for null, because Rust references
    // cannot be null.
    pub fn Set(&mut self, target: &Rc<RefCell<T>>, list: &mut emCrossPtrList) {
        self.target = Rc::downgrade(target);
        self.flag = Some(Rc::clone(&list.flag));
    }

    /// Unbind (set to null). Corresponds to C++ `Reset()`.
    pub fn Reset(&mut self) {
        self.target = Weak::new();
        self.flag = None;
    }

    /// Returns `true` if the target is alive and has not been explicitly
    /// invalidated via `BreakCrossPtrs()`.
    pub fn is_valid(&self) -> bool {
        match &self.flag {
            Some(f) => f.0.get() && self.target.strong_count() > 0,
            None => false,
        }
    }
}

impl<T> Default for emCrossPtr<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for emCrossPtr<T> {
    fn clone(&self) -> Self {
        Self {
            target: self.target.clone(),
            flag: self.flag.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// emCrossPtrList
// ---------------------------------------------------------------------------

impl emCrossPtrList {
    /// Start with an empty list. Corresponds to C++ `emCrossPtrList()`.
    pub fn new() -> Self {
        Self {
            flag: Rc::new(InvalidationFlag(Cell::new(true))),
        }
    }

    /// Invalidate all cross-pointers linked to this list.
    /// Corresponds to C++ `BreakCrossPtrs()`.
    pub fn BreakCrossPtrs(&mut self) {
        // Set the current flag to false so every existing pointer sees it.
        self.flag.0.set(false);
        // Allocate a fresh flag so that any *future* `from_target` / `Set`
        // calls using this list get a new, valid flag.
        self.flag = Rc::new(InvalidationFlag(Cell::new(true)));
    }
}

impl Default for emCrossPtrList {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for emCrossPtrList {
    fn drop(&mut self) {
        self.BreakCrossPtrs();
    }
}
