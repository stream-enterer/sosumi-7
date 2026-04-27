# B-004-no-wire-misc — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-004-no-wire-misc.md`
**Pattern:** P-001-no-subscribe-no-accessor
**Scope:** misc (emcore=2, emmain/Bookmarks=1, emmain/VirtualCosmos=1) — 4 rows
**Mechanical-vs-judgement:** balanced — wiring is mechanical once the per-row accessor shape is decided; the four shapes are heterogeneous.

## Goal & scope

Wire the four P-001 small-scope leftovers. Each row is a different (accessor-side, consumer-side) pair, so the design is per-row rather than per-accessor-group. All four rows apply **D-006-subscribe-shape** (first-Cycle init + IsSignaled at top of Cycle) for the consumer-side wiring; the accessor side falls under **D-003 option A** (fill the gap, scoped per bucket).

The four rows:

| Row ID | C++ ref | Rust target | Accessor side | Consumer side |
|---|---|---|---|---|
| `emImageFile-117` | `emImageFile.cpp:117` | `emImageFileImageFilePanel.rs` ctor | `emFilePanel::GetVirFileStateSignal` (gap on emFilePanel base) | `emImageFilePanel` ctor + Cycle |
| `emFilePanel-accessor-vir-file-state` | n/a (parent of emImageFile-117 etc.) | `emFilePanel.rs` (base) | new `vir_file_state_signal: SignalId` field + `GetVirFileStateSignal` accessor + `Signal` calls | (covered by `emImageFile-117` consumer plus 4 derived-panel polling rows in B-015 / future P-006 buckets) |
| `emBookmarks-1479` | `emBookmarks.cpp:1479` | `emBookmarks.rs:528` (`emBookmarkButton`) | `click_signal` (gap on emBookmarkButton — exists on emButton but emBookmarkButton does NOT extend it) | `emBookmarkButton::Cycle` |
| `emVirtualCosmosModel-accessor-model-change` | n/a | `emVirtualCosmos.rs:213` | new `change_signal: SignalId` on `emVirtualCosmosModel`, fired in `Reload()` | none in scope (no current consumer subscribes to it; D-003 option A fills the accessor anyway) |

## Decisions cited

- **D-006-subscribe-shape** — canonical port shape for every consumer-side subscribe. First-Cycle init block calling `ectx.connect(sig, ectx.id())`, gated on `subscribed_init: bool`, then `IsSignaled` checks at top of Cycle, then reactions inline.
- **D-003-gap-blocked-fill-vs-stub** — option A (fill the gap, scoped per bucket). All four rows fill their accessor side in this bucket; per the bucket-level D-003 check, none of the missing accessors require porting an absent upstream model.

No other D-### decisions apply (D-001/D-002/D-004/D-005 do not touch this scope).

## Audit-data corrections

The bucket sketch (`B-004-no-wire-misc.md`) raises five open questions; this design re-validates each against actual Rust source.

