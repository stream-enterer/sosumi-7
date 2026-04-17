use emcore::emAvlTreeSet::emAvlTreeSet;

#[test]
fn empty_set() {
    let s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    assert!(s.IsEmpty());
    assert_eq!(s.GetCount(), 0);
}

#[test]
fn insert_and_contains() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(42);
    assert!(s.Contains(&42));
    assert!(!s.Contains(&99));
    assert_eq!(s.GetCount(), 1);
}

#[test]
fn cow_shallow_copy() {
    let mut a: emAvlTreeSet<i32> = emAvlTreeSet::new();
    a.Insert(1);
    a.Insert(2);

    let b = a.clone();
    assert_eq!(a.GetDataRefCount(), 2);
    assert!(b.Contains(&1));
}

#[test]
fn cow_clone_on_mutate() {
    let mut a: emAvlTreeSet<i32> = emAvlTreeSet::new();
    a.Insert(1);
    a.Insert(2);

    let mut b = a.clone();
    assert_eq!(a.GetDataRefCount(), 2);

    b.Insert(3);
    assert_eq!(a.GetDataRefCount(), 1);
    assert_eq!(a.GetCount(), 2);
    assert_eq!(b.GetCount(), 3);
}

#[test]
fn ordered_access() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(10);
    s.Insert(20);
    s.Insert(30);

    assert_eq!(s.GetFirst(), Some(&10));
    assert_eq!(s.GetLast(), Some(&30));
    assert_eq!(s.GetNearestGreater(&10), Some(&20));
    assert_eq!(s.GetNearestLess(&30), Some(&20));
}

#[test]
fn set_union() {
    let mut a: emAvlTreeSet<i32> = emAvlTreeSet::new();
    a.Insert(1);
    a.Insert(2);

    let mut b: emAvlTreeSet<i32> = emAvlTreeSet::new();
    b.Insert(2);
    b.Insert(3);

    a.InsertSet(&b);
    assert_eq!(a.GetCount(), 3);
    assert!(a.Contains(&1));
    assert!(a.Contains(&2));
    assert!(a.Contains(&3));
}

#[test]
fn set_intersection() {
    let mut a: emAvlTreeSet<i32> = emAvlTreeSet::new();
    a.Insert(1);
    a.Insert(2);
    a.Insert(3);

    let mut b: emAvlTreeSet<i32> = emAvlTreeSet::new();
    b.Insert(2);
    b.Insert(3);
    b.Insert(4);

    a.Intersect(&b);
    assert_eq!(a.GetCount(), 2);
    assert!(a.Contains(&2));
    assert!(a.Contains(&3));
}

#[test]
fn set_subtraction() {
    let mut a: emAvlTreeSet<i32> = emAvlTreeSet::new();
    a.Insert(1);
    a.Insert(2);
    a.Insert(3);

    let mut b: emAvlTreeSet<i32> = emAvlTreeSet::new();
    b.Insert(2);

    a.RemoveSet(&b);
    assert_eq!(a.GetCount(), 2);
    assert!(a.Contains(&1));
    assert!(a.Contains(&3));
}

#[test]
fn from_element() {
    let s = emAvlTreeSet::from_element(42);
    assert_eq!(s.GetCount(), 1);
    assert!(s.Contains(&42));
}

#[test]
fn get_returns_reference() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(5);
    assert_eq!(s.Get(&5), Some(&5));
    assert_eq!(s.Get(&6), None);
}

#[test]
fn nearest_or_equal() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(10);
    s.Insert(20);
    s.Insert(30);

    assert_eq!(s.GetNearestGreaterOrEqual(&10), Some(&10));
    assert_eq!(s.GetNearestGreaterOrEqual(&15), Some(&20));
    assert_eq!(s.GetNearestLessOrEqual(&30), Some(&30));
    assert_eq!(s.GetNearestLessOrEqual(&25), Some(&20));
}

#[test]
fn remove_first_last() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(1);
    s.Insert(2);
    s.Insert(3);

    s.RemoveFirst();
    assert_eq!(s.GetCount(), 2);
    assert!(!s.Contains(&1));

    s.RemoveLast();
    assert_eq!(s.GetCount(), 1);
    assert!(!s.Contains(&3));
}

#[test]
fn clear() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(1);
    s.Insert(2);
    s.Clear();
    assert!(s.IsEmpty());
    assert_eq!(s.GetCount(), 0);
}

