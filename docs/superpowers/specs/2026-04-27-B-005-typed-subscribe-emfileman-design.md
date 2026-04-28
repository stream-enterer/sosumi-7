# B-005-typed-subscribe-emfileman — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-005-typed-subscribe-emfileman.md`
**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** emfileman, 21 rows
**Mechanical-vs-judgement:** mechanical-heavy after D-006 lands

## Summary

Wire 21 missing signal subscriptions in two emfileman panels, mirroring the C++ `AddWakeUpSignal(...) + IsSignaled(...) in Cycle()` shape. The bucket pilots a new global decision **D-006-subscribe-shape** that becomes the canonical port shape for every future P-002 bucket.

## Decisions cited

- **D-006-subscribe-shape (proposed by this design, to be reconciled into `decisions.md`).** Canonical subscribe-and-react shape for P-002. See §"Proposed D-006" below.
- **D-001-typemismatch-accessor-policy (cross-reference only).** `FMModel.GetSelectionSignal` and `FMVConfig.GetChangeSignal` are P-003 type-mismatch rows owned by another bucket. B-005's Cycle body must include them in its `IsSignaled` checks (mirroring C++ Cycle 360–446) but the accessor flip is out of scope here. See §"Cross-bucket interaction" below.

No other decisions in `decisions.md` (D-002–D-005) apply to B-005.

## Proposed D-006-subscribe-shape

> **Question.** What is the canonical Rust shape for the C++ `AddWakeUpSignal(sig)` ctor + `IsSignaled(sig)` Cycle pattern, given that the Rust port registers panel-engines *after* panel `new()` returns (and so panel constructors cannot call `Scheduler::connect`)?
>
> **Options considered.**
>
> - **A. First-Cycle init.** Panel adds a `subscribed_init: bool` field. First `Cycle()` invocation calls `ectx.connect(sig, ectx.id())` for every signal of interest, sets the flag, then runs the regular `IsSignaled` checks. Subsequent `Cycle()` invocations skip the init block.
> - **B. Deferred-queue at construction.** Mirror `emDialog::add_pre_show_wake_up_signal` — extend `ConstructCtx` with a panel-side `add_init_wake_up_signal` queue that the framework drains during `register_engine_for`. Allows constructor-side authoring at the cost of a new framework surface.
> - **C. Extend ConstructCtx with `connect`.** Requires registering the panel's engine before behavior `new()` runs, which inverts the current panel-tree lifetime (`set_behavior` precedes `register_engine_for`). Largest blast radius, no clear payoff.
>
> **Chosen direction (recommended).** **A. First-Cycle init.**
>
> **Why.** Honest local fix: works today with zero framework changes; uses APIs already in `EngineCtx` (`connect`, `id`); preserves the C++ Cycle-body structure verbatim. The construct-time signal registration in C++ is enabled by base-class virtual dispatch during ctor, which Rust intentionally doesn't replicate. B is a viable fallback if a future P-002 panel can't run a first-Cycle init (e.g., needs subscriptions live before its first wake), but no B-005 row needs it.
>
> **Relationship to D-005 (poll-replacement-shape).** D-005 says "direct subscribe — Cycle re-reads relevant state when signal arrives." D-006 is *how* that subscription is established (where the `connect` call lives) and *how* the reaction is structured (`IsSignaled` checks at top of Cycle, reactions inline). They are complementary: D-005 picks the reaction model, D-006 picks the wiring shape. Working-memory session: please add a `see D-006` pointer to D-005 during reconciliation.

## Cross-bucket interaction

`emFileManControlPanel`'s C++ `Cycle` body checks `FMModel.GetSelectionSignal()` and `FMVConfig.GetChangeSignal()` (lines 365–369). Both are P-003 type-mismatch accessors owned by **D-001** and assigned to a different bucket. B-005's connect-list and Cycle body must include them — the panel observably reacts to these signals — but the *type flip* and the consumer-migration-from-`u64` work is out of B-005's scope.

The implementer marks each affected line with `// see D-001 — accessor returns u64 today; flip pending` and the inventory-enriched.json edge `prereq: D-001` for those rows is wired by the working-memory session.

> **PREREQ SATISFIED post-B-009 merge (2026-04-28, 50994e26):** D-001 has landed. The three flipped accessors (`GetSelectionSignal`, `GetCommandsSignal`, `GetChangeSignal` on `emFileManViewConfig`) now return `SignalId` and use the **combined-form** signature `GetXxxSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` (per D-008 A1 amendment). The "see D-001" annotations and pending-flip caveats below are obsolete; the connect-list compiles unchanged in shape but each call passes `ectx` to the accessor (e.g., `ectx.connect(fmm.GetSelectionSignal(ectx), eid)`). Implementer: drop the caveat comments; use combined-form accessor calls.

This is not a bucket-boundary violation: B-005's rows are exclusively the 21 P-002 sites listed in the bucket sketch. The two P-003 accessors are *consumed* by B-005's design but *fixed* elsewhere.