1. **`emVirtualCosmosModel-accessor-model-change` — gap shape.** `emVirtualCosmosModel` is **fully ported** (`emVirtualCosmos.rs:213`, `Acquire` at `:225`, `Reload` at `:253`). The gap is a missing per-instance change signal that the C++ original would `Signal()` on `Reload()`. Per D-003 option A this is **fill-in-scope** (one new SignalId field + accessor + fire-in-Reload). Distinct from B-008 row `emVirtualCosmos-104`, which wired the *inbound* `FileUpdateSignalModel` subscribe on a `VirtualCosmosUpdateEngine` driving engine. B-004 here is the *outbound* per-model change signal (downstream consumers can observe model rebuilds). The two rows are non-overlapping: B-008 wires the input edge (broadcast → Reload), B-004 wires the output edge (Reload → change broadcast).
2. **`emFilePanel-accessor-vir-file-state` — gap shape.** `emFilePanel` is **ported** (`emFilePanel.rs:46`, full `VirtualFileState` enum at `:16`, `cycle_inner` at `:138`). The C++ class owns `emSignal VirFileStateSignal` (`emFilePanel.h:156`) with accessor at `:80`. Rust has *neither* the SignalId field nor the accessor; instead `cycle_inner` returns a `bool` indicating "state changed" and the engine framework consumes that return as a wake heuristic. Per D-003 option A this is **fill-in-scope** (one new SignalId field on `emFilePanel`, allocated at `new` time, fired from `cycle_inner` whenever `last_vir_file_state` changes). Replaces no current code path — additive.
3. **`emImageFile-117` — file location.** C++ `emImageFilePanel::emImageFilePanel` ctor lives at `emImageFile.cpp:110-119`; line 117 is the `AddWakeUpSignal(GetVirFileStateSignal())` call. Rust port is in the SPLIT file `emImageFileImageFilePanel.rs` (header at line 1: `// SPLIT: Split from emImageFile.h — panel type extracted`). The wire belongs in the SPLIT file's `emImageFilePanel`, not in the primary `emImageFile.rs` (which holds the model + LoaderEngine). Confirm by reading: the model file has no `emImageFilePanel` impl; only the SPLIT file does.
4. **`emBookmarks-1479` — emBookmarkButton structural status.** C++ `emBookmarkButton : public emButton` (header in `emBookmarks.h`); inherits `GetClickSignal` and the bookmark click ctor body at `emBookmarks.cpp:1479` includes `AddWakeUpSignal(GetClickSignal())`. Rust `emBookmarkButton` is a **standalone struct** at `emBookmarks.rs:528` — it does NOT extend `emButton` (and `emButton` itself is not currently used as a base via composition either). The bucket sketch asks "restructure to extend emButton, or bespoke accessor?" — **bespoke accessor** (option B) is the right answer, scoped within this bucket. Rationale below.
5. **emFilePanel-derived polling rows.** The bucket sketch flags that, once `vir_file_state_signal` exists, the five derived panels currently polling `vir_file_state` (image, dir, dirstat, filelink, stocksfile) become downstream consumers. **Not in B-004's scope.** Those polling consumers belong to the P-006 family. The image consumer specifically (`emImageFilePanel`) IS wired in B-004 because row `emImageFile-117` is in this bucket — that single wire doubles as the canonical first consumer. The remaining four polling-derived consumers are owned by B-015 (already designed for emFilePanel polling) or follow-on P-006 buckets.

These corrections do not move any rows out of B-004.

## Accessor groups

Following the B-001 / B-008 organising convention. Each group: which Rust state is touched, what fix the accessor needs, and which row(s) depend on it.

### G1 — `emFilePanel::GetVirFileStateSignal` (panel-side virtual-file-state broadcast)

**C++ source.** Owned: `emSignal VirFileStateSignal;` at `emFilePanel.h:156`; accessor at `:80`. Fired from every state-change path inside `emFilePanel`: `SetFileModel` (`:51`), `Cycle` on transitions, `set_custom_error`, `clear_custom_error`. Subscribers include `emImageFilePanel`, `emDirPanel`, `emDirStatPanel`, `emFileLinkPanel`, `emStocksFilePanel` (all subscribe in their constructor with `AddWakeUpSignal(GetVirFileStateSignal())`).

**Rust state today.** `VirtualFileState` enum + `last_vir_file_state` cache exist (`emFilePanel.rs:16,49`). `cycle_inner` (`:138`) recomputes the state and returns a `bool`. **No SignalId field, no accessor.** Mutators (`SetFileModel:74`, `set_custom_error:85`, `clear_custom_error:90`) update `last_vir_file_state` silently.

**Fix.**

```rust
pub struct emFilePanel {
    // ... existing fields ...
    /// Port of C++ emFilePanel::VirFileStateSignal.
    /// Fired whenever last_vir_file_state mutates.
    vir_file_state_signal: SignalId,
}

impl emFilePanel {
    pub fn new<C: ConstructCtx>(cc: &mut C) -> Self {
        Self {
            // ... existing fields ...
            vir_file_state_signal: cc.create_signal(),
        }
    }

    /// Port of C++ emFilePanel::GetVirFileStateSignal.
    pub fn GetVirFileStateSignal(&self) -> SignalId {
        self.vir_file_state_signal
    }

    fn signal_vir_file_state(&self, ectx: &mut EngineCtx<'_>) {
        ectx.fire(self.vir_file_state_signal);
    }
}
```

