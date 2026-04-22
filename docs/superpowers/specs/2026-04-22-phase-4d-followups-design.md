# Phase 4d Follow-ups — Design Spec

**Date:** 2026-04-22
**Scope:** Three non-blocking follow-ups from the Phase 4d closeout. None touch Phase 4e
scope. Each is independently deliverable.

---

## Background

Phase 4d shipped `emRecNodeConfigModel`, `emRecListener`, and the compound
round-trip tests. Three loose ends were flagged `TODO(phase-4d-followup)` at
source sites:

1. **Auto dirty-tracking** — `emRecNodeConfigModel` uses manual
   `modify()`/`mark_unsaved()` because the `emRecListener` wire-up was deferred.
   `emRecListener` is now fully implemented and tested; this is a wire-up task.
2. **Compat test comment + assertion** — the `license_vcitem_round_trips_*` test
   comment claims C++ emits `.10434` (no leading zero) and a different color
   format. Empirical verification shows both C++ and Rust normalize identically
   (`{ 153 153 136 }` integers, `0.10434` with leading zero). The comment is
   wrong; the test only asserts values, not bytes.
3. **`mem::forget` in compound round-trip tests** — three tests leak their
   `Fixture` to suppress a scheduler-drop pending-signals assertion. A
   `EngineScheduler::abort_all_pending()` helper removes the need for the leak.

---

## Task 1 — Wire `emRecListener` into `emRecNodeConfigModel`

### What changes

**`crates/emcore/src/emRecNodeConfigModel.rs`**

Add two fields:
```rust
unsaved_flag: Rc<Cell<bool>>,   // shared with listener closure
listener: emRecListener,         // observes value.listened_signal()
```

`unsaved: bool` is replaced by `unsaved_flag`. All reads/writes go through
`unsaved_flag.get()` / `unsaved_flag.set(true)` / `unsaved_flag.set(false)`.

`new()` gains `ctx: &mut SchedCtx<'_>`:
```rust
pub fn new(value: T, install_path: PathBuf, ctx: &mut SchedCtx<'_>) -> Self {
    let unsaved_flag = Rc::new(Cell::new(false));
    let flag_cb = Rc::clone(&unsaved_flag);
    let listener = emRecListener::new(
        Some(&value),
        Box::new(move |_sc| flag_cb.set(true)),
        ctx,
    );
    Self { value, install_path, unsaved_flag, listener, format_name: None }
}
```

The `Rc<Cell<bool>>` sidesteps the self-referential closure problem: the
closure captures a clone of the flag, not `self`.

`modify()` keeps its immediate `unsaved_flag.set(true)` call — the listener
fires on the next scheduler cycle (DIVERGED per `emRecListener` module docs),
so `IsUnsaved()` would return stale for one cycle otherwise. `modify()` remains
the recommended mutation path for synchronous dirty detection.

Add `detach(&mut self, ctx: &mut SchedCtx<'_>)` for cleanup (mirrors C++
`~emRecListener` calling `SetListenedRec(NULL)`). Non-consuming so callers can
still access record fields (e.g. for signal teardown) after detaching:
```rust
pub fn detach(&mut self, ctx: &mut SchedCtx<'_>) {
    self.listener.detach_mut(ctx);
}
```

Remove the `mark_unsaved()` pub method — callers that needed it were working
around the missing auto-listener. Replace any remaining callers with `modify()`.

Remove the `DIVERGED: dirty tracking is manual` comment block and the
`TODO(phase-4d-followup)` marker.

Update the `modify()` DIVERGED comment: it no longer stands in for the
listener — it exists for synchronous immediacy.

### Call-site migration

All existing `emRecNodeConfigModel::new(cfg, path)` calls gain `&mut sc`.
Currently these are test-only (no production callers yet). Update all test
call sites in:
- `crates/emcore/src/emRecNodeConfigModel.rs` (inline tests)
- `crates/emcore/tests/emrec_config_loadandsave.rs`

All tests that previously called `teardown()` or manually called
`sc.scheduler.abort(sig)` / `sc.remove_signal(sig)` for model signals must
now also call `model.detach(&mut sc)` before drop.

### Invariants

- **I-1a.** After `model.GetRecMut().SomeField.SetValue(v, ctx)` + one scheduler
  cycle, `model.IsUnsaved()` returns `true`.