## File-by-file plan

### `crates/emfileman/src/emFileManControlPanel.rs` — 20 rows

Add field:

```rust
/// First-Cycle init flag for D-006-subscribe-shape.
subscribed_init: bool,
```

Initialize to `false` in the constructor's struct literal. The constructor acquires no other new responsibilities — in particular, the existing `sync_from_config` scratch-`PanelCtx` pre-init at construction stays exactly as-is.

Replace the existing `Cycle` body (or add one if absent) with:

```rust
fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx) -> bool {
    if !self.subscribed_init {
        let eid = ectx.id();
        let fmm = self.file_man.borrow();
        let fmc = self.config.borrow();
        // Cross-bucket: see D-001. Today these accessors return u64; the
        // type flip is owned by the P-003 / emfileman accessor bucket.
        ectx.connect(fmm.GetSelectionSignal(), eid);
        ectx.connect(fmc.GetChangeSignal(), eid);
        drop(fmm);
        drop(fmc);
        // Group-level radio check signals (C++ -328, -329).
        ectx.connect(self.theme_ar_group.borrow().check_signal, eid);
        ectx.connect(self.theme_style_group.borrow().check_signal, eid);
        // Per-button radio check signals (C++ -330..-335 sort, -338..-340 nss).
        for rb in &self.sort_radios   { ectx.connect(rb.borrow().check_signal, eid); }
        for rb in &self.nss_radios    { ectx.connect(rb.borrow().check_signal, eid); }
        // Checkbox signals (C++ -336, -337, -341).
        ectx.connect(self.dirs_first_check.check_signal, eid);
        ectx.connect(self.show_hidden_check.check_signal, eid);
        ectx.connect(self.autosave_check.check_signal, eid);
        // Action button click signals (C++ -342..-347).
        ectx.connect(self.save_button.click_signal, eid);
        ectx.connect(self.select_all_button.click_signal, eid);
        ectx.connect(self.clear_sel_button.click_signal, eid);
        ectx.connect(self.swap_sel_button.click_signal, eid);
        ectx.connect(self.paths_clip_button.click_signal, eid);
        ectx.connect(self.names_clip_button.click_signal, eid);
        self.subscribed_init = true;
    }

    // Body mirrors C++ emFileManControlPanel::Cycle (lines 360–446).
    // For each IsSignaled branch: relocate the corresponding reaction
    // currently in Input() into this Cycle body. The widgets themselves
    // continue to fire their click_signal/check_signal via their own
    // Input handling — the panel's job is to react, which now happens
    // here instead of in Input().
    // ...
    false
}
```

The reaction code that today lives in `emFileManControlPanel::Input` (and the per-widget `last_*` polling around `last_config_gen`) moves into Cycle, gated by `ectx.IsSignaled(...)` checks, in the exact branch order of C++ Cycle 360–446. The Rust diff per row is:
- `emFileManControlPanel-330..-335` (sort radios) → one `if ectx.IsSignaled(self.sort_radios[i].borrow().check_signal) { self.config.borrow_mut().SetSortCriterion(SortCriterion::variant_for(i)); }` per index.
- `emFileManControlPanel-338..-340` (nss radios) → analogous, with `SetNameSortingStyle`.
- `emFileManControlPanel-328, -329` (theme groups) → composite branch matching C++: read both indices, clamp to bounds, call `SetThemeName(...)`.
- `emFileManControlPanel-336, -337, -341` (checkboxes) → setter call mirroring C++.
- `emFileManControlPanel-342..-347` (action buttons) → mutator call mirroring C++ `SaveAsDefault`, `SelectAll` (with the dir-panel walk), `ClearSourceSelection+ClearTargetSelection`, `SwapSelection`, `SelectionToClipboard(_, false, false)`, `SelectionToClipboard(_, false, true)`.

Audit-trail rule: even though many rows currently anchor at `emFileManControlPanel.rs:300` (the constructor block), each row is realized by a *distinct* `connect` call and a *distinct* `IsSignaled` branch. The diff is naturally one-edit-per-row at the Cycle level, even if the constructor block edit is a single hunk.

