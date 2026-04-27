# B-015-polling-emcore-plus — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-015-polling-emcore-plus.md`
**Pattern:** P-006-polling-accessor-present
**Scope:** emcore (`emColorField`, `emFilePanel`) + emmain singleton (`emMainPanel`), 10 rows
**Mechanical-vs-judgement:** mechanical-heavy. Eight rows collapse into one `emColorField::Cycle` rewrite (one D-006 init block + N-fold child-signal subscribes, mirroring C++ `AutoExpand` exactly). Two rows are independent panel-level rewrites (`emFilePanel::SetFileModel` and `emMainPanel::Cycle`).

## Goal & scope

Replace per-Cycle polling with subscribe-driven Cycle reactions across the ten P-006 rows, applying **D-005-poll-replacement-shape** (direct subscribe, react in Cycle) wired through **D-006-subscribe-shape** (first-Cycle init for the panel-side rows; Acquire-time subscribe for `SetFileModel`). The polling code in each Cycle stays — what changes is *when* Cycle runs (now scheduled by the subscribed signal instead of fired every frame).

Three logical sub-targets:

1. **`emColorField::Cycle` (rows -245, -255, -265, -277, -288, -298, -308, -320).** Eight rows for one consumer. C++ `AutoExpand` calls `AddWakeUpSignal(child->GetValueSignal())` for each of seven `emScalarField` children plus `AddWakeUpSignal(child->GetTextSignal())` for the Name `emTextField`. Rust currently polls all eight via `sync_from_children` per Cycle. Per D-005 open question, the bucket sketch defaults to mirror C++: subscribe individually to each child's value/text signal.

2. **`emFilePanel::SetFileModel` (row emFilePanel-50).** C++ pairs `AddWakeUpSignal(fm->GetFileStateSignal())` / `RemoveWakeUpSignal(...)` on (un)set. Rust `SetFileModel` only stores the model handle and never subscribes; `cycle_inner` then re-reads model state every frame. Subscribe inside `SetFileModel` matches C++ structure exactly.

3. **`emMainPanel::Cycle` (row emMainPanel-68).** C++ `AddWakeUpSignal(SliderTimer.GetSignal())` in ctor + `IsSignaled(SliderTimer.GetSignal())` in Cycle. Rust replaces the `emTimer` with `slider_hide_timer_start: Option<Instant>` polled every Cycle, and `Cycle` returns `false` so the panel is not re-woken once the timer expires (the panel relies on whatever else schedules it, which is incidental). Replace with an `emTimer` instance and a wake-up subscribe to the timer's signal.

## Decisions cited

- **D-005-poll-replacement-shape** — primary citation for the reaction model. Direct subscribe (consumer collapses into the `IsSignaled` branch in Cycle). The N-fold open question for `emColorField` (line 113 of `decisions.md`) resolves here per the bucket sketch's default: **mirror C++ — subscribe individually to each child's value/text signal**, confirmed by C++ `AutoExpand` (`emColorField.cpp:245,255,265,277,288,298,308,320`) which issues eight separate `AddWakeUpSignal` calls. No aggregated child-changed signal exists in C++; introducing one in Rust would diverge structurally.
- **D-006-subscribe-shape** — wiring shape for the two panel-side consumers (`emColorField`, `emMainPanel`). First-Cycle init block + cached `SignalId`s + `IsSignaled` branches at the top of Cycle. The `emColorField` init block is conditional on `self.expansion.is_some()` because the eight child signals only exist while the expansion children exist.
- **D-006-subscribe-shape, override clause** — the `emFilePanel::SetFileModel` row uses **deferred-queue / direct subscribe at SetFileModel time**, not first-Cycle init. Mirrors C++ exactly: C++ subscribes inside `SetFileModel` (not in Cycle), because the file-state signal identity changes every time the panel's bound model changes. First-Cycle init would be observably wrong if the model is swapped after the first Cycle. This is the D-006 §"per-bucket override" path; documented here, no global D-### update needed because the override is local and follows directly from the C++ structure of the row.

D-001, D-002, D-003, D-004 do not apply (no type-mismatch accessors, no rc-shim consumers, no gap-blocked rows, no stocks panels in scope).

## Audit-data corrections