- **I-1b.** After `model.TrySave(false)`, `model.IsUnsaved()` returns `false`;
  a subsequent mutation + cycle sets it `true` again.
- **I-1c.** `mark_unsaved()` is absent from the public API.

---

## Task 2 — Fix compat test comment and add byte-level assertion

### What changes

**`crates/emcore/tests/emrec_persistence_cpp_compat.rs`**

In `license_vcitem_round_trips_field_values_through_writer`:

1. Replace the wrong comment ("C++ uses `.10434` (no leading zero) …") with an
   accurate one explaining that C++ and Rust normalize to the same bytes:
   - doubles: `%.9G` / `format_g_9` both emit `0.10434` (leading zero, glibc)
   - colors: `TryStartWriting` / `TryWrite` both emit `{ R G B }` integers
   - the original fixture uses hand-authored short-hex and no-leading-zero
     spellings that neither C++ nor Rust preserves on re-write

2. Add a byte-level assertion after re-serialization. The expected normalized
   bytes are known from empirical verification:
   ```rust
   let expected = b"{\n\
       \tTitle = \"License\"\n\
       \tPosX = 0.10434\n\
       \tPosY = 0.1\n\
       \tWidth = 0.01\n\
       \tContentTallness = 0.5\n\
       \tBackgroundColor = {153 153 136}\n\
       \tBorderColor = {102 102 102}\n\
       \tTitleColor = {238 238 255}\n\
       \tFileName = \"License.emFileLink\"\n\
       \tCopyToUser = no\n\
   }";
   assert_eq!(out_bytes.as_slice(), expected);
   ```

3. Remove the `TODO(phase-4d-followup)` marker.

### Invariants

- **I-2a.** The byte-level assertion passes (no `#[ignore]`).
- **I-2b.** The comment no longer claims C++ behavior that contradicts C++ source.

---

## Task 3 — `EngineScheduler::abort_all_pending()` and replace `mem::forget`

### What changes

**`crates/emcore/src/emScheduler.rs`**

Add to `EngineScheduler`:
```rust
/// Clear the pending-signal queue without firing the signals.
/// Used in test teardown when compound records have allocated internal
/// signals with no external handle. Not for production use.
pub fn abort_all_pending(&mut self) {
    for &id in &self.inner.pending_signals {
        if let Some(sig) = self.inner.signals.get_mut(id) {
            sig.pending = false;
        }
    }
    self.inner.pending_signals.clear();
}
```

**`crates/emcore/tests/emrec_persistence_roundtrip.rs`**

At the three `TODO(phase-4d-followup)` sites (`union_rec_roundtrip`,
`array_rec_roundtrip`, `tarray_rec_roundtrip_persons`), replace:
```rust
std::mem::forget(u);
std::mem::forget(u2);
std::mem::forget(fx);
```
with:
```rust
fx.sched.abort_all_pending();
// u, u2, fx drop normally
```

(Pattern is the same for `arr`/`arr2`/`fx` and the tarray variants.)

Remove all three `TODO(phase-4d-followup)` comments and their associated
rationale blocks.

### Why `abort_all_pending()` is sufficient

The scheduler's drop-time assertion fires only on a non-empty
`pending_signals` queue. Compound types (union, array, tarray) allocate
internal signals when their element count or active variant changes; those
signals fire but have no test handle to abort them. After
`abort_all_pending()`, the queue is empty and the scheduler drops cleanly.
The signal SlotMap entries for internal signals are orphaned but drop silently
(no assertion). The task 3 review verdict from Phase 4d ("not a production
leak") still holds — `SetVariant`/`SetCount` drop owning Boxes correctly.

### Invariants

- **I-3a.** `std::mem::forget` absent from `emrec_persistence_roundtrip.rs`.
- **I-3b.** All three tests pass without any `#[ignore]`.

---

## Cross-cutting

- All three tasks are independently mergeable in any order.
- Task 1 is the only one that changes a public API (`new()` signature, removal
  of `mark_unsaved()`). It must update all call sites atomically.
- No golden tests are affected. No production callers of
  `emRecNodeConfigModel::new` exist yet.
- Exit check: `cargo-nextest ntr` green; `cargo clippy -- -D warnings` clean.
