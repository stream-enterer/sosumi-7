# C++/Rust Correspondence

Do not assume any claim in these files is true. Use them to find
where to look, not to decide what is there. Before acting on any
claim: read the actual source code it references. If you skip this
step, you will make errors that look plausible but are wrong.

## State of the port

C++ emCore has 90 headers. The Rust port has 103 .rs files (some C++
headers were split into multiple Rust files per the one-type-per-file
rule). 8 C++ headers have no .rs file — these have .no_rs marker
files. 1 Rust file has no C++ header — emPainterDrawList.rust_only
(record-replay pattern; deferred to Phase 4).

All 11 marker files contain evidence gathered by LLM agents and
reviewed by a human-LLM pair. Each marker file has three sections:
an unreviewed agent audit, a mechanically reproducible grep of
outside-emCore usage, and a reviewed summary. All 11 have reviewed
summaries as of 2026-03-28. All NOT VERIFIED items in marker files
have been closed as of Phase 4 (2026-03-28).

### Phase 1 changes (2026-03-28)

Fold-backs completed — 4 .rust_only files eliminated:
- toolkit_images.rs → folded into emBorder.rs (RUST_ONLY comment)
- widget_utils.rs → inlined into 8+ callers (RUST_ONLY comments)
- fixed.rs → folded into emPainter.rs (RUST_ONLY comment)
- rect.rs → folded into emPanel.rs (RUST_ONLY comment, PixelRect dead code removed)

New port — 1 .no_rs file eliminated:
- emCrossPtr.rs created with emCrossPtr<T> and emCrossPtrList.
  Uses shared Rc<Cell<bool>> invalidation flag (DIVERGED from C++
  intrusive linked list). emCrossPtr.no_rs deleted.

### Phase 2 changes (2026-03-28)

New ports — 4 .no_rs files eliminated:
- emArray.rs created with emArray<T> (COW array with stable cursor).
  Backed by Vec<T> wrapped in Rc for COW sharing. emArray.no_rs deleted.
  DIVERGED: Vec-backed contiguous storage vs C++ custom allocator with
  gap buffer. Cursor is an external struct (not embedded in the array
  as in C++). COW clone triggers full Vec clone (no structural sharing).
- emAvlTreeMap.rs created with emAvlTreeMap<K,V> (COW ordered map with
  stable cursor). Backed by BTreeMap<K,V> wrapped in Rc for COW sharing.
  emAvlTreeMap.no_rs deleted. DIVERGED: BTreeMap B-tree vs C++ AVL tree
  (same O(log n) complexity, different constants). Cursor is an external
  struct (not an intrusive iterator as in C++).
- emAvlTreeSet.rs created with emAvlTreeSet<T> (COW ordered set with
  stable cursor and set algebra). Backed by BTreeSet<T> wrapped in Rc
  for COW sharing. emAvlTreeSet.no_rs deleted. DIVERGED: BTreeSet B-tree
  vs C++ AVL tree. Set operations (Union, Intersection, etc.) return new
  sets rather than mutating in place.
- emList.rs created with emList<T> (COW doubly-linked list with stable
  cursor). Backed by Vec<T> (not std::collections::LinkedList).
  emList.no_rs deleted. DIVERGED: Vec-backed contiguous storage vs C++
  intrusive doubly-linked list. O(1) index access but O(n) insert/remove
  in the middle (C++ is O(1) insert/remove, O(n) index). Cursor is an
  external struct.

### Phase 3 changes (2026-03-28)

New ports — 2 .no_rs files eliminated:
- emFileStream.rs created with buffered file I/O and endian-aware
  typed read/write methods. Wraps std::fs::File with manual 8KB byte
  buffer. emFileStream.no_rs deleted. DIVERGED: File paths use
  PathBuf/&Path (not String) to handle non-UTF-8 paths. C++ mode
  strings ("rb", "wb") mapped to Rust OpenOptions.