**Mutator audit (must call `Signal` after the state mutation, mirroring C++ `Signal(VirFileStateSignal)`):**

- `SetFileModel` (`:74`) — currently `self.last_vir_file_state = new_state;`. C++ `:51` fires unconditionally after the model swap. Add a fire here. **Note:** `SetFileModel` does not currently take an `ectx`; either thread `ectx` through (preferred — preserves the C++ "fire on swap" timing) or queue the fire and drain it from the next `Cycle()` (fallback, matches the `add_pre_show_wake_up_signal` precedent cited in D-006).
- `set_custom_error` (`:85`), `clear_custom_error` (`:90`) — same threading question. C++ analogues fire from `SetCustomError`. Same resolution: prefer threading `ectx`.
- `cycle_inner` (`:138`) — already returns `true` when `new_state != last_vir_file_state`. Add a fire on the `true` branch *before* returning. Cycle has access to `ectx` via the outer `Cycle` impl (`:402`); thread it down by changing `cycle_inner`'s signature to `cycle_inner(&mut self, ectx: &mut EngineCtx<'_>) -> bool` and adjust the single caller at `:407`.

**Constructor signature change.** `emFilePanel::new()` currently takes no args. Adding the SignalId allocation requires `new(cc: &mut C: ConstructCtx)`. The two existing callers in scope are `emImageFilePanel::new` (`emImageFileImageFilePanel.rs:24`) and `emImageFilePanel::with_model` (`:31`); both must be updated to take a `cc` and forward it. Test code (`emFilePanel.rs:493 make_panel_with_model`) needs the same. Below-surface adaptation; no annotation. The threading is mechanical.

**Test impact.** The existing `refresh_vir_file_state` (`:106`) helper exists for tests that mutate the model directly and want the panel cache refreshed. Tests that previously asserted on the bool return of `cycle_inner` continue to work; new tests asserting the signal fires use `Harness::fire`/wait helpers per the B-005/B-008 test pattern.

**Rows depending on G1:**
- `emImageFile-117` (consumer subscribe — wired in this bucket).
- Four derived-panel polling rows (image already covered; dir/dirstat/filelink/stocksfile remain in B-015 / future P-006 buckets — not in B-004).

### G2 — `emBookmarkButton::GetClickSignal` (per-button click broadcast)

**C++ source.** Inherited via `emButton::GetClickSignal`; the bookmark-button ctor calls `AddWakeUpSignal(GetClickSignal())` at `emBookmarks.cpp:1479` and reacts in its Cycle by issuing the bookmark navigation (`emView::Visit(...)` to the bookmarked location).

**Rust state today.** `emBookmarkButton` (`emBookmarks.rs:528`) is a standalone struct with three fields (bookmark, icon_load_attempted, icon). No SignalId, no click-detection logic. Its `Cycle` is a stub returning `false` with a comment "Navigation is not wired yet" (`:638`). Input → click conversion happens in the standard `PanelBehavior::input` path (not currently overridden on this struct, so click input is silently dropped).

**Fix — bespoke accessor + minimal click detection.** Per audit correction #4, restructuring `emBookmarkButton` to compose `emButton` is out of scope (it would introduce a 200+-line composition graft, restyle the Paint code, and cross into B-013-rc-shim-emcore territory where button click handling is already under design). The narrower fix:

```rust
pub struct emBookmarkButton {
    bookmark: emBookmarkRec,
    icon_load_attempted: bool,
    icon: Option<emImage>,
    // New per-D-006 wiring fields.
    /// Port of C++ inherited emButton::ClickSignal.
    click_signal: SignalId,
    subscribed_init: bool,
}

impl emBookmarkButton {
    pub fn new<C: ConstructCtx>(cc: &mut C, bookmark: emBookmarkRec) -> Self {
        Self {
            bookmark,
            icon_load_attempted: false,
            icon: None,
            click_signal: cc.create_signal(),
            subscribed_init: false,
        }
    }

