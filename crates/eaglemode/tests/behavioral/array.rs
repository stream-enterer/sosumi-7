use emcore::emArray::emArray;

#[test]
fn empty_array() {
    let a: emArray<i32> = emArray::new();
    assert_eq!(a.GetCount(), 0);
    assert!(a.IsEmpty());
}

#[test]
fn add_and_get() {
    let mut a: emArray<i32> = emArray::new();
    a.Add_one(42);
    assert_eq!(a.GetCount(), 1);
    assert_eq!(a.Get_at(0), &42);
}

#[test]
fn cow_shallow_copy() {
    let mut a: emArray<i32> = emArray::new();
    a.Add_one(1);
    a.Add_one(2);
    a.Add_one(3);

    let b = a.clone();
    assert_eq!(a.GetDataRefCount(), 2);
    assert_eq!(b.GetDataRefCount(), 2);
    assert_eq!(b.Get_at(0), &1);
    assert_eq!(b.Get_at(1), &2);
    assert_eq!(b.Get_at(2), &3);
}

#[test]
fn cow_clone_on_mutate() {
    let mut a: emArray<i32> = emArray::new();
    a.Add_one(1);
    a.Add_one(2);

    let mut b = a.clone();
    assert_eq!(a.GetDataRefCount(), 2);

    b.Set(0, 99);

    assert_eq!(a.Get_at(0), &1);
    assert_eq!(b.Get_at(0), &99);
    assert_eq!(a.GetDataRefCount(), 1);
    assert_eq!(b.GetDataRefCount(), 1);
}

#[test]
fn cow_multiple_shares() {
    let mut a: emArray<i32> = emArray::new();
    a.Add_one(10);

    let b = a.clone();
    let c = a.clone();
    assert_eq!(a.GetDataRefCount(), 3);

    drop(b);
    assert_eq!(a.GetDataRefCount(), 2);

    drop(c);
    assert_eq!(a.GetDataRefCount(), 1);
}

#[test]
fn cursor_basic_iteration() {
    let mut a: emArray<i32> = emArray::new();
    a.Add_one(10);
    a.Add_one(20);
    a.Add_one(30);

    let mut cur = a.cursor(0);
    assert!(cur.IsValid(&a));
    assert_eq!(cur.Get(&a), Some(&10));
    cur.SetNext(&a);
    assert_eq!(cur.Get(&a), Some(&20));
    cur.SetNext(&a);
    assert_eq!(cur.Get(&a), Some(&30));
    cur.SetNext(&a);
    assert!(!cur.IsValid(&a));
}

#[test]
fn cursor_reverse_iteration() {
    let mut a: emArray<i32> = emArray::new();
    a.Add_one(10);
    a.Add_one(20);
    a.Add_one(30);

    let mut cur = a.cursor_last();
    assert_eq!(cur.Get(&a), Some(&30));
    cur.SetPrev(&a);
    assert_eq!(cur.Get(&a), Some(&20));
    cur.SetPrev(&a);
    assert_eq!(cur.Get(&a), Some(&10));
    cur.SetPrev(&a);
    assert!(!cur.IsValid(&a));
}

#[test]
fn cursor_survives_cow_clone() {
    let mut a: emArray<i32> = emArray::new();
    a.Add_one(1);
    a.Add_one(2);
    a.Add_one(3);

    let cur = a.cursor(1); // points to element "2"
    let mut b = a.clone();

    // Mutate b (triggers COW)
    b.Set(0, 99);

    // Cursor still valid against a (unchanged)
    assert_eq!(cur.Get(&a), Some(&2));
}

#[test]
fn test_sort_by_custom_comparator() {
    let mut arr = emArray::new();
    arr.Add_one(3);
    arr.Add_one(1);
    arr.Add_one(2);
    // Sort descending
    arr.Sort_by(|a, b| b.cmp(a));
    assert_eq!(arr.Get_at(0), &3);
    assert_eq!(arr.Get_at(1), &2);
    assert_eq!(arr.Get_at(2), &1);
}

#[test]
fn test_binary_search_by() {
    let mut arr = emArray::new();
    for i in [1, 3, 5, 7, 9] {
        arr.Add_one(i);
    }
    assert_eq!(arr.BinarySearch_by(|x| x.cmp(&5)), Ok(2));
    assert_eq!(arr.BinarySearch_by(|x| x.cmp(&4)), Err(2));
}

#[test]
fn test_binary_insert_by() {
    let mut arr = emArray::new();
    // Insert in reverse order
    arr.BinaryInsert_by(3, |a, b| b.cmp(a));
    arr.BinaryInsert_by(1, |a, b| b.cmp(a));
    arr.BinaryInsert_by(2, |a, b| b.cmp(a));
    assert_eq!(arr.Get_at(0), &3);
    assert_eq!(arr.Get_at(1), &2);
    assert_eq!(arr.Get_at(2), &1);
}

#[test]
fn test_binary_search_by_key() {
    let mut arr: emArray<(i32, &str)> = emArray::new();
    arr.Add_one((1, "one"));
    arr.Add_one((3, "three"));
    arr.Add_one((5, "five"));
    assert_eq!(arr.BinarySearchByKey(&3, |item| item.0), Ok(1));
    assert_eq!(arr.BinarySearchByKey(&2, |item| item.0), Err(1));
}

#[test]
fn test_binary_replace() {
    let mut arr = emArray::new();
    arr.Add_one(1);
    arr.Add_one(3);
    arr.Add_one(5);
    assert!(arr.BinaryReplace(3, |a, b| a.cmp(b)));
    assert!(!arr.BinaryReplace(4, |a, b| a.cmp(b)));
}

#[test]
fn test_binary_remove_by_key() {
    let mut arr: emArray<(i32, &str)> = emArray::new();
    arr.Add_one((1, "one"));
    arr.Add_one((3, "three"));
    arr.Add_one((5, "five"));
    assert!(arr.BinaryRemoveByKey(&3, |item| item.0));
    assert_eq!(arr.GetCount(), 2);
    assert!(!arr.BinaryRemoveByKey(&4, |item| item.0));
}
