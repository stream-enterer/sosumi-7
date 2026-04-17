use emcore::emCursor::emCursor;

#[test]
fn all_19_variants_exist() {
    let variants = [
        emCursor::Normal,
        emCursor::Invisible,
        emCursor::Wait,
        emCursor::Crosshair,
        emCursor::Text,
        emCursor::Hand,
        emCursor::ArrowN,
        emCursor::ArrowS,
        emCursor::ArrowE,
        emCursor::ArrowW,
        emCursor::ArrowNE,
        emCursor::ArrowNW,
        emCursor::ArrowSE,
        emCursor::ArrowSW,
        emCursor::ResizeNS,
        emCursor::ResizeEW,
        emCursor::ResizeNESW,
        emCursor::ResizeNWSE,
        emCursor::Move,
    ];
    assert_eq!(variants.len(), 19);
}

#[test]
fn as_str_returns_correct_names() {
    assert_eq!(emCursor::Normal.emInputKeyToString(), "Normal");
    assert_eq!(emCursor::Invisible.emInputKeyToString(), "Invisible");
    assert_eq!(emCursor::Wait.emInputKeyToString(), "Wait");
    assert_eq!(emCursor::Crosshair.emInputKeyToString(), "Crosshair");
    assert_eq!(emCursor::Text.emInputKeyToString(), "Text");
    assert_eq!(emCursor::Hand.emInputKeyToString(), "Hand");
    assert_eq!(emCursor::ArrowN.emInputKeyToString(), "ArrowN");
    assert_eq!(emCursor::ArrowS.emInputKeyToString(), "ArrowS");
    assert_eq!(emCursor::ArrowE.emInputKeyToString(), "ArrowE");
    assert_eq!(emCursor::ArrowW.emInputKeyToString(), "ArrowW");
    assert_eq!(emCursor::ArrowNE.emInputKeyToString(), "ArrowNE");
    assert_eq!(emCursor::ArrowNW.emInputKeyToString(), "ArrowNW");
    assert_eq!(emCursor::ArrowSE.emInputKeyToString(), "ArrowSE");
    assert_eq!(emCursor::ArrowSW.emInputKeyToString(), "ArrowSW");
    assert_eq!(emCursor::ResizeNS.emInputKeyToString(), "ResizeNS");
    assert_eq!(emCursor::ResizeEW.emInputKeyToString(), "ResizeEW");
    assert_eq!(emCursor::ResizeNESW.emInputKeyToString(), "ResizeNESW");
    assert_eq!(emCursor::ResizeNWSE.emInputKeyToString(), "ResizeNWSE");
    assert_eq!(emCursor::Move.emInputKeyToString(), "Move");
}

#[test]
fn display_matches_as_str() {
    let variants = [
        emCursor::Normal,
        emCursor::Invisible,
        emCursor::Wait,
        emCursor::Crosshair,
        emCursor::Text,
        emCursor::Hand,
        emCursor::ArrowN,
        emCursor::ArrowS,
        emCursor::ArrowE,
        emCursor::ArrowW,
        emCursor::ArrowNE,
        emCursor::ArrowNW,
        emCursor::ArrowSE,
        emCursor::ArrowSW,
        emCursor::ResizeNS,
        emCursor::ResizeEW,
        emCursor::ResizeNESW,
        emCursor::ResizeNWSE,
        emCursor::Move,
    ];
    for v in &variants {
        assert_eq!(format!("{v}"), v.emInputKeyToString());
    }
}

#[test]
fn cursor_is_copy_clone_eq_hash() {
    let a = emCursor::Hand;
    let b = a; // Copy
    let c = a; // Clone
    assert_eq!(a, b); // PartialEq
    assert_eq!(a, c);

    // Hash: insert into HashSet
    let mut set = std::collections::HashSet::new();
    set.insert(a);
    set.insert(b);
    assert_eq!(set.len(), 1);
}

#[test]
fn cursor_debug_format() {
    let dbg = format!("{:?}", emCursor::ResizeNWSE);
    assert!(dbg.contains("ResizeNWSE"));
}

#[test]
fn all_as_str_values_unique() {
    let variants = [
        emCursor::Normal,
        emCursor::Invisible,
        emCursor::Wait,
        emCursor::Crosshair,
        emCursor::Text,
        emCursor::Hand,
        emCursor::ArrowN,
        emCursor::ArrowS,
        emCursor::ArrowE,
        emCursor::ArrowW,
        emCursor::ArrowNE,
        emCursor::ArrowNW,
        emCursor::ArrowSE,
        emCursor::ArrowSW,
        emCursor::ResizeNS,
        emCursor::ResizeEW,
        emCursor::ResizeNESW,
        emCursor::ResizeNWSE,
        emCursor::Move,
    ];
    let mut names: Vec<&str> = variants.iter().map(|c| c.emInputKeyToString()).collect();
    let original_len = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), original_len, "as_str() values must be unique");
}