    /// Port of inherited C++ emButton::GetClickSignal.
    pub fn GetClickSignal(&self) -> SignalId { self.click_signal }
}
```

**Click detection.** Add an `input` impl on `PanelBehavior` for `emBookmarkButton` mirroring `emButton::Input` (the relevant subset: detect activation key/mouse-down, fire `click_signal`). Use `emButton`'s existing input pattern (`emButton.rs:417,449,474` show `sched.fire(self.click_signal)` after the LP_LEFT click and after Enter/Space). Copy the trigger logic, not the full focus-ring rendering. Below-surface adaptation; the trigger semantics (single click = navigate) are observable, the rendering stays as-is in `Paint`.

**Reaction — `Cycle`.** Per D-006: first-Cycle init connects to `click_signal`; `IsSignaled` branch in Cycle calls navigation. Navigation is the bookmark-`Visit` op against the active view. C++ uses `GetView().Visit(LocationIdentity, LocationRelX, LocationRelY, LocationRelA)` from `bookmark.entry`. Rust threading: `pctx.view().Visit(...)` if the helper exists, else add a small `pctx.visit(loc)` shim. The bookmark fields (`LocationIdentity`, `LocationRelX/Y/A`) are already on `emBookmarkRec` (used for serialization).

**Rows depending on G2:**
- `emBookmarks-1479` (consumer subscribe + reaction — both wired in this bucket).

### G3 — `emVirtualCosmosModel::GetChangeSignal` (model→change broadcast)

**C++ source.** C++ `emVirtualCosmosModel` calls `Signal(ChangeSignal)` from `Reload()` (verify against `emVirtualCosmos.cpp` Reload body — the `:121 Reload()` call inside `Cycle` followed by an emModel-default `Signal()` propagation). Subscribers in C++: `emVirtualCosmosFpPlugin` (the file-panel plugin that renders cosmos items).

**Rust state today.** `emVirtualCosmosModel` (`emVirtualCosmos.rs:213`) has *no* SignalId field, no accessor, and `Reload()` mutates `items` + `item_recs` silently. Acquire at `:225` returns an `Rc<RefCell<Self>>`; the `from_items` test helper at `:239` bypasses Acquire entirely.

**Fix.** Add a `change_signal: SignalId` field, allocate in `Acquire`'s closure (which currently has no signal-allocator handle — see open question below), and fire in `Reload()` after the item rebuild completes.

```rust
pub struct emVirtualCosmosModel {
    items_dir: String,
    item_files_dir: String,
    items: Vec<LoadedItem>,
    item_recs: Vec<usize>,
    /// Port of C++ emVirtualCosmosModel inherited ChangeSignal.
    change_signal: SignalId,
}

impl emVirtualCosmosModel {
    pub fn Acquire(ctx: &Rc<emContext>) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>("", || {
            let change_signal = ctx.create_signal(); // see open question
            let mut model = Self {
                items_dir: String::new(),
                item_files_dir: String::new(),
                items: Vec::new(),
                item_recs: Vec::new(),
                change_signal,
            };
            model.Reload();
            model
        })
    }

    pub fn GetChangeSignal(&self) -> SignalId { self.change_signal }

    pub fn Reload(&mut self) {
        // ... existing body ...
        // After self.items = new_items; self.sort_item_recs();
        // Fire the change broadcast. Same site as C++ Reload() final Signal().
        // Threading: see open question.
    }
}
```

**Cross-bucket interaction with B-008 row `-104`.** B-008 wires `VirtualCosmosUpdateEngine` to call `model.Reload()` on `App::file_update_signal` arrival. With G3 in place, `Reload()` *also* fires `change_signal`, so any consumer that subscribes to `change_signal` will wake on the broadcast → reload → change cascade. This is the C++ contract. **Sequencing:** B-004 G3 can land before, after, or concurrently with B-008 row 104; they are independent edges of the same model. No cross-bucket prereq.

**Rows depending on G3:**
- The audit row itself (the accessor). No B-004 consumer subscribes to it (per the bucket sketch). Per D-003 option A, fill the accessor anyway.
- Future bucket: when an `emVirtualCosmosFpPlugin` Rust port lands, it inherits this accessor as its subscribe target. **Future-bucket reuse, not a B-004 prereq edge.**

## Wiring-shape application (D-006)

### `emImageFilePanel::Cycle` (row `emImageFile-117`)

```rust
pub struct emImageFilePanel {
    file_panel: emFilePanel,
    current_image: Option<emImage>,
    // New per-D-006 wiring field.
    subscribed_init: bool,
}

