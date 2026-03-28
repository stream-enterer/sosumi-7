# Cross-Cutting Patterns Across Marker Files

## How to read the marker files

Each .no_rs and .rust_only file has three sections separated by labels:

  AGENT AUDIT (unreviewed — treat claims as unverified)
  OUTSIDE EMCORE (grep output — reproducible, no interpretation)
  REVIEWED SUMMARY (reviewed — unverified items marked NOT VERIFIED)

Do not trust any section uncritically.

The AGENT AUDIT was written by an LLM that read C++ headers and grepped
the Rust codebase. It contains real file paths and line numbers mixed
with interpretive claims. The line numbers were correct when written
but may have drifted. The interpretive claims ("not needed", "by design",
"no gaps") were not reviewed and may be wrong.

The OUTSIDE EMCORE section is grep output. It is mechanically reproducible
and contains no interpretation. It is the strongest section.

The REVIEWED SUMMARY was written by a human-LLM pair that verified
specific claims against source code. Items marked "verified" mean:
a grep or file read confirmed the claim at the time of writing. This
does NOT mean the claim is currently true — code changes since the
review may have invalidated it. Items marked "NOT VERIFIED" mean: the
claim was identified as unverified and left open rather than guessed at.

Before acting on any claim in any section:
  - If it names a file path or line number, read the file.
  - If it says something exists or doesn't exist, grep for it.
  - If it says behavior X happens, read the code that does X.

The marker files are a starting point for investigation, not a
substitute for it.

---

Patterns that span multiple marker files and are not visible by reading
any single file in isolation. Each pattern names the concern and lists
the files where evidence is documented.

## COW semantics not replicated

C++ copy-on-write (shared data, deep copy on mutation) appears in 5
types. Rust uses move semantics and Clone throughout. Whether any code
depends on COW behavior is NOT VERIFIED in any of these files.

- emArray.no_rs
- emList.no_rs
- emString.no_rs
- emAvlTreeMap.no_rs
- emAvlTreeSet.no_rs

## Stable iterators not replicated

C++ iterators that survive mutations (auto-adjust on element removal,
auto-adjust on COW clone) appear in the same 5 types plus emAvlTree.
Rust iterators borrow the collection immutably. Whether any code
mutates while iterating is NOT VERIFIED in any of these files.

- emArray.no_rs
- emList.no_rs
- emAvlTree.no_rs
- emAvlTreeMap.no_rs
- emAvlTreeSet.no_rs

## Zero emCore consumers with outside-emCore usage

Types that appear unused from within emCore but are consumed by
eaglemode apps. Each file has a NOTE about this. Gaps will surface
when those apps are ported.

- emFileStream.no_rs (13 outside files — all image format loaders)
- emAvlTreeSet.no_rs (4 outside files — emOsm, emStocks)
- emTmpFile.no_rs (2 outside files — emTmpConv)

## Workaround for missing feature

Rust code that reimplements part of an unported C++ type's functionality
under a different name, without referencing the original type.

- emResTga.rs decodes TGA from &[u8], working around missing emFileStream
  (documented in emFileStream.no_rs)
- emFontCache.rs uses OnceLock<emImage> single atlas, replacing C++
  emOwnPtrArray<Entry> dynamic cache + emRef/emModel shared ownership
  (documented in emOwnPtrArray.no_rs and emRef.no_rs)

## Concrete rendering/feature gaps

C++ functionality with no Rust counterpart where the gap affects
visible output or user-facing features.

- toolkit_images.rust_only: ImgTunnel missing — emTunnel rendering gap
- toolkit_images.rust_only: ImgDir/ImgDirUp missing — file selection icons
- emCrossPtr.no_rs: emBorder PanelPointerCache has no Rust counterpart
- emCrossPtr.no_rs: emFileDialog OverwriteDialog has no Rust counterpart

## Encoding risk

C++ emString is byte-oriented; Rust String enforces UTF-8. File paths
on Unix can contain non-UTF-8 bytes. This affects any code that stores
file paths in strings.

- emString.no_rs

## Architectural divergence chain

The threading model change and the record-replay pattern are causally
linked: panel state uses Rc (emLook.rs:22), Rc is not Send, therefore
user paint code cannot run on worker threads, therefore record-replay
was introduced.

- emThread.no_rs (threading model change)
- emPainterDrawList.rust_only (record-replay pattern)

## BreakCrossPtrs timing

C++ invalidates cross pointers early in destructors (before cleanup).
Rust Weak invalidates only when last Rc drops (after cleanup). Whether
any code checks a cross pointer during the target's destruction is
NOT VERIFIED.

- emCrossPtr.no_rs

## Reproducible queries

These grep commands produce structural data across marker files.
Run against ~/git/eaglemode-0.96.4/. The C++ source does not change,
so the output is stable.

### Which marker types depend on which other marker types (C++ #include)

For each .no_rs type, which other .no_rs types does its C++ header include:

```
for type in emAnything emArray emAvlTree emAvlTreeMap emAvlTreeSet emCrossPtr emFileStream emList emOwnPtr emOwnPtrArray emRef emString emThread emTmpFile emToolkit; do
  includes=$(grep "#include.*emCore/" ~/git/eaglemode-0.96.4/include/emCore/${type}.h 2>/dev/null | sed 's/.*emCore\///' | sed 's/\.h.*//')
  for inc in $includes; do
    [ -f "src/emCore/${inc}.no_rs" ] && echo "  $type -> $inc"
  done
done
```

Produces: which marker types include which other marker types.
As of 2026-03-28:
  emAvlTreeMap -> emAvlTree
  emAvlTreeSet -> emAvlTree
  emFileStream -> emOwnPtr
  emOwnPtrArray -> emArray

### Which eaglemode app modules use which marker types

For each .no_rs type, which app modules outside emCore reference it:

```
for type in emAnything emArray emAvlTree emAvlTreeMap emAvlTreeSet emCrossPtr emFileStream emList emOwnPtr emOwnPtrArray emRef emString emThread emTmpFile emToolkit; do
  apps=$(grep -rl "$type" ~/git/eaglemode-0.96.4/include/ ~/git/eaglemode-0.96.4/src/ --include='*.h' --include='*.cpp' 2>/dev/null | grep -v "/emCore/" | sed 's|.*/include/||;s|.*/src/||' | sed 's|/.*||' | sort -u | tr '\n' ' ')
  [ -n "$apps" ] && echo "$type: $apps"
done
```

Produces: per-type list of app modules that depend on it.

### Which marker types each app module needs (inverse of above)

Tells you: if you're porting emStocks, which marker types will you encounter?

```
declare -A app_types
for type in emAnything emArray emAvlTree emAvlTreeMap emAvlTreeSet emCrossPtr emFileStream emList emOwnPtr emOwnPtrArray emRef emString emThread emTmpFile emToolkit; do
  for app in $(grep -rl "$type" ~/git/eaglemode-0.96.4/include/ ~/git/eaglemode-0.96.4/src/ --include='*.h' --include='*.cpp' 2>/dev/null | grep -v "/emCore/" | sed 's|.*/include/||;s|.*/src/||' | sed 's|/.*||' | sort -u); do
    app_types[$app]="${app_types[$app]} $type"
  done
done
for app in $(echo "${!app_types[@]}" | tr ' ' '\n' | sort); do
  echo "$app:${app_types[$app]}"
done
```

Produces: per-app list of marker types it depends on.
