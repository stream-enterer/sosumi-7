# Marker File Audit Summary

Generated: 2026-03-27

## Overview

19 marker files remain under audit: 14 `.no_rust_equivalent` files covering C++ types replaced by Rust standard library equivalents, and 5 `.rust_only` files covering Rust infrastructure with no single C++ header counterpart. emThread has been reviewed and finalized as `.no_rs` (see src/emCore/emThread.no_rs). Across remaining files, 19 open questions and 27 coverage gaps were identified.

## File Summary Table

| Marker File | Type | C++ API Size | Rust Equivalents Found | Coverage Gaps | Open Questions |
|-------------|------|-------------|----------------------|---------------|----------------|
| emAnything | no_rust_equivalent | 2 classes, ~10 methods | `Box<dyn Any>`, `downcast_ref` in emListBox, emPanel | 3 (implicit sharing, construction, extraction patterns) | 1 |
| emArray | no_rust_equivalent | 1 class + iterator, ~55 methods | `Vec<T>` pervasively (7+ representative sites) | 3 (BinaryInsert family, PointerToIndex, custom Iterator) | 0 |
| emAvlTree | no_rust_equivalent | 1 struct, ~30 macros, 1 function | `HashMap`, `SlotMap` in emContext, emPanel, emListBox | 3 (intrusive AVL system, emAvlCheck, COW/stable-iterator semantics) | 0 |
| emAvlTreeMap | no_rust_equivalent | 1 class + iterator, ~35 methods | `HashMap` (8 usage sites across codebase) | 4 (ordered access, COW, stable iterators, emFileSelectionBox dedup) | 1 |
| emAvlTreeSet | no_rust_equivalent | 1 class + iterator, ~35 methods | `HashSet` in emInputState, `BTreeSet` in emImage | 4 (ordered access, COW, stable iterators, set algebra) | 0 |
| emCrossPtr | no_rust_equivalent | 3 classes, ~15 methods | `Weak<T>` in emContext (parent/child refs) | 5 (BreakCrossPtrs timing, LinkCrossPtr, copy semantics, emBorder cache, emFileDialog) | 2 |
| emFileStream | no_rust_equivalent | 1 class, 39 methods | None in emCore (std::fs::File used directly) | 1 (entire class unported; 0 emCore consumers) | 1 |
| emList | no_rust_equivalent | 1 class + iterator + 2 free fns, ~60 methods | `Vec::sort_by` replaces emSortSingleLinkedList in emClipRects | 2 (emList template, sort free functions) | 1 |
| emOwnPtr | no_rust_equivalent | 4 classes, ~12 methods | `Option<Box<T>>` (emColorField, emEngine, emViewAnimator, emWindow) | 0 | 0 |
| emOwnPtrArray | no_rust_equivalent | 1 class, ~20 methods | `Vec<Option<Box<T>>>` or `Vec<T>` (pattern, not directly named) | 4 (compact/capacity, BinaryInsert, BinaryRemoveByKey, concrete usages) | 2 |
| emRef | no_rust_equivalent | 2 classes, ~15 methods | `Rc<T>`, `Rc<RefCell<T>>` (emScreen, emFpPlugin, emCoreConfig, emContext) | 3 (emRefTarget base, explicit Set/Reset, GetRefCount) | 1 |
| emString | no_rust_equivalent | 1 class, ~50 methods | `String`/`&str` pervasively (1042+ C++ occurrences) | 3 (Extract, SetLenGetWritable, GetWritable by index) | 1 |
| emThread | **DONE** → .no_rs | — | — | — | — |
| emTmpFile | no_rust_equivalent | 2 classes, ~10 methods | None in emCore (tempfile crate in test deps only) | 3 (emTmpFile, emTmpFileMaster, IPC cleanup; 0 emCore consumers) | 2 |
| emToolkit | no_rust_equivalent | 0 (aggregation header, 25 includes) | All 25 widget headers have corresponding .rs files | 1 (no aggregation equivalent; not needed in Rust) | 0 |
| emPainterDrawList | rust_only | N/A (no C++ equivalent) | 1 enum (37 variants), 1 struct, replay method | 0 | 1 |
| fixed | rust_only | N/A (inline C++ arithmetic) | Fixed12 newtype, 15+ methods/impls | 0 | 1 |
| rect | rust_only | N/A (C++ uses 4 separate params) | Rect (6 methods), PixelRect (1 method) | 0 | 1 |
| toolkit_images | rust_only | N/A (extracted from emBorder TkResources) | ToolkitImages (15 fields), with_toolkit_images accessor | 3 (ImgDir, ImgDirUp, ImgTunnel missing) | 2 |
| widget_utils | rust_only | N/A (extracted from duplicated C++ inline code) | 2 functions (trace_input_enabled, check_mouse_round_rect) | 0 | 2 |

## All Open Questions

### Shared-state and copy semantics
- C++ emAnything supports implicit sharing (multiple emAnything values point to same data). The Rust port uses Box<dyn Any> which is not shared. Is any C++ code path relying on shared-copy semantics of emAnything (beyond just passing it around)? -- from emAnything
- The C++ pattern allows early invalidation via BreakCrossPtrs() in destructors before full cleanup; Rust's Weak only invalidates when the last Rc drops. Is this timing difference significant for any emCore code path? -- from emCrossPtr
- Some C++ code stores emRef<emVarModel<emRef<T>>> (nested references); the Rust equivalent pattern is not verified. -- from emRef