#[test]
fn make_non_shared() {
    let mut a: emAvlTreeSet<i32> = emAvlTreeSet::new();
    a.Insert(1);
    let _b = a.clone();
    assert_eq!(a.GetDataRefCount(), 2);
    a.MakeNonShared();
    assert_eq!(a.GetDataRefCount(), 1);
}

#[test]
fn equality() {
    let mut a: emAvlTreeSet<i32> = emAvlTreeSet::new();
    a.Insert(1);
    a.Insert(2);

    let mut b: emAvlTreeSet<i32> = emAvlTreeSet::new();
    b.Insert(1);
    b.Insert(2);

    assert_eq!(a, b);

    b.Insert(3);
    assert_ne!(a, b);
}

#[test]
fn default_is_empty() {
    let s: emAvlTreeSet<i32> = emAvlTreeSet::default();
    assert!(s.IsEmpty());
}

#[test]
fn self_subtraction_clears() {
    let mut a: emAvlTreeSet<i32> = emAvlTreeSet::new();
    a.Insert(1);
    a.Insert(2);

    let b = a.clone();
    a.RemoveSet(&b);
    assert!(a.IsEmpty());
}

#[test]
fn cursor_traversal() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(10);
    s.Insert(20);
    s.Insert(30);

    let mut c = s.cursor_first();
    assert_eq!(c.Get(&s), Some(&10));
    c.SetNext(&s);
    assert_eq!(c.Get(&s), Some(&20));
    c.SetNext(&s);
    assert_eq!(c.Get(&s), Some(&30));
    c.SetNext(&s);
    assert_eq!(c.Get(&s), None);
}

#[test]
fn cursor_reverse_traversal() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(10);
    s.Insert(20);
    s.Insert(30);

    let mut c = s.cursor_last();
    assert_eq!(c.Get(&s), Some(&30));
    c.SetPrev(&s);
    assert_eq!(c.Get(&s), Some(&20));
    c.SetPrev(&s);
    assert_eq!(c.Get(&s), Some(&10));
    c.SetPrev(&s);
    assert_eq!(c.Get(&s), None);
}

#[test]
fn cursor_at() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(10);
    s.Insert(20);
    s.Insert(30);

    let c = s.cursor_at(&20);
    assert_eq!(c.Get(&s), Some(&20));

    let c2 = s.cursor_at(&15);
    assert_eq!(c2.Get(&s), None);
}

#[test]
fn cursor_detach() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(10);

    let mut c = s.cursor_first();
    assert_eq!(c.Get(&s), Some(&10));
    c.Detach();
    assert_eq!(c.Get(&s), None);
}

#[test]
fn cursor_returns_none_after_element_removed() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(10);
    s.Insert(20);

    let c = s.cursor_at(&10);
    s.Remove(&10);
    assert_eq!(c.Get(&s), None);
}

#[test]
fn duplicate_insert_no_change() {
    let mut s: emAvlTreeSet<i32> = emAvlTreeSet::new();
    s.Insert(42);
    s.Insert(42);
    assert_eq!(s.GetCount(), 1);
}

#[test]
fn test_bitor_union() {
    let mut a = emAvlTreeSet::from_element(1);
    a.Insert(2);
    let mut b = emAvlTreeSet::from_element(2);
    b.Insert(3);
    let c = &a | &b;
    assert_eq!(c.GetCount(), 3);
    assert!(c.Contains(&1));
    assert!(c.Contains(&2));
    assert!(c.Contains(&3));
}

#[test]
fn test_bitand_intersection() {
    let mut a = emAvlTreeSet::from_element(1);
    a.Insert(2);
    a.Insert(3);
    let mut b = emAvlTreeSet::from_element(2);
    b.Insert(3);
    b.Insert(4);
    let c = &a & &b;
    assert_eq!(c.GetCount(), 2);
    assert!(c.Contains(&2));
    assert!(c.Contains(&3));
}

#[test]
fn test_sub_difference() {
    let mut a = emAvlTreeSet::from_element(1);
    a.Insert(2);
    a.Insert(3);
    let b = emAvlTreeSet::from_element(2);
    let c = &a - &b;
    assert_eq!(c.GetCount(), 2);
    assert!(c.Contains(&1));
    assert!(c.Contains(&3));
}

#[test]
fn test_bitor_assign() {
    let mut a = emAvlTreeSet::from_element(1);
    let b = emAvlTreeSet::from_element(2);
    a |= &b;
    assert_eq!(a.GetCount(), 2);
}