The bucket sketch lists all 10 rows as `accessor present`. Re-validation (per the B-006/B-007/B-008 precedent of verifying every accessor claim) confirms:

| Row | Claimed accessor | Verified at | Status |
|---|---|---|---|
| emColorField-{245..308} | `emScalarField::GetValueSignal()` | `crates/emcore/src/emScalarField.rs:115` (`pub value_signal: SignalId`, allocated `:142`, fired `:200`) | **Present** |
| emColorField-320 | `emTextField::GetTextSignal()` | `crates/emcore/src/emTextField.rs:132` (`pub text_signal: SignalId`, allocated `:207`, fired `:235`) | **Present** |
| emFilePanel-50 | `FileModel::GetFileStateSignal()` | `crates/emcore/src/emFileModel.rs:47,63,169` (trait method on `FileModelState` + impl on `emFileModel<T>`) | **Present** |
| emMainPanel-68 | `emTimer::GetSignal()` (timer infra exists) | `crates/emcore/src/emTimer.rs:34` (`create_timer(signal_id)`), `crates/emcore/src/emScheduler.rs:402-419` (sched-ctx surface: `create_timer`/`start_timer`/`restart_timer`) | **Present** (signal allocated by caller, attached to timer via `create_timer`) |

No stale gap-blocked tags. No row needs reclassification.

**One sketch ambiguity worth flagging.** The bucket-sketch row `emMainPanel-68` says "C++ line 68" but C++ `emMainPanel.cpp:68` is `AddWakeUpSignal(SliderTimer.GetSignal())`. The bucket-sketch's note ("emTimer exists; emMainPanel uses wall-clock polling, Cycle returns false so panel is not re-woken") aligns with this row only — not with line 67 (EOI signal, owned by B-008) or line 69 (window flags signal, also owned by B-008). The sketch is correct; flagging because B-008 already touches the surrounding lines, so the implementer must merge B-015's row-68 changes with whatever B-008 leaves in `emMainPanel::Cycle`. **Cross-bucket prereq edge B-008 → B-015 (row 68 only)** to avoid merge conflict on the same `Cycle` body and the same `subscribed_init` field.

## Per-row design

### emColorField (rows -245, -255, -265, -277, -288, -298, -308, -320) — child-signal subscribes

**C++ ref:** `src/emCore/emColorField.cpp:245,255,265,277,288,298,308,320` (eight `AddWakeUpSignal` calls in `emColorField::AutoExpand`); reactions in `emColorField::Cycle` body (`emColorField.cpp:200-211`).

**Rust target:** `crates/emcore/src/emColorField.rs:271` (`pub fn Cycle`); subscribe call site is logically `AutoExpand` (`:232`) but currently `auto_expand` has no engine-context access.

**Accessor status:** Present (see audit-data table).

**Subscribe shape — D-006 first-Cycle init, expansion-gated:**

The eight child signals are *only* meaningful while the `Expansion` children exist. The C++ pairs `AutoExpand` (`AddWakeUpSignal`) with `AutoShrink` (children destroyed → `AddWakeUpSignal`s implicitly invalidated). The Rust port must mirror this lifecycle.

Plan: extend the existing first-Cycle init pattern (per D-006) but predicate on expansion presence and re-arm whenever the expansion is recreated.

```rust
// New fields on emColorField:
subscribed_to_children: bool,
sf_signals: [Option<SignalId>; 7], // r,g,b,a,h,s,v
tf_name_signal: Option<SignalId>,
```

In `Cycle`, before the existing change-detection body:

```rust
pub fn Cycle(&mut self, ctx: &mut PanelCtx<'_>) -> bool {
    if self.expansion.is_none() {
        // No children, no signals to observe. Reset for next AutoExpand.
        self.subscribed_to_children = false;
        return false;
    }

    if !self.subscribed_to_children {
        self.connect_child_signals(ctx);
        self.subscribed_to_children = true;
    }

    // Existing sync_from_children + change-detection body unchanged.
    self.sync_from_children(ctx);
    // ... (existing rgba_changed / hsv_changed / text_changed logic) ...
}
```