impl PanelBehavior for emImageFilePanel {
    fn Cycle(
        &mut self,
        ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        ctx: &mut PanelCtx,
    ) -> bool {
        if !self.subscribed_init {
            // Row emImageFile-117. Mirrors C++ emImageFile.cpp:117.
            ectx.connect(self.file_panel.GetVirFileStateSignal(), ectx.id());
            self.subscribed_init = true;
        }

        // React to vir-file-state change: refresh cached image from model.
        if ectx.IsSignaled(self.file_panel.GetVirFileStateSignal()) {
            // Mirrors C++ emImageFilePanel::Cycle which on VirFileStateSignal
            // reloads CurrentImage from the model when state is good.
            self.refresh_current_image_from_model();
            ctx.invalidate_painting();
        }

        // Delegate the inner state-update bookkeeping.
        self.file_panel.cycle_inner(ectx)
    }
}
```

The `refresh_current_image_from_model` helper either copies `model.GetImage()` into `self.current_image` (when state is `Loaded`/`Unsaved`) or clears it (other states). Mirrors C++ `emImageFilePanel::Cycle` body; implementer reads the C++ ranges to confirm.

### `emBookmarkButton::Cycle` (row `emBookmarks-1479`)

```rust
fn Cycle(
    &mut self,
    ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
    ctx: &mut PanelCtx,
) -> bool {
    if !self.subscribed_init {
        // Row emBookmarks-1479. Mirrors C++ emBookmarks.cpp:1479.
        ectx.connect(self.click_signal, ectx.id());
        self.subscribed_init = true;
    }

    if ectx.IsSignaled(self.click_signal) {
        // Mirrors C++ emBookmarkButton's bookmark-navigation reaction.
        ctx.view().Visit(
            &self.bookmark.entry.LocationIdentity,
            self.bookmark.entry.LocationRelX,
            self.bookmark.entry.LocationRelY,
            self.bookmark.entry.LocationRelA,
        );
    }

    false
}
```

### `emFilePanel::cycle_inner` mutator wiring (row `emFilePanel-accessor-vir-file-state`)

```rust
pub(crate) fn cycle_inner(&mut self, ectx: &mut EngineCtx<'_>) -> bool {
    let new_state = self.compute_vir_file_state();
    if new_state != self.last_vir_file_state {
        self.last_vir_file_state = new_state;
        ectx.fire(self.vir_file_state_signal); // mirrors C++ Signal(VirFileStateSignal)
        true
    } else {
        false
    }
}
```

`SetFileModel`, `set_custom_error`, `clear_custom_error` call `ectx.fire(self.vir_file_state_signal)` after the state update (signature change required — see open questions).

### `emVirtualCosmosModel::Reload` mutator wiring (row `emVirtualCosmosModel-accessor-model-change`)

```rust
pub fn Reload(&mut self /* + ectx threading */) {
    // ... existing body ...
    self.items = new_items;
    self.sort_item_recs();
    // ectx.fire(self.change_signal); // mirrors C++ Signal(ChangeSignal)
}
```

The fire belongs at the end of `Reload()` matching the C++ `Reload()` exit. The threading question (how `Reload` gets an `ectx`) is the same shape as the `emFilePanel` mutator-threading question — see open questions.

## Verification strategy

C++ → Rust observable contract: each signal fires and the documented Cycle reaction runs.

**Pre-fix observable behavior:**
- `emImageFile-117`: When the underlying image-file model transitions from `Loading` to `Loaded`, `emImageFilePanel`'s cached `current_image` does not refresh (it relies on `cycle_inner`'s bool return reaching the framework, but no subscribe wires the panel into the wake-up path of vir-file-state changes).
- `emBookmarks-1479`: Clicking a bookmark button has no effect (Cycle is a stub).
- `emFilePanel-accessor-vir-file-state`: No external observer can subscribe to vir-file-state changes; derived panels rely on their *own* Cycle being scheduled by the framework.
- `emVirtualCosmosModel-accessor-model-change`: After B-008 lands, `Reload()` runs but no consumer can observe completion.

**Post-fix observable behavior:** all four wires fire and downstream Cycle reactions run.

**New test file:** `crates/emcore/tests/no_wire_b004_emcore.rs` (rows `emImageFile-117`, `emFilePanel-accessor-vir-file-state`) and `crates/emmain/tests/no_wire_b004_emmain.rs` (rows `emBookmarks-1479`, `emVirtualCosmosModel-accessor-model-change`). Both files RUST_ONLY: dependency-forced — no C++ test analogue, mirrors B-005/B-008 test rationale.

Test sketches (one per row):

```rust
// Row emFilePanel-accessor-vir-file-state
let mut h = Harness::new();
let panel = make_panel_with_model(&mut h.cc());
let sig = panel.GetVirFileStateSignal();
let watcher = h.watch(sig);
panel.SetFileModel(/* swap to a new model */);
h.run_cycle();
assert!(watcher.fired());