### Unverified port completeness
- Whether emFileSelectionBox.rs has ported the deduplication logic from lines 720-721 of the C++ implementation (no HashMap usage found in that file). -- from emAvlTreeMap
- emBorder's PanelPointerCache and emFileDialog's OverwriteDialog: are these ported with Weak references or with a different pattern? -- from emCrossPtr
- emFontCache.rs uses a global OnceLock rather than the C++ model/ref pattern; does EntryArray have a Rust equivalent? -- from emOwnPtrArray
- emFpPlugin.rs exists but its internal Plugins array structure was not verified against emOwnPtrArray<emFpPlugin>. -- from emOwnPtrArray

### Encoding and byte-level semantics
- The C++ emString is byte-oriented (strlen-based, no encoding assumption). Rust String is UTF-8. Whether any emCore code relies on non-UTF-8 byte sequences in strings is not verified by this audit. -- from emString

### Future port dependencies
- When/if image file model classes (emBmp, emGif, etc.) are ported to Rust, will they need an emFileStream equivalent, or will std::io::BufReader/BufWriter + byteorder or from_le_bytes suffice? -- from emFileStream
- When/if emTmpConv is ported, will it need the full emTmpFileMaster IPC-based cleanup mechanism, or will the tempfile crate's auto-cleanup suffice? -- from emTmpFile
- emTmpFile depends on emModel and emMiniIpc. The Rust port has emMiniIpc (src/emCore/emMiniIpc.rs). Does it have sufficient functionality to support emTmpFileMaster's liveness-ping pattern if needed? -- from emTmpFile
- Will ImgDir and ImgDirUp be needed when emFileSelectionBox or emFileDialog is fully ported? -- from toolkit_images

### Performance characteristics
- emClipRects.h uses a raw linked list internally and calls emSortSingleLinkedList. The Rust emClipRects uses Vec<Rect> instead. Whether performance characteristics differ for large clip rect sets is untested. -- from emList

### Unsafe and soundness
- The unsafe Send/Sync impls for DrawOp rely on the invariant that emImage pointers remain valid between recording and replay. This is documented in comments but not enforced by the type system. -- from emPainterDrawList

### Missing fixed-point variants
- C++ also uses 24-bit fixed point in emPainter_ScTl*.cpp. Whether a Fixed24 type exists or is needed is not documented. -- from fixed

### Unused types
- PixelRect is defined but no dependents were found importing it in the current grep. It may be used via the module re-export or may be unused. -- from rect

### Resource loading gaps
- Why is Tunnel.tga present in res/toolkit/ but not loaded by ToolkitImages? -- from toolkit_images

### Code provenance
- widget_utils.rs was created during "Phase 3" restructuring. It may have previously been inline code in individual widget files, or it may have been created fresh as a deduplication during the rename. -- from widget_utils
- trace_input_enabled uses OnceLock (from std::sync). The CLAUDE.md rules say "no Arc/Mutex -- single-threaded UI tree". OnceLock is not Arc/Mutex but is a sync primitive; whether this is considered an exception is unclear. -- from widget_utils

## Cross-Cutting Observations

- Copy-on-write (COW) semantics appear in 5 of the 15 no_rust_equivalent files (emArray, emList, emString, emAvlTreeMap, emAvlTreeSet). None of these COW implementations are replicated in Rust. The Rust port uses move semantics and Clone throughout.

- Stable iterator semantics (iterators that auto-advance when elements are removed and auto-adjust on COW clone) appear in emArray, emList, emAvlTreeMap, and emAvlTreeSet. None are replicated in Rust; standard Rust iterators are used instead.

- Three no_rust_equivalent files (emFileStream, emTmpFile, emAvlTreeSet) have zero usage within C++ emCore itself. Their markers document types that exist in the C++ header set but have no active consumers in the ported scope.

- Two rust_only files (fixed.rs, rect.rs) consolidate patterns that are scattered across multiple C++ files as inline code or bare parameters. fixed.rs wraps bare `int` fixed-point arithmetic from emPainter*.cpp; rect.rs wraps the (x, y, w, h) four-parameter pattern used throughout the C++ layout API.

- widget_utils.rs and toolkit_images.rs both extract duplicated C++ code into shared Rust modules: widget_utils.rs deduplicates a hit-test formula copied across 6+ widget CheckMouse methods, and toolkit_images.rs extracts TkResources from emBorder into a standalone module.

- emPainterDrawList.rs is the only rust_only file that represents an architectural divergence from C++ rather than a code consolidation. C++ uses mutex-based concurrent panel tree traversal; Rust uses a record-replay pattern because Rc-based panel behaviors cannot cross thread boundaries.

- HashMap replaces three separate C++ data structures: emAvlTree macros (in emContext, emPanel, emListBox), emAvlTreeMap (in emFileSelectionBox), and the intrusive AVL node system. All three AVL-related no_rust_equivalent markers document types replaced by the same Rust standard library type.

- Two marker files (emOwnPtrArray, emCrossPtr) raise open questions about whether specific C++ member variables in emBorder, emFileDialog, emFontCache, and emFpPlugin have been ported with equivalent patterns. These are the most concrete verification gaps.

- Three rust_only files (fixed.rs, rect.rs, widget_utils.rs) were all created during or before the "Phase 3" module restructuring (2026-03-23) and renamed during the workspace flatten (2026-03-26), indicating they originated in earlier module layouts.