`connect_child_signals` walks the same children path as `sync_from_children` (root → `emRasterLayout` → 8 grandchildren), reads each child's `value_signal` (or `text_signal` for `n`), caches it into `sf_signals` / `tf_name_signal`, and calls `ectx.connect(sig, ectx.id())` for each. Mirrors C++ `AutoExpand`'s eight `AddWakeUpSignal` calls.

**`AutoShrink` change:** clear `subscribed_to_children = false` and zero out the cached signals. Mirrors C++ implicit invalidation when children destruct.

```rust
fn auto_shrink(&mut self) {
    self.expansion = None;
    self.subscribed_to_children = false;
    self.sf_signals = [None; 7];
    self.tf_name_signal = None;
}
```

**Reaction:** unchanged. The existing Cycle body already handles all eight signals' effects (`sync_from_children` + `rgba_changed`/`hsv_changed`/`text_changed`/`UpdateRGBAOutput`/etc.). The wins from this bucket: Cycle is no longer invoked every frame; it's invoked only when one of the eight subscribed signals fires (i.e., when a child widget value changes). The C++ pattern of "any IsSignaled fires → re-read all → branch" is preserved exactly.

**No `IsSignaled` branches needed** — the existing body re-reads all child values and compares against cached `*_out` fields, so per-signal branching would be redundant work. This matches C++ `emColorField::Cycle` (`emColorField.cpp:115-211`), which similarly does not switch on which specific child fired; it re-reads all and recomputes. The subscribe gates *whether* Cycle runs; the body decides what changed.

**Why one row group, not eight separate fixes:** the eight rows share one `Cycle` body, one `AutoExpand` site, and one `AutoShrink` site. Splitting them would force eight intermediate states where `emColorField` subscribes to some children but not others — no observable benefit, more churn. Land as one commit covering all eight rows.

### emFilePanel-50 — `SetFileModel` Add/Remove pair

**C++ ref:** `src/emCore/emFilePanel.cpp:48,50` — `if (fm) RemoveWakeUpSignal(fm->GetFileStateSignal());` then `if (fileModel) AddWakeUpSignal(fileModel->GetFileStateSignal());`. Reaction in C++ `emFilePanel::Cycle` is the file-state poll → `Signal(VirFileStateSignal)` cascade.

**Rust target:** `crates/emcore/src/emFilePanel.rs:74` (`pub fn SetFileModel`).

**Accessor status:** Present. `FileModelState::GetFileStateSignal` trait method (`emFileModel.rs:47,63`) is implemented by `emFileModel<T>` (`:169`).

**Subscribe shape — D-006 override, subscribe-at-SetFileModel-time:**

C++ subscribes at `SetFileModel` time, not in Cycle, because the signal identity changes whenever the bound model changes. First-Cycle init would be wrong (would bind to whichever model is set at first Cycle and never re-bind on swap).

The challenge: `SetFileModel` currently has signature `pub fn SetFileModel(&mut self, model: ...)` — no engine context. To call `connect`/`disconnect`, the signature must accept an engine context. Two options:

- **Option A.** Thread `&mut SchedCtx` (or `&mut EngineCtx`) through `SetFileModel`. Touches every caller. Rust `SetFileModel` is currently called from `emFilePanel::new`-ish paths and tests; the call-site count is bounded.
- **Option B.** Cache the new model and a pending-subscribe flag, then connect/disconnect lazily on first Cycle after a model swap. Adds state, semantically equivalent to A only if no consumer relies on the connection being live before the next Cycle.

**Pick A.** The C++ semantics ("connect synchronously at SetFileModel time so the next signal arrival is observed") are observable: between `SetFileModel(new)` and the next signal fire, an out-of-band `Cycle` invocation must not miss the signal. Lazy connection (B) introduces a one-Cycle window where the panel is bound to the model but not subscribed — observable drift, no forced category. Mirror C++ structure.

**Wiring:**

```rust
pub fn SetFileModel(
    &mut self,
    sched: &mut SchedCtx<'_>,
    panel_engine_id: EngineId,
    new_model: Option<Rc<RefCell<dyn FileModelState>>>,
) {
    // Disconnect old model's signal, if any.
    if let Some(old_rc) = &self.model {
        let old_sig = old_rc.borrow().GetFileStateSignal();
        sched.disconnect(old_sig, panel_engine_id);
    }

    self.model = new_model;

    // Connect new model's signal, if any.
    if let Some(new_rc) = &self.model {
        let new_sig = new_rc.borrow().GetFileStateSignal();
        sched.connect(new_sig, panel_engine_id);
    }

    self.last_vir_file_state = self.compute_vir_file_state();
}
```