// Row emImageFile-117
let mut h = Harness::new();
let panel = h.create_image_file_panel(/* with model */);
let model = panel.borrow().model();
model.borrow_mut().complete_load(/* image data */);
h.run_cycle();
assert!(panel.borrow().current_image().is_some());

// Row emBookmarks-1479
let mut h = Harness::new();
let btn = emBookmarkButton::new(&mut h.cc(), bookmark_with_location("foo:bar"));
let sig = btn.GetClickSignal();
h.fire(sig);
h.run_cycle();
assert_eq!(h.last_visit_target(), Some("foo:bar".into()));

// Row emVirtualCosmosModel-accessor-model-change
let mut h = Harness::new();
let model = emVirtualCosmosModel::Acquire(h.ctx());
let sig = model.borrow().GetChangeSignal();
let watcher = h.watch(sig);
model.borrow_mut().Reload(/* + ectx */);
h.run_cycle();
assert!(watcher.fired());
```

**Four-question audit-trail evidence per row:** (1) signal connected? — D-006 init block / accessor fire-site. (2) Cycle observes? — `IsSignaled` branch (consumer rows). (3) reaction fires documented mutator? — assertions above. (4) C++ branch order preserved? — code review against the cited C++ line ranges.

## Implementation sequencing

The four rows are largely independent. Recommended order:

1. **Step 1: G1 accessor + mutator threading on `emFilePanel`.** Add `vir_file_state_signal` field; thread `cc: &mut C: ConstructCtx` into `new`; thread `ectx: &mut EngineCtx<'_>` into `cycle_inner`, `SetFileModel`, `set_custom_error`, `clear_custom_error`. Fire from each mutator. Update the two `emImageFilePanel::new`/`with_model` callers and the test helper. Land as one commit.
2. **Step 2: G1 consumer + row `emImageFile-117`.** Add `subscribed_init` to `emImageFilePanel`; first-Cycle connect to `file_panel.GetVirFileStateSignal()`; IsSignaled branch refreshes `current_image`. Land with row-117 test. Depends on step 1.
3. **Step 3: G2 row `emBookmarks-1479`.** Add `click_signal` + `subscribed_init` fields; `cc`-threaded `new`; minimal input → `click_signal` fire; D-006 first-Cycle connect + IsSignaled → `view().Visit(...)`. Land with row-1479 test. Independent of steps 1-2.
4. **Step 4: G3 row `emVirtualCosmosModel-accessor-model-change`.** Add `change_signal` field; allocate in `Acquire`; fire from `Reload`. Resolve `Acquire` ectx-handle threading per open question 2. Land with the row test. Independent of steps 1-3 and of B-008.

Steps 3 and 4 can run in parallel with steps 1-2.

## Cross-bucket prereq edges

**None.** No B-004 row blocks on another bucket's deliverable, and no other bucket's row blocks on B-004 (B-008 row 104 is the input edge to `Reload`; B-004 G3 is the output edge of `Reload`; the two are independent).

