# B-004-no-wire-misc — Design

**Date:** 2026-04-27
**Status:** Partially merged (emcore-slice 9b8ee012; emmain G3 already merged via B-014 c2871547) — only `emBookmarks-1479` remains for dispatch
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-004-no-wire-misc.md`
**Pattern:** P-001-no-subscribe-no-accessor
**Scope (post-reconciliation 2026-05-01):** misc — 4 audit rows total: 2 emcore rows merged at 9b8ee012; emmain G3 (`emVirtualCosmosModel-accessor-model-change`) merged at c2871547 by B-014; **only `emBookmarks-1479` remains designed**. See Amendment Log 2026-05-01.
**Mechanical-vs-judgement:** balanced — wiring is mechanical once the per-row accessor shape is decided; the four shapes are heterogeneous.

## Goal & scope

Wire the four P-001 small-scope leftovers. Each row is a different (accessor-side, consumer-side) pair, so the design is per-row rather than per-accessor-group. All four rows apply **D-006-subscribe-shape** (first-Cycle init + IsSignaled at top of Cycle) for the consumer-side wiring; the accessor side falls under **D-003 option A** (fill the gap, scoped per bucket).

The four rows:

| Row ID | C++ ref | Rust target | Accessor side | Consumer side |
|---|---|---|---|---|
| `emImageFile-117` | `emImageFile.cpp:117` | `emImageFileImageFilePanel.rs` ctor | `emFilePanel::GetVirFileStateSignal` (gap on emFilePanel base) | `emImageFilePanel` ctor + Cycle |
| `emFilePanel-accessor-vir-file-state` | n/a (parent of emImageFile-117 etc.) | `emFilePanel.rs` (base) | new `vir_file_state_signal: SignalId` field + `GetVirFileStateSignal` accessor + `Signal` calls | (covered by `emImageFile-117` consumer plus 4 derived-panel polling rows in B-015 / future P-006 buckets) |
| `emBookmarks-1479` | `emBookmarks.cpp:1479` | `emBookmarks.rs:528` (`emBookmarkButton`) | `click_signal` (gap on emBookmarkButton — exists on emButton but emBookmarkButton does NOT extend it) | `emBookmarkButton::Cycle` |
| ~~`emVirtualCosmosModel-accessor-model-change`~~ | ~~n/a~~ | ~~`emVirtualCosmos.rs:213`~~ | **MERGED via B-014 at c2871547.** Live at `crates/emmain/src/emVirtualCosmos.rs:215-287`: `change_signal: Cell<SignalId>` (line 225), combined-form `GetChangeSignal(&self, &mut impl SignalCtx)` (line 262, D-008 A1), `CALLSITE-NOTE` on `Reload` (lines 277-287) documenting why bootstrap fire is benign-no-op. **Struck from B-004 scope.** | — |

## Decisions cited

- **D-006-subscribe-shape** — canonical port shape for every consumer-side subscribe. First-Cycle init block calling `ectx.connect(sig, ectx.id())`, gated on `subscribed_init: bool`, then `IsSignaled` checks at top of Cycle, then reactions inline.
- **D-003-gap-blocked-fill-vs-stub** — option A (fill the gap, scoped per bucket).
- **D-007-mutator-fire-shape** — mutator-side fires take `&mut impl SignalCtx`; out-of-Cycle mutators use a deferred-fire (`pending_*: bool` drained next Cycle) when no `ectx` is reachable. Cited by emcore-slice (`pending_vir_state_fire`) and the `emBookmarks-1479` reaction below.
- **D-008-signal-allocation-shape** (A1 combined-form) — accessor `Get*Signal(&self, &mut impl SignalCtx)` lazily allocates from a `Cell<SignalId>` initialized to `SignalId::null()`. Used by B-014's merged G3 implementation and required for `emBookmarkButton::GetClickSignal` per Adversarial Review Finding 5.
- **D-009-no-polling-intermediaries** — no `Cell`/`RefCell` polling drains between distinct Cycles; all wires fire synchronously where C++ does, or use the documented deferred-fire shape under D-007.

D-001/D-002/D-004/D-005 do not touch this scope.

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

**Fix — D-008 A1 combined-form accessor + deferred click detection.** Per Adversarial Review Findings 5–6 and the merged emcore-slice precedent (`pending_vir_state_fire` deferred allocation):

The only caller of `emBookmarkButton::new` is `emBookmarksPanel::LayoutChildren` at `emBookmarks.rs:736`, which has `&mut PanelCtx` — no `ConstructCtx` is reachable at construction time. This is the same language-forced constraint that drove the emcore-slice's deferred-allocation pattern. The constructor signature stays `new(bookmark: emBookmarkRec)`; the click signal is allocated lazily on first subscribe via D-008 A1 combined form.

```rust
pub struct emBookmarkButton {
    bookmark: emBookmarkRec,
    icon_load_attempted: bool,
    icon: Option<emImage>,
    // New per-D-008 A1 + D-006 wiring fields.
    /// Port of C++ inherited emButton::ClickSignal.
    /// Lazily allocated per D-008 A1 combined form (precedent: B-014's
    /// emVirtualCosmosModel::change_signal at emVirtualCosmos.rs:225).
    click_signal: Cell<SignalId>,
    /// Set by input handler when click detected; drained next Cycle (D-007 deferred-fire).
    pending_click_fire: Cell<bool>,
    subscribed_init: bool,
}