- emTmpFile.rs created with RAII temp file/directory deletion.
  emTmpFile.no_rs deleted. DIVERGED: emTmpFileMaster deferred — the
  IPC-based singleton for crash-resilient cleanup requires deep
  emModel/emContext integration. Will be ported with emTmpConv if
  needed.

Audits completed — 3 .no_rs files remain with verified conclusions:
- emAnything.no_rs — Box<dyn Any> confirmed sufficient. All C++ usage
  (emStocks, emTestContainers) is store-retrieve-extract with no
  sharing dependence.
- emOwnPtrArray.no_rs — Vec<T> confirmed sufficient. Outside-emCore
  consumers (emAvClient, emTimeZonesModel) use BinarySearchByKey
  which is composable from stdlib binary_search_by() + insert().
- emString.no_rs — encoding audit completed. All filesystem paths in
  emCore use PathBuf/&Path. Residual to_string_lossy() in
  emFileSelectionBox is acceptable (non-UTF-8 filenames would be
  un-selectable in GUI, matching C++ behavior).

Documentation:
- emResTga.rs — DIVERGED comment added documenting that Rust embeds
  TGA assets via include_bytes!() instead of loading from disk via
  emFileStream.

Marker file updated:
- emAvlTree.no_rs updated to reflect that emAvlTreeMap.rs and
  emAvlTreeSet.rs now provide the ordered-access functionality that C++
  emAvlTree macros powered. emAvlTree.no_rs remains because the C++ type
  is a macro library with no direct Rust equivalent — Map/Set provide
  the functionality at a higher level.

### Phase 4 changes (2026-03-28)

NOT VERIFIED items closed — 6 marker files updated:
- emPainterDrawList.rust_only — DrawList safety invariant #2 verified:
  frame loop structure guarantees no tree modification between record
  and replay. RefCell/interior mutability risk: none (DrawOp contains
  only plain data and raw pointers, no Rc/RefCell).
- emThread.no_rs — 19 outside-emCore files verified, all use standard
  threading patterns mapping to std::thread/std::sync.
- emAvlTree.no_rs — HashMap ordering in emContext.rs verified (only
  iteration is GetListing debug dump, order-independent). Outside-emCore
  files all use wrappers only (no raw EM_AVL_* macros). emAvlCheck()
  has no outside-emCore callers.
- emOwnPtr.no_rs — 17 outside-emCore files verified, all use
  emOwnPtr/emOwnArrayPtr/emOwnPtrArray as owned member fields mapping
  to Option<Box<T>> / Box<T> / Vec<T>.
- emRef.no_rs — emVarModel/WatchedVar ownership change verified: no
  sharing gap. C++ shared-model pattern replaced by OnceLock (emBorder
  TkResources) or transient access (no persistent multi-consumer sharing).
- emString.no_rs — COW performance impact verified as negligible. C++
  passes strings by const reference (370 sites, 0 by-value outside emCore).
  No hot-loop O(1)-copy dependence found.

emPainterDrawList.rs resolution:
- emPainterDrawList.rs is NOT a rename of emThread. It is an
  architectural divergence caused by Rust's ownership model (Rc is
  not Send). C++ emThread is replaced by std::thread/std::sync.
  emPainterDrawList.rs replaces the rendering pipeline pattern that
  C++ emThread enabled. The .rust_only marker remains — this is
  genuinely Rust-only code with no C++ equivalent.
- Rename to emThread.rs rejected: would violate emPainter firewall
  (emPainter.rs imports emPainterDrawList) and would be misleading
  (DrawList is not a thread abstraction).

No empainter-deferred-refactors.log entries: no deferred refactors
were logged during Phases 1-3.