The `panel_engine_id` argument is whatever `EngineId` the owning `emFilePanel`'s engine carries. Existing call sites must pass it (look up from the panel-tree if not at hand).

**Cycle body change:** none required for the subscribe-shape — the existing `cycle_inner` (`emFilePanel.rs:138`) re-reads `compute_vir_file_state` and compares; that body becomes a `IsSignaled`-gated branch only as an optimization. For minimum-diff fidelity, leave the body as-is. The win from this bucket is Cycle is no longer invoked unless the model's file-state signal fires.

### emMainPanel-68 — replace `slider_hide_timer_start: Option<Instant>` with `emTimer` subscribe

**C++ ref:** `src/emMain/emMainPanel.cpp:68` (`AddWakeUpSignal(SliderTimer.GetSignal())` in ctor); `:121-123` (`IsSignaled(SliderTimer.GetSignal())` in Cycle → `Slider->SetHidden(true)`).

**Rust target:** `crates/emmain/src/emMainPanel.rs:349` (the `slider_hide_timer_start: Option<Instant>` field), `:391` (initialization), `:524` (timer-start site in `update_slider_hiding`), `:663-669` (the polling block in `Cycle`).

**Accessor status:** Present. `TimerCentral::create_timer(signal_id)` at `emTimer.rs:34`; `SchedCtx::create_timer/start_timer/restart_timer` at `emScheduler.rs:402-419`.

**Subscribe shape — D-006 first-Cycle init for the timer's signal:**

C++ creates `SliderTimer` as a member, connects its signal in ctor. Rust port needs:

1. A `SignalId` allocated for the slider-timer (call it `slider_timer_signal`).
2. A `TimerId` returned from `sched.create_timer(slider_timer_signal)`.
3. A first-Cycle init block (D-006) that: allocates the signal via `ectx.create_signal()` if not already (or moves to `new()` which has access to a `ConstructCtx` that exposes `create_signal`), creates the timer, connects the timer signal to the panel engine.
4. The existing `update_slider_hiding(to_hide=true)` site (`:522-524`) calls `sched.start_timer(slider_timer_id, 5000, false)` instead of recording `Instant::now()`.
5. The existing `update_slider_hiding(to_hide=false)` reset site (`:519-520`) calls `sched.cancel_timer(slider_timer_id, true)` instead of clearing `slider_hide_timer_start`.
6. The Cycle polling block (`:663-669`) becomes `if let Some(sig) = self.slider_timer_signal && ectx.IsSignaled(sig) { self.slider_hidden = true; }`.

**Field changes:**

```rust
// Removed:
// slider_hide_timer_start: Option<Instant>,

// Added:
slider_timer_signal: Option<SignalId>,
slider_timer_id: Option<TimerId>,
subscribed_init: bool, // shared with B-008 rows 67/69
```

**Cross-bucket merge note (B-008 → B-015):** B-008 introduces the same `subscribed_init: bool` field for rows 67 and 69. The B-015 row-68 changes share that field (one init block covers EOI + flags + slider-timer signals). The B-015 implementer must land *after* B-008's panel-level changes have merged (or merge them concurrently and resolve conflicts in `emMainPanel::Cycle` and the field list). This is the only cross-bucket dependency.

**Wiring (post-B-008 merged form, illustrative):**

```rust
fn Cycle(
    &mut self,
    ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
    ctx: &mut PanelCtx,
) -> bool {
    if !self.subscribed_init {
        let eid = ectx.id();
        let mut sched = ectx.as_sched_ctx();

        // (B-008 row 67) EOI subscribe.
        // (B-008 row 69) Window flags subscribe.

        // (B-015 row 68) Slider timer.
        let sig = sched.create_signal_for_timer(); // or whatever the API is
        let tid = sched.create_timer(sig);
        sched.connect(sig, eid);
        self.slider_timer_signal = Some(sig);
        self.slider_timer_id = Some(tid);

        self.subscribed_init = true;
    }

    // (B-008 row 67) IsSignaled(eoi) branch.
    // (B-008 row 69) IsSignaled(flags) branch.

    // (B-015 row 68) Slider timer fired → hide.
    if let Some(sig) = self.slider_timer_signal
        && ectx.IsSignaled(sig)
    {
        self.slider_hidden = true;
    }

    // ... existing slider-state propagation + drag/double-click handling ...
    false
}
```