**Forward edges (informational, not prereqs):**
- B-015 (and any future P-006 polling-to-subscribe bucket touching emFilePanel-derived panels) **inherits** G1's `GetVirFileStateSignal`. Once B-004 step 1 lands, the dir / dirstat / filelink / stocksfile derived panels gain a target for their D-006 first-Cycle subscribes. This is reuse, not a hard edge: those buckets can be designed against the planned accessor and integrated in any order.
- B-008 row 104 **may** combine cleanly with B-004 G3: once both land, `App::file_update_signal → VirtualCosmosUpdateEngine → model.Reload() → model.change_signal` becomes the full C++ reload-broadcast cascade. Neither blocks the other.

## Open questions for the implementer

1. **`emFilePanel` mutator ectx threading.** `SetFileModel`, `set_custom_error`, `clear_custom_error` currently take `&mut self` only. Two options:
   - **(a)** Thread `ectx: &mut EngineCtx<'_>` through every caller (preserves C++ "fire on swap" timing — preferred).
   - **(b)** Set a `pending_vir_state_fire: bool` flag and drain in next `cycle_inner` (matches the `add_pre_show_wake_up_signal` precedent from D-006 option B; one-Cycle delay observable in tests).
   Pick (a) unless a caller cannot supply `ectx`; flag if (b) becomes necessary.
2. **`emVirtualCosmosModel::Acquire` ectx-handle threading.** `Acquire` takes only `&Rc<emContext>`. To allocate `change_signal` and to fire it from `Reload`, the model needs either a `SignalId` allocator on the context or `&mut SchedCtx`. Two options:
   - **(a)** Use the existing `ctx.create_signal()` if `emContext` already exposes signal allocation (verify in `emContext` API).
   - **(b)** If not, follow B-008's pattern (lazy registration on first model use) — but for B-004 the simpler shape is to allocate the signal at `Acquire` time using whatever signal-allocator handle `emContext` exposes today; if none, escalate to working-memory session because this becomes a structural limitation.
3. **`emBookmarkButton` input handling.** Decide the smallest viable click-detection: copy the relevant subset of `emButton::input` (`emButton.rs:417,449,474`) into `emBookmarkButton`, or extract a small `fn detect_click_input(...) -> bool` helper into a shared module. Below-surface; implementer picks for least duplication.
4. **`view().Visit(...)` accessor on `PanelCtx`.** If `pctx.view()` does not directly expose a `Visit` shim, add one. Confirms the navigation path matches existing emcore Visit-call sites (search for `Visit(` to find the canonical pattern).

## Open items deferred to working-memory session

1. **No D-### proposed.** All four rows fit under D-003 (option A — fill in scope) and D-006 (subscribe shape). No new cross-cutting decision surfaces.
2. **No row anomalies requiring `inventory-enriched.json` patching.** The four rows are correctly classified as `drifted` P-001. The bucket sketch's open questions are answered above; sketch text can be updated in place.
3. **Forward-edge note for B-015 / future P-006 buckets:** once B-004 step 1 lands, derived emFilePanel-panel polling consumers gain `GetVirFileStateSignal` as their subscribe target. Working-memory session can flag this in `work-order.md` as a soft sequencing hint (B-004 step 1 before B-015's emFilePanel rows is the cheaper order, though not required for correctness — B-015 can stub against the planned accessor).
4. **Forward-edge note for emVirtualCosmosFpPlugin port (future bucket).** When the plugin lands, inherit `GetChangeSignal` as its subscribe target. Out of scope for B-004.

## Success criteria

- `emFilePanel::GetVirFileStateSignal` exists; the signal fires from every `last_vir_file_state` mutator.
- `emImageFilePanel` subscribes to the signal in a D-006 first-Cycle init block and refreshes `current_image` on signal arrival.
- `emBookmarkButton` exposes `GetClickSignal`, fires it on activation input, subscribes to it in Cycle, and reacts by navigating the view to the bookmark's location.
- `emVirtualCosmosModel::GetChangeSignal` exists; the signal fires from `Reload()`.
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- New `crates/emcore/tests/no_wire_b004_emcore.rs` and `crates/emmain/tests/no_wire_b004_emmain.rs` cover all four rows.
- B-004 status in `work-order.md` flips `pending → designed` (working-memory session reconciliation).