The `select_all_button` reaction needs the dir-panel walk currently performed in C++ — `for (p=ContentView.GetActivePanel(); p; p=p->GetParent()) dynamic_cast<emDirPanel*>(p)`. Rust currently does this in Input via `self.dir_path` cached resolution; the design preserves the cached-`dir_path` shortcut and only relocates the trigger from Input to Cycle. (If the implementer finds the cache is stale-by-design and the C++ shape needs walking on every signal, that's a follow-up correctness item — not a B-005 design choice.)

### `crates/emfileman/src/emFileLinkPanel.rs` — 1 row (`emFileLinkPanel-53`)

Add `subscribed_init: bool` field. Existing `Cycle` (line 175) gains a first-Cycle init block:

```rust
if !self.subscribed_init {
    let eid = ectx.id();
    let model = emFileModel::Acquire(&self.ctx);
    ectx.connect(model.borrow().GetUpdateSignal(), eid);
    self.subscribed_init = true;
}
// Existing Cycle body, plus new branch:
let model = emFileModel::Acquire(&self.ctx);
if ectx.IsSignaled(model.borrow().GetUpdateSignal()) {
    pctx.invalidate_painting(self_panel_id);
}
```

Mirrors C++ `emFileLinkPanel.cpp:53` (`AddWakeUpSignal(FileModel->GetUpdateSignal())`) plus the `Cycle` body's `if (IsSignaled(FileModel->GetUpdateSignal())) InvalidatePainting()` site.

This row is structurally distinct from the 20 control-panel rows (model-update-broadcast vs. widget-signal). The wire-up *shape* is identical (D-006); only the source signal and the reaction differ.

## Implementation ordering

1. Resolve D-006 (working-memory session reconciliation step).
2. Land `emFileLinkPanel-53` first as the smallest validation of the D-006 shape (one connect, one Cycle branch, isolated panel).
3. Land `emFileManControlPanel` 20 rows as a single PR — the constructor field, the first-Cycle init block, and the relocated Cycle body together. Splitting per-row creates an inconsistent intermediate state where some signals fire-and-forget while others fire-and-react.

Both panels can land independently (no inter-panel prereq).

## Verification strategy

The C++ → Rust observable contract for these 21 rows is: **firing the signal causes the documented Cycle reaction.** The existing DIVERGED note in `emFileManControlPanel.rs:43-46` (manual paint vs child-panel composition) means pixel-level golden tests don't exercise this drift. Verification is **behavioral**:

A new test file `crates/emfileman/tests/typed_subscribe_b005.rs` (RUST_ONLY: dependency-forced — there is no C++ test analogue, the C++ test surface is the X11 integration suite) covers all 21 rows. Per-row pattern:

```rust
// fire(widget.click_signal); run_cycle(); assert(config setter took effect)
let mut h = Harness::new();
let panel = h.create_fileman_control_panel();
let initial = h.config().GetSortCriterion();
h.fire(panel.sort_radios[2].borrow().check_signal);
h.run_cycle();
assert_ne!(h.config().GetSortCriterion(), initial);
assert_eq!(h.config().GetSortCriterion(), SortCriterion::ByClass);
```

For `emFileLinkPanel-53`: fire `model.UpdateSignal`, run Cycle, assert the panel's `panel_id` appears in the tree's invalidation set.

The harness reuses the existing `emcore::emEngineCtx::PanelCtx`-with-real-scheduler test fixtures already in use across emcore (`crates/emcore/src/emCheckButton.rs:522`-style). No new harness primitives required.

**Before/after evidence per row** is the audit's four-question standard: (1) is the signal connected? (2) does Cycle observe it? (3) does the reaction fire the documented mutator? (4) is the C++ `IsSignaled` branch order preserved? Tests answer (1)–(3); code review against C++ Cycle answers (4).

## Open items deferred to working-memory session

1. **Reconcile proposed D-006 into `decisions.md`** with stable ID, and back-propagate "see D-006" pointer into D-005.
2. **Inventory-enriched.json edges:** the 20 `emFileManControlPanel` rows gain a soft prereq on the P-003 / D-001 bucket (their connect-list mentions `GetSelectionSignal` and `GetChangeSignal`). The connect calls themselves work today against `u64` accessors only if `connect` accepts `u64` — which it doesn't (`connect: SignalId`). So D-001 is in fact a **hard prereq** for the two cross-bucket lines in the connect block. The 18 widget-signal connects and the entire Cycle body (excluding the two model/config branches) are *not* prereq-blocked.
3. **Bucket-sketch stale prose:** the B-005 bucket file currently asserts "decisions.md currently contains no resolved D-### entries" (false — D-001..D-005 exist) and "pattern catalog entry not yet authored" (false — pattern-catalog.md §P-002 exists). Recommend the working-memory session refresh the sketch text in a follow-up commit.
4. **Cross-bucket row anomaly (informational, not reassignment):** `emFileLinkPanel-53` is the only model-update-broadcast row in B-005. If a future bucket consolidates model-update-broadcast P-002 sites, this row may move; for now, B-005's design accommodates it cleanly.

## Success criteria

- All 21 rows have a `connect(...)` call in their panel's first-Cycle init block.
- All 21 rows have a corresponding `IsSignaled(...)` branch in their panel's Cycle body, in C++ source order.
- Existing Input()-inline reactions for these widgets are deleted from `emFileManControlPanel::Input`.
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- New `tests/typed_subscribe_b005.rs` covers all 21 rows; every assertion passes.
- B-005 status in `work-order.md` flips `pending → designed` (working-memory session reconciliation).