`update_slider_hiding` becomes:

```rust
pub fn update_slider_hiding(&mut self, sched: &mut SchedCtx<'_>, ...) {
    // ... existing arm/disarm logic ...
    if self.slider_hidden {
        self.slider_hidden = false;
        if let Some(tid) = self.slider_timer_id {
            sched.cancel_timer(tid, true);
        }
    }
    if to_hide && !self.slider_hidden {
        if let Some(tid) = self.slider_timer_id {
            sched.restart_timer(tid, 5000, false);
        }
    }
}
```

**Open implementer detail:** `update_slider_hiding` currently does not take a `SchedCtx` — its callers must thread one through. If this is invasive, the alternative is to record a "pending arm/cancel" intent on `self` and process it in the next Cycle (similar to B-007's lazy-engine-registration pattern). Either preserves observable behavior. Mirror-C++-structure preference favors the synchronous `SchedCtx` thread-through.

## Wiring-shape application (D-006)

Three application points, summarized:

- **`emColorField::Cycle`** — D-006 first-Cycle init, predicated on `self.expansion.is_some()`, re-armed on every `AutoExpand` cycle (because expansion children come and go). No per-signal `IsSignaled` branches; the existing body handles all eight uniformly.
- **`emFilePanel::SetFileModel`** — D-006 override path. Subscribe at SetFileModel-time, not Cycle-time. Disconnect old, connect new, every model swap.
- **`emMainPanel::Cycle`** — D-006 first-Cycle init (shared `subscribed_init` flag with B-008 rows 67/69). Add timer signal, register timer, connect signal. `IsSignaled` branch in Cycle.

## Verification strategy

C++ → Rust observable contract: each polled re-read becomes a signal-driven Cycle invocation; observable behavior identical (state mutation matches; what changes is invocation timing → no longer per-frame).

**Pre-fix observable behavior:**
- emColorField: typing into the Hex Name field updates the color, but only because Cycle runs every frame (defensive polling). With Cycle returning `false` and no other wake source on the panel, future scheduler optimizations could starve the polling. The drift is "live behavior accidentally works due to over-scheduling."
- emFilePanel: file-state changes on the model trigger no panel update unless something else wakes the panel; `cycle_inner` does the right computation but only when invoked.
- emMainPanel: slider auto-hide only fires when something else wakes the panel within the 5-second window; if the user is idle (no input), the timer never trips because `Cycle` never runs.

**Post-fix observable behavior:** all three sub-targets fire their existing reactions on signal arrival, independent of any other wake source.

**New test file:** `crates/emcore/tests/polling_b015.rs` (rows -245..-320, -50) and additions to existing emmain test file or new `crates/emmain/tests/polling_b015.rs` (row -68). RUST_ONLY: dependency-forced — no C++ test analogue, mirrors B-005/B-006/B-007/B-008 test rationale.

Test pattern per sub-target:

```rust
// emColorField — fire one child's value signal, assert Cycle reaction recomputes color.
let mut h = Harness::new();
let cf = h.create_color_field_with_expansion();
let sf_red_sig = h.find_child_value_signal(cf, "r");
{
    let exp = cf.borrow_mut().expansion_mut().unwrap();
    exp.sf_red = 5000; // mutate child value
}
h.fire(sf_red_sig);
h.run_cycle();
assert_eq!(cf.borrow().color.GetRed(), 127); // ~5000/10000 * 255

// emFilePanel — bind model, fire FileStateSignal, assert vfs recomputed.
let mut h = Harness::new();
let panel = h.create_file_panel();
let model = h.make_test_model(FileState::Loading { progress: 0.0 });
panel.borrow_mut().SetFileModel(h.sched(), panel_eid, Some(model.clone()));
model.borrow_mut().set_state(FileState::Loaded);
h.fire(model.borrow().GetFileStateSignal());
h.run_cycle();
assert!(matches!(panel.borrow().GetVirFileState(), VirtualFileState::Loaded));

// emFilePanel — swap model, assert old signal disconnected, new one connected.
let model2 = h.make_test_model(FileState::Waiting);
panel.borrow_mut().SetFileModel(h.sched(), panel_eid, Some(model2.clone()));
h.fire(model.borrow().GetFileStateSignal()); // old signal — must NOT wake panel
h.run_cycle_count_wakes(panel_eid); // assert == 0
h.fire(model2.borrow().GetFileStateSignal());
h.run_cycle_count_wakes(panel_eid); // assert == 1

// emMainPanel — arm slider timer, advance time, assert hidden flips.
let mut h = Harness::new();
let panel = h.create_main_panel();
panel.borrow_mut().update_slider_hiding(h.sched(), /* to_hide */ true, /* enabled */ true);
h.advance_time(Duration::from_millis(5001));
h.run_cycle();
assert!(panel.borrow().slider_hidden);
```

**Four-question audit-trail evidence per sub-target:** (1) signal connected? — D-006 init / SetFileModel connect call. (2) Cycle observes? — `IsSignaled` branch (or, for emColorField, body re-reads all child values). (3) reaction fires documented mutator? — assertions above. (4) C++ branch order preserved? — code review against C++ `emColorField::Cycle`, C++ `emFilePanel::SetFileModel`, C++ `emMainPanel::Cycle` line ranges.

## Implementation sequencing

1. **B-008 lands first** (cross-bucket prereq for row -68 only). The slider-timer subscribe shares `subscribed_init: bool` with B-008's EOI/flags subscribes, and shares the `emMainPanel::Cycle` body. Without B-008 first, the implementer is rewriting the same Cycle twice. emColorField (rows -245..-320) and emFilePanel (row -50) are independent of B-008 and can land in any order.
2. **emColorField — eight rows in one commit.** Add `subscribed_to_children`, `sf_signals`, `tf_name_signal` fields; extend Cycle with first-Cycle init gated on expansion presence; clear flags in `auto_shrink`. Add tests covering all 8 child signals.
3. **emFilePanel — row -50 in one commit.** Change `SetFileModel` signature to accept `&mut SchedCtx` + `EngineId`; implement disconnect-then-connect logic; update all callers. Add tests covering subscribe + model-swap.
4. **emMainPanel — row -68 in one commit (post-B-008).** Replace `slider_hide_timer_start` with `slider_timer_signal` + `slider_timer_id`; allocate signal + timer + connect in shared first-Cycle init block; replace polling check with `IsSignaled` branch; thread `SchedCtx` through `update_slider_hiding`. Add test covering arm + advance-time + signal-driven hide.
5. **Working-memory reconciliation:** mark all 10 rows resolved in `inventory-enriched.json`; flip B-015 status `pending → designed → merged` as commits land; record B-008 → B-015 prereq edge in `work-order.md` DAG.

Steps 2 and 3 are independent and can land in either order. Step 4 is gated on B-008.

## Cross-bucket prereq edges

- **B-008 → B-015 (row -68 only).** Shared `emMainPanel::Cycle` body and shared `subscribed_init` field. B-015 row -68 cannot start until B-008's panel-level changes are merged. Rows -245..-320 and -50 are independent — no cross-bucket prereqs.

## Out-of-scope adjacency

- **`emFilePanel::cycle_inner` polling structure (preserved).** This bucket subscribes the panel's wake-up to the file-state signal, but does not restructure the polling body. The body re-reads `compute_vir_file_state` and compares against `last_vir_file_state` — semantically equivalent to a per-Cycle poll, observably correct now that Cycle is signal-driven. Restructuring to a pure event-handler shape is not required by P-006; if a future bucket wants it, that's separate work.
- **`emColorField::sync_from_children` walking the panel tree every Cycle.** Same rationale: walking is internal and correct; what mattered was that *Cycle* be signal-driven. Tree-walk-on-Cycle is below-surface adaptation; no annotation needed.
- **`update_slider_hiding` signature change cascades to call-sites in Mouse/Activate paths.** Implementer threads `SchedCtx` through; below-surface refactor.
- **Eight other `emMainPanel` lines that surrounding B-008 touches (rows 67, 69) are not in scope here.** B-015 only addresses row 68.

## Open questions for the implementer

1. **Engine-context plumbing for `emColorField`.** `Cycle(&mut self, ctx: &mut PanelCtx)` does not currently take an `EngineCtx`. The first-Cycle init requires `ectx.connect(...)` and `ectx.id()`. Either extend `Cycle`'s signature to add `&mut EngineCtx` (consistent with `emMainPanel::Cycle` shape) or use a `PanelCtx` accessor that exposes the panel's engine id and a `SchedCtx`. Pick the minimum-blast-radius option; if `PanelCtx` already carries this through `as_sched_ctx`, use it. The existing `ctx.as_sched_ctx()` in the Cycle body (`emColorField.rs:325`) suggests this path exists.
2. **Signal allocation for the slider timer in `emMainPanel`.** `TimerCentral::create_timer` takes a `SignalId` argument — the caller allocates. Either allocate at panel `new()` time (panel construction has access to `ConstructCtx` which exposes `create_signal`) or at first-Cycle init time. Construct-time matches C++ emTimer's "signal owned by timer member, allocated at member-init" semantics most closely.
3. **`SetFileModel` caller migration.** Inventory all `SetFileModel` callers (likely a small set: panel constructors, derived `emFilePanel` subclasses, tests). Update each to pass `&mut SchedCtx` + `EngineId`. If a caller has no engine context (e.g., a no-op test fixture), it may need a `SetFileModel_no_subscribe` test-only variant — but flagged: any production caller without engine context is itself a bug and should be fixed, not papered over.
4. **`auto_expand` re-arm semantics.** When the user toggles expansion off then on (`AutoShrink` then `AutoExpand`), the new children get fresh `SignalId`s. The first-Cycle-init pattern handles this because `subscribed_to_children = false` after `AutoShrink`. Confirm by reading the existing `auto_expand` / `auto_shrink` callers — there should be no path that calls `auto_expand` mid-Cycle in a way that would skip re-arming.

## Open items deferred to working-memory session

1. **No new D-### proposed.** This bucket reuses D-005 (reaction model: direct subscribe), D-006 (wiring shape: first-Cycle init for emColorField + emMainPanel; SetFileModel-time subscribe for emFilePanel under D-006's per-bucket override clause). The override is local — it follows directly from C++ structure (signal identity changes per model swap) and does not generalize.
2. **D-005 open question resolved.** Line 113 of `decisions.md` flags "for `emColorField::Cycle` polling four child ScalarFields, the subscribe call is N-fold; bucket sketcher confirms whether the C++ original subscribes individually or to an aggregated signal." **Resolved here: individual subscribes**, eight in total (RGBA + HSV + Name = 4 + 3 + 1), matching C++ `AutoExpand` (`emColorField.cpp:245,255,265,277,288,298,308,320`). Working-memory session may strike that open-question line.
3. **Cross-bucket prereq edge B-008 → B-015 (row -68 only).** Add to `work-order.md` DAG. B-015 cannot reach `merged` for row -68 before B-008's panel-level changes land; rows -245..-320 and -50 are independent.
4. **No row reclassifications.** All 10 rows verified `accessor present`. Bucket sketch is accurate; no `inventory-enriched.json` tag flips required.

## Success criteria

- `emColorField` subscribes to all eight expansion-child signals in a D-006 first-Cycle init block (re-armed on `AutoExpand`, cleared on `AutoShrink`).
- `emFilePanel::SetFileModel` calls `disconnect` on the old model's `FileStateSignal` and `connect` on the new model's, mirroring C++ `RemoveWakeUpSignal`/`AddWakeUpSignal` pair.
- `emMainPanel` allocates a slider-timer `SignalId`, creates an `emTimer` for it, connects the signal to the panel engine, and replaces the wall-clock polling block in `Cycle` with an `IsSignaled` branch.
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- New tests cover: 8 `emColorField` child signals (one assertion per row group), `emFilePanel` subscribe + model-swap disconnect, `emMainPanel` slider-timer arm + signal-driven hide.
- B-015 status in `work-order.md` flips `pending → designed` (working-memory session reconciliation), and per-row-group commits flip to `merged` as they land.