impl emBookmarkButton {
    pub fn new(bookmark: emBookmarkRec) -> Self {
        Self {
            bookmark,
            icon_load_attempted: false,
            icon: None,
            click_signal: Cell::new(SignalId::null()),
            pending_click_fire: Cell::new(false),
            subscribed_init: false,
        }
    }

    /// Port of inherited C++ emButton::GetClickSignal.
    /// D-008 A1: lazy allocation on first call.
    pub fn GetClickSignal(&self, ectx: &mut impl SignalCtx) -> SignalId {
        let sig = self.click_signal.get();
        if sig.is_null() {
            let new_sig = ectx.create_signal();
            self.click_signal.set(new_sig);
            new_sig
        } else {
            sig
        }
    }
}
```

**Click detection — recommend defer to B-013.** Per Adversarial Review Finding 6: porting `emButton::Input` (C++ `emButton.rs:331-475` ≈ multi-hundred lines covering LP_LEFT, focus/active state, mouse capture, hover repaint, Enter/Space key activation) is **not** "minimal subset" work. B-013-rc-shim-emcore already owns button click-handling semantics. **Implementer flag:** the `emBookmarks-1479` row should not attempt to copy `emButton::Input` here. Two options for the implementer:
  - **(a)** Defer the click-detection portion to B-013: land the accessor + Cycle reaction skeleton in this bucket, leave `pending_click_fire` set only by a stub that B-013 will replace. Annotate the stub `UPSTREAM-GAP:` (no — it's not an upstream gap, it's a Rust-side ordering choice). Use a tracking row in the work-order instead.
  - **(b)** Accept partial-coverage divergence: copy only LP_LEFT mouse-down → `pending_click_fire = true` (the simplest possible trigger) and annotate `DIVERGED: language-forced` at the input impl, citing that the Enter/Space/focus-ring/capture parity is deferred to B-013.

Pick (b) **only** if B-013 dispatch is materially delayed; otherwise (a) is cleaner.

**ContentView (Adversarial Finding 4).** C++ `emBookmarkButton::ContentView` (`emBookmarks.cpp:1470,1523-1535`) lets a button visit a view *other* than its owning view (multi-view bookmark configurations). The Rust port has **no `ContentView` field** and Rust `emView` has no multi-view content/control split. Annotate the Cycle reaction `DIVERGED: upstream-gap-forced — Rust has not ported the multi-view content/control split; emBookmarkButton visits its owning view only.` Track a follow-up row for when the multi-view split lands. Do **not** add an `Rc`/`Weak` field speculatively.

**Reaction — `Cycle`.** Per D-006: first-Cycle init lazily allocates+connects `click_signal`; `IsSignaled` branch calls navigation. Per Adversarial Review Finding 3, `PanelCtx` has no `view()` accessor and `emView::Visit` (`emView.rs:1094-1107`) takes 8 args (`tree, panel, rel_x, rel_y, rel_a, adherent, ctx: &mut SchedCtx`) — a 4-arg `pctx.view().Visit(...)` call is fictional. **Resolution:** enqueue a deferred navigation onto the framework's `pending_actions` rail (the existing closure-rail pattern used by other framework-mediated actions); the framework drains the queue and invokes the canonical `emView::VisitByIdentity` path. Sketch:

```rust
if ectx.IsSignaled(self.click_signal.get()) {
    // DIVERGED: upstream-gap-forced — single-view navigation only;
    // C++ emBookmarkButton::ContentView multi-view target not ported.
    let id = self.bookmark.LocationIdentity.clone();
    let rx = self.bookmark.LocationRelX;
    let ry = self.bookmark.LocationRelY;
    let ra = self.bookmark.LocationRelA;
    ctx.enqueue_visit_by_identity(id, rx, ry, ra);
}
```

The bookmark fields (`LocationIdentity`, `LocationRelX/Y/A`) are **direct on `emBookmarkRec`** (`emBookmarks.rs:107-110`), not nested under `.entry`. (`.entry: emBookmarkEntryBase` carries only `Name`/`Icon`/`BgColor`/`FgColor`.) The earlier draft using `self.bookmark.entry.LocationIdentity` was wrong and would not compile — fixed here per Adversarial Review Finding 2.

**M-001 reminder.** Before writing the Cycle body, read C++ `emBookmarks.cpp:1511-1538` directly. The C++ Cycle calls `emButton::Cycle()` and runs an `UpToDate` Update path; the single-branch sketch above must be reconciled with that branch structure (e.g., does the icon-load `UpToDate` path also need a Cycle branch here?).

**Rows depending on G2:**
- `emBookmarks-1479` (consumer subscribe + reaction — both wired in this bucket).

### G3 — `emVirtualCosmosModel::GetChangeSignal` (model→change broadcast) — **MERGED via B-014 at c2871547**

> **Reconciliation 2026-05-01.** This entire group is **already implemented** by the B-014 merge. The live code at `crates/emmain/src/emVirtualCosmos.rs:215-287` carries `change_signal: Cell<SignalId>` (line 225), the combined-form lazy accessor `GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` (line 262, D-008 A1), and a `CALLSITE-NOTE` on `Reload` (lines 277-287) documenting that the only existing callsite is the Acquire-bootstrap closure where `change_signal == SignalId::null()` — the missing fire is benign-by-construction under the D-007/D-008 composition. Future post-Acquire callers (e.g., a future port of `emVirtualCosmosModel::Cycle` reacting to `FileUpdateSignalModel`) must thread `&mut impl SignalCtx` and fire after a successful reload, mirroring B-003's `emAutoplayViewModel::signal_change` mutator-fire pattern.
>
> The "Fix" sketch below is **superseded** by the merged code and is retained only for design-history continuity. Do not use it as an implementation brief.

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

Amended 2026-05-01 per Adversarial Review Findings 2/3/4/5. Lazy-allocate `click_signal` via D-008 A1 in the first-Cycle init block (no `ConstructCtx` available at construction); read bookmark fields direct off `emBookmarkRec` (not `.entry`); enqueue navigation onto the framework's deferred-action rail (no `PanelCtx::view().Visit(...)` exists). Single-view navigation; multi-view ContentView is `DIVERGED: upstream-gap-forced`.

```rust
fn Cycle(
    &mut self,
    ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
    ctx: &mut PanelCtx,
) -> bool {
    // Drain any pending click fire (D-007 deferred-fire pattern; input handler
    // sets pending_click_fire because input runs without a SignalCtx).
    if self.pending_click_fire.replace(false) {
        let sig = self.GetClickSignal(ectx); // D-008 A1 lazy alloc
        ectx.fire(sig);
    }

    if !self.subscribed_init {
        // Row emBookmarks-1479. Mirrors C++ emBookmarks.cpp:1479
        // AddWakeUpSignal(GetClickSignal()).
        let sig = self.GetClickSignal(ectx);
        ectx.connect(sig, ectx.id());
        self.subscribed_init = true;
    }

    let sig = self.click_signal.get();
    if !sig.is_null() && ectx.IsSignaled(sig) {
        // DIVERGED: upstream-gap-forced — Rust emView has no multi-view
        // content/control split, so emBookmarkButton::ContentView (C++
        // emBookmarks.cpp:1470,1523-1535) is not ported. Visit the owning
        // view only; multi-view bookmarks observably diverge.
        let id = self.bookmark.LocationIdentity.clone();
        let rx = self.bookmark.LocationRelX;
        let ry = self.bookmark.LocationRelY;
        let ra = self.bookmark.LocationRelA;
        ctx.enqueue_visit_by_identity(id, rx, ry, ra);
    }

    // M-001: confirm against C++ emBookmarks.cpp:1511-1538 — the C++ Cycle
    // also calls emButton::Cycle() and runs an UpToDate Update path. If the
    // icon-load branch needs an analogous Cycle hook, add it here.
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

### ~~`emVirtualCosmosModel::Reload` mutator wiring~~ — **MERGED via B-014 at c2871547**

Live code at `crates/emmain/src/emVirtualCosmos.rs:215-287`. The `CALLSITE-NOTE` on `Reload` (lines 277-287) documents the bootstrap-callsite analysis: the only existing caller is the Acquire closure, where `change_signal == SignalId::null()` so a missing fire is benign-by-construction under D-007/D-008 composition. Future post-Acquire callers must thread `&mut impl SignalCtx` and fire after a successful reload. This row is **struck from B-004 dispatch scope.**

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

// Row emBookmarks-1479 (amended 2026-05-01)
let mut h = Harness::new();
let btn = emBookmarkButton::new(bookmark_with_location("foo:bar"));
// D-008 A1: GetClickSignal takes &mut impl SignalCtx and lazily allocates.
let sig = btn.GetClickSignal(&mut h.signal_ctx());
let watcher = h.watch(sig);
// Simulate input → pending_click_fire flag set by input handler stub.
btn.set_pending_click_fire_for_test();
h.run_cycle();
assert!(watcher.fired());
// Navigation enqueued onto the framework's pending_actions rail.
assert_eq!(h.last_visit_target(), Some("foo:bar".into()));

// Row emVirtualCosmosModel-accessor-model-change — STRUCK from B-004 scope
// (already implemented by B-014 at c2871547). See Amendment Log 2026-05-01.
```

**Four-question audit-trail evidence per row:** (1) signal connected? — D-006 init block / accessor fire-site. (2) Cycle observes? — `IsSignaled` branch (consumer rows). (3) reaction fires documented mutator? — assertions above. (4) C++ branch order preserved? — code review against the cited C++ line ranges.

## Implementation sequencing

**Status as of 2026-05-01:** Steps 1, 2, and 4 are merged. Only Step 3 (`emBookmarks-1479`) remains for dispatch.

1. ~~**Step 1: G1 accessor + mutator threading on `emFilePanel`.**~~ **MERGED at 9b8ee012.** See work-order 2026-04-29 entry for the deferred-allocation amendment (`new()` signature unchanged; `ensure_vir_file_state_signal` lazy-allocates in Cycle).
2. ~~**Step 2: G1 consumer + row `emImageFile-117`.**~~ **MERGED at 9b8ee012.** Augmented (not replaced) the existing B-007 ChangeSignal subscription.
3. **Step 3: G2 row `emBookmarks-1479`.** Add `click_signal: Cell<SignalId>` + `pending_click_fire: Cell<bool>` + `subscribed_init: bool` fields; `new(bookmark)` signature unchanged (D-008 A1 lazy alloc); D-006 first-Cycle connect + IsSignaled branch enqueues navigation onto the framework's pending-actions rail (no `view().Visit(...)` shim exists). Decide click-detection scope (defer to B-013 vs partial-coverage `DIVERGED: language-forced`) per Adversarial Review Finding 6. Annotate single-view navigation `DIVERGED: upstream-gap-forced` per Finding 4. Land with row-1479 test.
4. ~~**Step 4: G3 row `emVirtualCosmosModel-accessor-model-change`.**~~ **MERGED via B-014 at c2871547** (live at `crates/emmain/src/emVirtualCosmos.rs:215-287`).

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

## Adversarial Review — 2026-05-01 (emmain slice)

### Summary
- Critical: 2 | Important: 4 | Minor: 2 | Notes: 1

### Findings

1. **[Critical] G3 / `emVirtualCosmosModel-accessor-model-change` already implemented by B-014** — `crates/emmain/src/emVirtualCosmos.rs:215-271` already carries `change_signal: Cell<SignalId>` (line 225), the combined-form `GetChangeSignal(&self, ectx: &mut impl SignalCtx)` (line 262), and a `CALLSITE-NOTE` on `Reload` (line 277-287) explaining why the bootstrap fire is omitted (benign per D-007/D-008 composition). work-order.md entry "2026-04-28 — B-014 merged" §3 confirms `emVirtualCosmos-575` carried this implementation. **The B-004 G3 row is duplicate work; mark merged in work-order.md and strike the row from this design's scope before dispatch.** The design's §G3 §"Fix" code block is contradicted by the merged source.

2. **[Critical] Design code samples reference non-existent fields on `emBookmarkRec`** — design doc lines 245-249 use `self.bookmark.entry.LocationIdentity` / `LocationRelX` / `LocationRelY` / `LocationRelA`, but `emBookmarkRec` (defined at `crates/emmain/src/emBookmarks.rs:104-112`) holds those four fields **directly on the rec**, not nested under `.entry` (`.entry` is the `emBookmarkEntryBase` carrying `Name`/`Icon`/`BgColor`/`FgColor` only). Naively pasting the design's snippet won't compile. Fix: write `self.bookmark.LocationIdentity`, `self.bookmark.LocationRelX`, etc.

3. **[Important] `view().Visit(...)` shim does not exist on `PanelCtx`** — design §G2 reaction calls `ctx.view().Visit(&id, x, y, a)`, and Open Question 4 acknowledges this. Reality is more constrained than the OQ suggests: `PanelCtx` (`emEngineCtx.rs:495-531`) has no `view` field and no view accessor. The actual `emView::Visit` (`emView.rs:1094-1107`) takes 8 args (`tree, panel, rel_x, rel_y, rel_a, adherent, ctx: &mut SchedCtx`), and `VisitByIdentity` (`:1115-1137`) takes 7. The 4-arg call in the design is fictional. The implementer needs either (a) a new `pctx.visit_identity(...)` helper that reaches into the owning view through the framework, or (b) a deferred-action enqueued onto `pending_actions` for the framework to drain. Option (b) matches existing closure-rail pattern — recommend over (a) and resolve before dispatch.

4. **[Important] Missing `ContentView` field — C++ contract not preserved** — C++ `emBookmarkButton` carries `ContentView` (`emBookmarks.cpp:1470,1523-1535`) and visits *that* view, which may differ from the button's owning view (per the bookmark navigator design). Rust `emBookmarkButton` (`emBookmarks.rs:528-535`) has no `content_view` field; design §G2 silently elides this. If the navigation target is "the panel's own view" the test passes accidentally, but a multi-view bookmark configuration would observably diverge. The design must either (a) add a `content_view` reference (Rc/Weak — needs ownership-model decision) or (b) annotate as `DIVERGED: upstream-gap-forced` because Rust hasn't ported the multi-view content/control split yet, with a follow-up tracking row.

5. **[Important] `emBookmarkButton::new` ConstructCtx threading is impossible at the existing callsite** — design §G2 line 115 specifies `pub fn new<C: ConstructCtx>(cc: &mut C, bookmark)`, but the only caller is `emBookmarksPanel::LayoutChildren` at `emBookmarks.rs:736` which has `&mut PanelCtx`, not `&mut C: ConstructCtx`. (LayoutChildren-time construction with no ConstructCtx is the same constraint that forced the B-004 emcore-slice to use deferred allocation — see work-order entry "2026-04-29 — B-004 emcore-slice merged" amendment 1.) Fix: keep `new(bookmark)`, add `click_signal: Cell<SignalId>` initialized null, and `ensure_click_signal(ectx)` allocates lazily in Cycle. This mirrors the merged emcore-slice precedent and the B-014 G3 precedent, so the implementer must apply the deferred-fire pattern, not the ConstructCtx-threading pattern shown in the brief.

6. **[Important] Click-detection is not just "copy emButton::Input"** — Open Question 3 and §G2 understate the work. `emBookmarkButton` has no `input` impl on `PanelBehavior` today (verified by reading the impl block at `emBookmarks.rs:573-646`). Adding LP_LEFT click detection requires also handling focus/active state, mouse capture, hover repaint, and key-activation (Enter/Space) parity with `emButton::Input` (`emButton.rs:331-475`). A "minimal subset" is not minimal — the C++ inherited behavior is multi-hundred-line. Either accept observable divergence vs C++ for non-LP-LEFT activation paths and annotate, or escalate to defer the row to B-013 (rc-shim-emcore button-handling) which the brief itself already cites (line 101) as the proper home. Recommend defer.

7. **[Minor] Test sketches use stale `GetChangeSignal` arity** — design line 331 calls `model.borrow().GetChangeSignal()` with no args; merged code requires `(&self, &mut impl SignalCtx)`. Moot if Finding 1 is applied (row dropped), but flag for any retained scaffolding.

8. **[Minor] Decisions cited list is stale** — §"Decisions cited" mentions only D-006 and D-003. D-007 (mutator-fire shape, `&mut impl SignalCtx`), D-008 (lazy allocation, combined form), and D-009 (no polling intermediaries) all bind the remaining work; their omission is what allowed Findings 5–7 to slip through. Add a citation block.

### Note
- The C++ `emBookmarkButton::Cycle` body (`emBookmarks.cpp:1511-1538`) also calls `emButton::Cycle()` and runs an `UpToDate` Update path. The design's reaction body is single-branch and ignores both. M-001 ("Verify C++ Cycle branch structure directly") applies — implementer must read 1511-1538 before writing the Rust Cycle.

### Recommended Pre-Implementation Actions
1. Strike the `emVirtualCosmosModel-accessor-model-change` row from B-004 scope and reconcile to `merged` in work-order.md citing B-014 c2871547.
2. Replace the §G2 §"Fix" code block: drop `cc: &mut C: ConstructCtx`; add `click_signal: Cell<SignalId>` + `subscribed_init: bool`; allocate via `ensure_click_signal(ectx)` in Cycle (D-008 A1 combined form on the accessor).
3. Resolve the `ContentView` question: decide (a) add field with ownership story, or (b) annotate DIVERGED upstream-gap-forced and defer multi-view semantics.
4. Resolve the view-Visit threading question: pick `pending_actions` deferred-action or escalate; do not leave as Open Question 4.
5. Decide whether to defer click-detection to B-013 or accept partial-coverage divergence with annotation.
6. Fix `bookmark.entry.Location*` → `bookmark.Location*` in §G2 and tests.
7. Add D-007 / D-008 / D-009 to §"Decisions cited" and walk §G2 against M-001 (read `emBookmarks.cpp:1511-1538`).

## Amendment Log — 2026-05-01

Reconciliation pass folding Adversarial Review findings into the design body. Adversarial Review section above is preserved verbatim as the source-of-truth audit trail.

| Finding | Resolution |
|---|---|
| **C-1** (G3 already implemented by B-014) | **Verified** at `crates/emmain/src/emVirtualCosmos.rs:215-287`: `change_signal: Cell<SignalId>` (line 225), combined-form `GetChangeSignal(&self, &mut impl SignalCtx)` (line 262), `CALLSITE-NOTE` on `Reload` (lines 277-287). Row `emVirtualCosmosModel-accessor-model-change` **struck from B-004 scope**: header status, scope summary, row table, accessor-group §G3, wiring-shape §"emVirtualCosmosModel::Reload", implementation-sequencing Step 4, and test sketch all updated to point to merged code. |
| **C-2** (`bookmark.entry.LocationIdentity` etc. — non-existent) | **Fixed.** `LocationIdentity`, `LocationRelX/Y/A` are direct on `emBookmarkRec` (`crates/emmain/src/emBookmarks.rs:107-110`); `.entry: emBookmarkEntryBase` carries only `Name`/`Icon`/`BgColor`/`FgColor`. All design-body and test-sketch references rewritten to `self.bookmark.LocationIdentity` / `self.bookmark.LocationRel{X,Y,A}`. |
| **I-1** (`PanelCtx::view()` does not exist; `emView::Visit` arity) | **Fixed.** Replaced the fictional `ctx.view().Visit(...)` 4-arg call with a deferred-action `ctx.enqueue_visit_by_identity(id, rx, ry, ra)` enqueue onto the framework's pending-actions rail (the canonical closure-rail pattern). The framework drains and invokes `emView::VisitByIdentity` (`emView.rs:1115-1137`, 7-arg) at a point where the necessary `tree`/`panel`/`adherent`/`SchedCtx` arguments are reachable. |
| **I-2** (no `ContentView` field on Rust `emBookmarkButton`) | **Fixed.** Annotated the navigation reaction `DIVERGED: upstream-gap-forced` — Rust `emView` has not ported the multi-view content/control split, so `emBookmarkButton::ContentView` (C++ `emBookmarks.cpp:1470,1523-1535`) cannot be ported. Single-view navigation is the only available shape; multi-view bookmarks observably diverge. Tracked as a follow-up row, not an in-bucket fix. |
| **I-3** (`emBookmarkButton::new` cannot take `ConstructCtx`) | **Fixed.** Constructor signature stays `new(bookmark: emBookmarkRec)`. `click_signal: Cell<SignalId>` initialized to `SignalId::null()`; D-008 A1 combined-form `GetClickSignal(&self, &mut impl SignalCtx)` lazy-allocates on first call. Mirrors B-014's `emVirtualCosmosModel::change_signal` precedent and the merged emcore-slice's deferred-allocation pattern (work-order 2026-04-29 amendment 1). |
| **I-4** (click-detection ≠ "minimal subset" of `emButton::Input`) | **Flagged, not transferred.** Per task constraint (B-013 is its own bucket; row-transfer is out of scope), the design now explicitly recommends two implementer-choice options: (a) defer click-detection to B-013, landing only the accessor + Cycle reaction skeleton in this bucket; (b) accept partial-coverage divergence (LP_LEFT mouse-down only) annotated `DIVERGED: language-forced` with focus/Enter/Space/capture parity deferred. Recommendation: (a). |
| **Minor 7** (test sketch uses stale 0-arg `GetChangeSignal`) | **Moot** (row struck per C-1) but the test sketch was rewritten to remove the call entirely and point to the merged implementation. |
| **Minor 8** (decisions-cited list stale) | **Fixed.** §"Decisions cited" now also cites D-007 (mutator-fire shape), D-008 A1 (combined-form lazy allocation), and D-009 (no polling intermediaries). |
| **Note** (M-001 — read C++ Cycle 1511-1538) | **Recorded** as an in-line implementer reminder in the amended `Cycle` sketch. Implementer must walk the C++ `emButton::Cycle()` + `UpToDate` Update path before finalizing the Rust Cycle body. |

**Net dispatch state.** B-004 scope post-reconciliation = **1 row**: `emBookmarks-1479`. The bucket is dispatchable once the implementer picks (a) or (b) for click-detection (Finding I-4); all other amendment items are concrete code-level guidance now folded into §G2 and §"Wiring-shape application".