Call site audit (no changes needed):
- Vec usage audited across all src/emCore/*.rs files. All Vec::clone()
  sites are defensive copies for before/after comparison (emListBox
  selected_indices, emPanelTree cycle_list) or independent deep copies.
  No site depends on COW semantics. All Vec sites left as Vec.
- HashMap usage audited in emContext.rs, emPanelTree.rs, emListBox.rs,
  emGUIFramework.rs, emLinearLayout.rs, emPackLayout.rs, emProcess.rs,
  emRes.rs, emScreen.rs, emTiling.rs. All HashMap sites are used for
  O(1) key lookup only — no ordered iteration or nearest-key access.
  All HashMap sites left as HashMap.

Rendering gaps closed:
- ImgTunnel, ImgDir, ImgDirUp added to ToolkitImages in emBorder.rs.
  Dir.tga and DirUp.tga copied from C++ source.
- emTunnel.rs refactored to use shared ToolkitImages (no standalone loading).
- OverwriteDialog implemented as Option<emDialog> in emFileDialog.rs
  (DIVERGED from C++ emCrossPtr<emDialog> — no signal/cycle infrastructure).

Architectural divergence resolved:
- PanelPointerCache not implemented — Rust emBorder is a data struct,
  not a panel. DIVERGED comments added to emBorder.rs.

The evidence quality varies. Some reviewed summaries resolved their
open questions with specific source references. Others could only
narrow the question and flag what remains NOT VERIFIED. The patterns
section below captures findings that span multiple files and took
significant investigation to surface.

## Porting rules

The highest priority is scope and parity with each C++ header. Every
C++ header in include/emCore/ should have a corresponding .rs file in
src/emCore/ that covers the same public API surface. Every difference
between the C++ and Rust codebases has been found, documented, and
reviewed by a human. A .no_rs file that still contains unreviewed
claims or NOT VERIFIED items is not done — it is work in progress.

### .rs files (the goal)

1. The porting unit is the C++ header, not the individual method. If a
   header is ported, all its public API should be accounted for.

2. A .rs file that covers only part of its C++ header's API surface is
   an incomplete port, not a finished one.

### .no_rs files (justified absence)

3. A .no_rs file is acceptable only when the C++ type is fully replaced
   by Rust's type system or stdlib (emArray→Vec, emRef→Rc,
   emThread→std::thread) and the replacement covers the same use cases.

4. "Rust replaces it" is not sufficient justification if the replacement
   changes behavior. COW, stable iterators, BreakCrossPtrs timing — these
   are behavioral differences that may matter. They need to be documented
   even if there's no .rs file.

5. Zero emCore consumers does not mean zero consumers. Outside-emCore
   usage determines whether a type needs to exist in the Rust port.

6. Workarounds are not solutions. emResTga working around missing
   emFileStream doesn't close the gap.

### .rust_only files (to be eliminated or justified)

7. A .rust_only file means Rust has code with no C++ header counterpart.
   The goal is to eliminate these where possible: fold the code into an
   existing .rs file that corresponds to a C++ header, or restructure to
   match the C++ file organization.

8. A .rust_only file that remains must document what C++ code it
   corresponds to (even if scattered across multiple C++ files) and why
   it cannot be folded into the corresponding .rs file.

---

Patterns that span multiple marker files and are not visible by reading
any single file in isolation. Each pattern names the concern and lists
the files where evidence is documented.

## COW semantics

C++ copy-on-write (shared data, deep copy on mutation) appears in 5
types. 4 now have Rust ports with COW via Rc-wrapped backing stores.
1 remains stdlib-only.

- ~~emArray.no_rs~~ RESOLVED: emArray.rs with Rc<Vec<T>> COW
- ~~emList.no_rs~~ RESOLVED: emList.rs with Rc<Vec<T>> COW
- emString.no_rs — Rust String has no COW; no port planned
- ~~emAvlTreeMap.no_rs~~ RESOLVED: emAvlTreeMap.rs with Rc<BTreeMap<K,V>> COW
- ~~emAvlTreeSet.no_rs~~ RESOLVED: emAvlTreeSet.rs with Rc<BTreeSet<T>> COW

Call site audit (Phase 2): no existing Vec or HashMap call site in
emCore depends on COW behavior. The new COW types are available for
outside-emCore consumers that need them.

## Stable iterators

C++ iterators that survive mutations (auto-adjust on element removal,
auto-adjust on COW clone) appear in the same 5 types plus emAvlTree.
4 now have Rust ports with stable Cursor types that survive mutations
via index tracking and generation checks.

- ~~emArray.no_rs~~ RESOLVED: emArray.rs Cursor (index-based, generation-checked)
- ~~emList.no_rs~~ RESOLVED: emList.rs Cursor (index-based, generation-checked)
- ~~emAvlTree.no_rs~~ RESOLVED: functionality provided by emAvlTreeMap/Set cursors
- ~~emAvlTreeMap.no_rs~~ RESOLVED: emAvlTreeMap.rs MapCursor (key-based, generation-checked)
- ~~emAvlTreeSet.no_rs~~ RESOLVED: emAvlTreeSet.rs SetCursor (value-based, generation-checked)

DIVERGED: C++ cursors are intrusive (embedded in the data structure and
auto-adjusted on mutation). Rust cursors are external structs that store
a position (index or key) and a generation counter. On access, they
re-validate via generation check and re-seek if the structure was mutated.
This is O(log n) for map/set cursors and O(1) for array/list cursors
when no mutation occurred.

## Zero emCore consumers with outside-emCore usage

Types that appear unused from within emCore but are consumed by
eaglemode apps. Each file has a NOTE about this. Gaps will surface
when those apps are ported.

- ~~emFileStream.no_rs~~ RESOLVED: emFileStream.rs now ported
- ~~emAvlTreeSet.no_rs~~ RESOLVED: emAvlTreeSet.rs now ported
- ~~emTmpFile.no_rs~~ RESOLVED: emTmpFile.rs now ported

## Workaround for missing feature

Rust code that reimplements part of an unported C++ type's functionality
under a different name, without referencing the original type.

- ~~emResTga.rs decodes TGA from &[u8], working around missing emFileStream~~ RESOLVED: emFileStream.rs now ported; emResTga intentionally retains &[u8] decoder for compile-time embedded assets (DIVERGED comment in emResTga.rs)
- emFontCache.rs uses OnceLock<emImage> single atlas, replacing C++
  emOwnPtrArray<Entry> dynamic cache + emRef/emModel shared ownership
  (documented in emOwnPtrArray.no_rs and emRef.no_rs)

## Concrete rendering/feature gaps

C++ functionality with no Rust counterpart where the gap affects
visible output or user-facing features.

- ~~toolkit_images.rust_only: ImgTunnel missing~~ RESOLVED: folded into emBorder.rs ToolkitImages
- ~~toolkit_images.rust_only: ImgDir/ImgDirUp missing~~ RESOLVED: added to emBorder.rs ToolkitImages
- ~~emCrossPtr.no_rs: emBorder PanelPointerCache~~ RESOLVED: architectural divergence (Rust emBorder is data struct, not panel; DIVERGED comments in emBorder.rs)
- ~~emCrossPtr.no_rs: emFileDialog OverwriteDialog~~ RESOLVED: implemented as Option<emDialog> (DIVERGED comments in emFileDialog.rs)

No remaining rendering/feature gaps in emCore.

## Encoding risk

~~C++ emString is byte-oriented; Rust String enforces UTF-8.~~ RESOLVED:
Phase 3 encoding audit confirmed all filesystem paths in emCore use
PathBuf/&Path. Residual to_string_lossy() in emFileSelectionBox is
acceptable (documented in emString.no_rs).

## Architectural divergence chain

The threading model change and the record-replay pattern are causally
linked: panel state uses Rc (emLook.rs:22), Rc is not Send, therefore
user paint code cannot run on worker threads, therefore record-replay
was introduced.

- emThread.no_rs (threading model change)
- emPainterDrawList.rust_only (record-replay pattern)

Resolution: emPainterDrawList.rs is the consequence of this chain.
It remains as .rust_only (genuinely Rust-only, no C++ equivalent).
The relationship is documented in Phase 4 findings.

## BreakCrossPtrs timing

C++ invalidates cross pointers early in destructors (before cleanup).
The Rust port (emCrossPtr.rs) uses a shared Rc<Cell<bool>> invalidation
flag instead of C++ intrusive linked lists. Invalidation is explicit
via invalidate() rather than implicit via destructor ordering. C++
BreakCrossPtrs is called in destructors of emWindow, emView, emContext;
Rust callers must call invalidate() at equivalent points.

VERIFIED (Phase 4): In C++, code after BreakCrossPtrs in destructors
(deleting child panels, windows, contexts, VIFs) does not check cross
pointers — it operates on direct ownership links. The two C++ consumers
of emCrossPtr (emBorder::PanelPointerCache, emFileDialog::OverwriteDialog)
are accessed only during normal operation (painting, layout), never during
destruction. In the Rust port, emCrossPtr has zero consumers — both C++
usage sites were resolved with different patterns (PanelPointerCache:
not implemented, OverwriteDialog: Option<emDialog>). The timing risk is
moot. emCrossPtrList::Drop calls BreakCrossPtrs(), matching C++ behavior.

- emCrossPtr.rs (DIVERGED from C++ intrusive linked list)

## Reproducible queries

These grep commands produce structural data across marker files.
Run against ~/git/eaglemode-0.96.4/. The C++ source does not change,
so the output is stable.

### Which marker types depend on which other marker types (C++ #include)

For each .no_rs type, which other .no_rs types does its C++ header include:

```
for type in emAnything emAvlTree emOwnPtr emOwnPtrArray emRef emString emThread emToolkit; do
  includes=$(grep "#include.*emCore/" ~/git/eaglemode-0.96.4/include/emCore/${type}.h 2>/dev/null | sed 's/.*emCore\///' | sed 's/\.h.*//')
  for inc in $includes; do
    [ -f "src/emCore/${inc}.no_rs" ] && echo "  $type -> $inc"
  done
done
```

Produces: which marker types include which other marker types.
As of 2026-03-28 (post Phase 3):
  (no remaining inter-marker dependencies)

### Which eaglemode app modules use which marker types

For each .no_rs type, which app modules outside emCore reference it:

```
for type in emAnything emAvlTree emOwnPtr emOwnPtrArray emRef emString emThread emToolkit; do
  apps=$(grep -rl "$type" ~/git/eaglemode-0.96.4/include/ ~/git/eaglemode-0.96.4/src/ --include='*.h' --include='*.cpp' 2>/dev/null | grep -v "/emCore/" | sed 's|.*/include/||;s|.*/src/||' | sed 's|/.*||' | sort -u | tr '\n' ' ')
  [ -n "$apps" ] && echo "$type: $apps"
done
```

Produces: per-type list of app modules that depend on it.

### Which marker types each app module needs (inverse of above)

Tells you: if you're porting emStocks, which marker types will you encounter?

```
declare -A app_types
for type in emAnything emAvlTree emOwnPtr emOwnPtrArray emRef emString emThread emToolkit; do
  for app in $(grep -rl "$type" ~/git/eaglemode-0.96.4/include/ ~/git/eaglemode-0.96.4/src/ --include='*.h' --include='*.cpp' 2>/dev/null | grep -v "/emCore/" | sed 's|.*/include/||;s|.*/src/||' | sed 's|/.*||' | sort -u); do
    app_types[$app]="${app_types[$app]} $type"
  done
done
for app in $(echo "${!app_types[@]}" | tr ' ' '\n' | sort); do
  echo "$app:${app_types[$app]}"
done
```

Produces: per-app list of marker types it depends on.
