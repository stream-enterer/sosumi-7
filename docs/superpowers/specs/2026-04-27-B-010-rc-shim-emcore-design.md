# B-010-rc-shim-emcore — Design

**Bucket:** B-010-rc-shim-emcore
**Pattern:** P-004-rc-shim-instead-of-signal
**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Source bucket file:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-010-rc-shim-emcore.md`
**Cited decisions:**
- D-002-rc-shim-policy (per-row triage; all 15 rows resolve to rule 1, convert).
- D-006-subscribe-shape (first-Cycle init block + `IsSignaled` checks at top of Cycle, per host panel).
- D-009-polling-intermediary-replacement (proposed; promotes the watch-list pattern to a real decision; see §3).

**Prereq buckets:** none (inbound or outbound).

**New global decisions:** **D-009-polling-intermediary-replacement** (proposed; sightings 3 and 4 occur inside this bucket, satisfying the 3-sighting promotion threshold).

---

## 0. Bucket overview

15 P-004 rows in two emcore sub-buckets:
- **9 rows in `crates/emcore/src/emCoreConfigPanel.rs`** (rows 80, 299, 300, 301, 563, 746, 755, 773, 791): widget→config rc-shim closures (`on_check` / `on_value` / `on_click`) that capture an `Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>` and mutate config + save. C++ uses `AddWakeUpSignal(widget->Get*Signal())` + `IsSignaled(...)` in the host panel's `Cycle()`.
- **6 rows in `crates/emcore/src/emFileSelectionBox.rs`** (rows 514, 521, 531, 532, 540, 550): the rows ARE the `FsbEvents` aggregator. Closures push to a shared `Rc<RefCell<FsbEvents>>` struct; the FSB's `Cycle` drains the struct on a later slice. C++ has no aggregator — `emFileSelectionBox::Cycle` (cpp:385) is a flat list of 7 `IsSignaled` branches in source order, each reading widget state directly via the widget's accessor methods.

All 15 rows are uniformly **rule-1 convert** per D-002. No row qualifies as rule 2 (no post-finish/post-cycle member-field assignment shape). No forced-category candidates (every shim is project-internal Rust convenience; the audit's "0 forced" headline holds).

Two new sightings of the polling-intermediary watch-list pattern surface inside this bucket — `FsbEvents` (in scope; resolved here by the rule-1 convert) and the `generation: Rc<Cell<u64>>` counter (out of B-010 row scope; tangled with row 80's reset reaction body but not owned by any bucket row). Together with the prior sightings in B-003 and B-012, this gives 4 total — past the user-set 3-sighting promotion threshold. D-009 is proposed in §3 with the user-ratified phrasing.

---

## 1. Per-row triage table

| # | Row | Host panel (Rust today) | C++ Cycle host | Widget signal | Reaction body (in Cycle) | D-002 |
|---|---|---|---|---|---|---|
| 1 | emCoreConfigPanel-80 | `ButtonsPanel` | `emCoreConfigPanel::Cycle` (cpp:42) | `emButton::click_signal` | Reset every config field to default + `Save()` + `generation.set(generation.get() + 1)` | rule 1 |
| 2 | emCoreConfigPanel-299 | `MouseMiscGroup` | `MouseMiscGroup::Cycle` (cpp:~245) | `emCheckBox::check_signal` (Stick) | If `IsChecked()` ≠ `StickMouseWhenNavigating`, set + `Save()` | rule 1 |
| 3 | emCoreConfigPanel-300 | `MouseMiscGroup` | same | `emCheckBox::check_signal` (Emu) | If `IsChecked()` ≠ `EmulateMiddleButton`, set + `Save()` | rule 1 |
| 4 | emCoreConfigPanel-301 | `MouseMiscGroup` | same | `emCheckBox::check_signal` (Pan) | If `IsChecked()` ≠ `PanFunction`, set + `Save()` | rule 1 |
| 5 | emCoreConfigPanel-563 | `MemFieldLayoutPanel` | `MaxMemGroup::Cycle` | `emScalarField::value_signal` | `MaxMegabytesPerView.SetValue(mem_val_to_cfg(GetValue()))` + `Save()` | rule 1 |
| 6 | emCoreConfigPanel-746 | `CpuGroup` | `PerformanceGroup::Cycle` (cpp:663) | `emScalarField::value_signal` (MaxRenderThreads) | If GetValue rounded/clamped ≠ `MaxRenderThreads`, set + `Save()` | rule 1 |
| 7 | emCoreConfigPanel-755 | `CpuGroup` | same | `emCheckBox::check_signal` (AllowSIMD) | If `IsChecked()` ≠ `AllowSIMD`, set + `Save()` | rule 1 |
| 8 | emCoreConfigPanel-773 | `PerformanceGroup` | same | `emScalarField::value_signal` (Downscale) | If GetValue rounded/clamped ≠ `DownscaleQuality`, set + `Save()` | rule 1 |
| 9 | emCoreConfigPanel-791 | `PerformanceGroup` | same | `emScalarField::value_signal` (Upscale) | If GetValue rounded/clamped ≠ `UpscaleQuality`, set + `Save()` (+ InvalidatePainting per C++ cpp:710) | rule 1 |
| 10 | emFileSelectionBox-514 | `emFileSelectionBox` | `emFileSelectionBox::Cycle` (cpp:396) | `emTextField::text_signal` (ParentDirField) | Read `GetText()`, update `parent_dir`, `InvalidateListing()` | rule 1 |
| 11 | emFileSelectionBox-521 | `emFileSelectionBox` | (cpp:405) | `emCheckBox::check_signal` (HiddenCheckBox) | Read `IsChecked()`, set `hidden_files_shown`, `InvalidateListing()` | rule 1 |
| 12 | emFileSelectionBox-531 | `emFileSelectionBox` | (cpp:413) | `emListBox::selection_signal` (FilesLB) | Read `GetSelectedIndices()`, update `selected_names`, fire FSB `selection_signal` | rule 1 |
| 13 | emFileSelectionBox-532 | `emFileSelectionBox` | (cpp:419) | `emListBox::item_trigger_signal` (FilesLB) | Read `GetTriggeredItemIndex()`, `EnterSubDir` or `TriggerFile` | rule 1 |
| 14 | emFileSelectionBox-540 | `emFileSelectionBox` | (cpp:434) | `emTextField::text_signal` (NameField) | Read `GetText()`, update `selected_names`, fire FSB `selection_signal` | rule 1 |
| 15 | emFileSelectionBox-550 | `emFileSelectionBox` | (cpp:469) | `emListBox::selection_signal` (FiltersLB) | Read `GetSelectedIndex()`, update `selected_filter_index`, `InvalidateListing()` | rule 1 |

**Rule-2 candidates: none.** Every row's C++ site is a button/check/value/text/selection signal at construction-time subscribe + `IsSignaled` in the host panel's Cycle. Verified by direct read of `src/emCore/emCoreConfigPanel.cpp` and `src/emCore/emFileSelectionBox.cpp` — no row uses a post-finish or post-cycle member-field assignment. (The B-013 lesson: rule-2 framing can be misapplied; for this bucket, every C++ site was confirmed against the 4-question test and none qualified.)

**Forced-category candidates: none.** All rc-shim closures and the `FsbEvents` aggregator are project-internal Rust convenience. Per Port Ideology, unannotated drift is fidelity-bug.

**Structural mismatch noted (rows 5, 6, 7).** Rust splits some C++ panels into more layers — `MemFieldLayoutPanel` lives inside `MaxMemGroup`; `CpuGroup` lives inside `PerformanceGroup`. The host-panel column above identifies the Rust panel that owns the widget today (and thus where the closure lives today and the new Cycle goes). The C++ Cycle host column shows where the C++ original IsSignaled lives. The structural difference (extra Rust layer) is pre-existing and out of B-010 scope; aligning Rust to C++ class structure would be a separate refactor.

---

## 2. Architecture

### 2.1 emCoreConfigPanel widget→config (rows 1–9)

Each of the 5 host panels (`ButtonsPanel`, `MouseMiscGroup`, `MemFieldLayoutPanel`, `CpuGroup`, `PerformanceGroup`) gains:

- `subscribed_init: bool` field, initialised `false` in the panel's `new()`.
- One `*_sig: SignalId` field per widget the panel owns (init `SignalId::null()` in `new()`).
- One `*_id: Option<PanelId>` field per widget the panel owns (init `None` in `new()`).
- A `Cycle` impl on the existing `impl PanelBehavior for X` block.

In `create_children` (the existing site that builds and parents the widget), capture the widget's signal id one line BEFORE `ctx.create_child_with(...)` consumes it, then capture the returned PanelId:

```rust
// Before:
//   let id = ctx.create_child_with("stick", Box::new(CheckBoxPanel { check_box: stick }));
// After:
self.stick_sig = stick.check_signal;       // SignalId is Copy; pre-capture before move.
let id = ctx.create_child_with("stick", Box::new(CheckBoxPanel { check_box: stick }));
self.stick_id = Some(id);
```

The on_check / on_value / on_click closures get **deleted entirely.** Widget signals fire automatically on user interaction (handled inside the widget's own `Input`/`Cycle`); no closure plumbing is needed to "publish" anything.

The new `Cycle` body follows the canonical D-006 first-Cycle init shape:

```rust
fn Cycle(
    &mut self,
    ectx: &mut crate::emEngineCtx::EngineCtx<'_>,
    ctx: &mut PanelCtx,
) -> bool {
    if !self.subscribed_init {
        let eid = ectx.id();
        ectx.connect(self.stick_sig, eid);
        ectx.connect(self.emu_sig, eid);
        ectx.connect(self.pan_sig, eid);
        self.subscribed_init = true;
        ectx.wake_up();   // Schedule periodic Cycle for IsSignaled checks.
    }
    if ectx.IsSignaled(self.stick_sig) {
        let checked = ctx.tree
            .with_behavior_as::<CheckBoxPanel, _>(
                self.stick_id.expect("stick_id set in create_children"),
                |p| p.check_box.IsChecked()
            )
            .unwrap_or(false);
        let mut cm = self.config.borrow_mut();
        let mut sched = ctx.as_sched_ctx().expect("sched");
        cm.modify(
            |c, sc| {
                if *c.StickMouseWhenNavigating.GetValue() != checked {
                    c.StickMouseWhenNavigating.SetValue(checked, sc);
                }
            },
            &mut sched,
        );
        let _ = cm.TrySave(false);
    }
    // … same shape for emu_sig, pan_sig
    false
}
```

The `with_behavior_as::<T, _>(panel_id, |p| ...)` typed downcast (`emPanelTree.rs:1714`) is the precedent already used elsewhere in this file and in `emColorFieldFieldPanel.rs`. It returns `Option<R>`; `None` indicates the child panel was destroyed mid-Cycle (rare; handle by `unwrap_or(default)` per row).

The `IsSignaled` branches mirror C++ source order (same order the C++ Cycle body uses) so handler firing stays observable-equivalent.

#### 2.1.1 Reset row (row 80) — generation counter touch

`ButtonsPanel::Cycle`'s `IsSignaled(self.bt_reset_sig)` branch keeps the original closure body verbatim, including the trailing `generation.set(generation.get() + 1)` line. That bump triggers `LayoutChildren` to rebuild the widget tree on the next layout pass, which is how the Rust port today refreshes visible widget values after Reset. C++ does not have this mechanism — it uses `emRecListener::OnRecChanged()` per group → `UpdateOutput()` to re-sync widget values without rebuilding the tree.

The `generation` counter is **sighting #4 of D-009** (Cell-set-then-polled topology — Reset closure bumps it; `LayoutChildren` polls it to rebuild children). Removing it requires either porting `emRecListener` semantics or introducing a config-changed signal that all panels subscribe to — neither belongs in B-010's row scope. The reset row's reaction body **touches** the counter; it does not own it. Per the user direction during brainstorm: surface the touch in the reconciliation log; do not attempt the counter removal in B-010.

A future bucket (or a downstream pass on D-009) will own the counter removal.

### 2.2 emFileSelectionBox aggregator drop (rows 10–15)

The existing `Cycle` at `emFileSelectionBox.rs:1494` stays; its body is rewritten to mirror C++ `emFileSelectionBox::Cycle` (cpp:385).

**Field changes** on the FSB struct (`emFileSelectionBox.rs:714+`):
- Add `subscribed_init: bool`.
- Add 6 cached widget signal IDs: `dir_text_sig`, `hidden_check_sig`, `files_sel_sig`, `files_trigger_sig`, `name_text_sig`, `filter_sel_sig` — all `SignalId`, initialised `SignalId::null()` in `new()`.
- The 5 existing `*_id: Option<PanelId>` fields (`dir_field_id`, `hidden_cb_id`, `files_lb_id`, `name_field_id`, `filter_lb_id`) stay — already populated in `LayoutChildren`.
- **Delete** `events: Rc<RefCell<FsbEvents>>` field (rs:739) and its initialiser at rs:779.
- **Delete** the `FsbEvents` struct (rs:694-704) entirely.

**LayoutChildren** changes (rs:~1080-1250): for each of the 6 widgets, capture the widget's signal id one line before its `ctx.create_child_with(...)` call (e.g., `self.dir_text_sig = tf.text_signal;`). Delete the 7 closure setters: `tf.on_text` (dir field), `cb.on_check` (hidden), `lb.on_selection` (files), `lb.on_trigger` (files), `tf.on_text` (name field), `lb.on_selection` (filter), and the now-unused `events.clone()` capture preceding each.

**Cycle** rewrite (in C++ source order; mirrors cpp:385-501):

```rust
fn Cycle(&mut self, ectx: &mut crate::emEngineCtx::EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
    // Defer subscription until LayoutChildren has run and signal IDs are populated.
    if !self.subscribed_init && self.files_lb_id.is_some() {
        let eid = ectx.id();
        if self.dir_field_id.is_some() {
            ectx.connect(self.dir_text_sig, eid);
        }
        if self.hidden_cb_id.is_some() {
            ectx.connect(self.hidden_check_sig, eid);
        }
        ectx.connect(self.files_sel_sig, eid);
        ectx.connect(self.files_trigger_sig, eid);
        if self.name_field_id.is_some() {
            ectx.connect(self.name_text_sig, eid);
        }
        if self.filter_lb_id.is_some() {
            ectx.connect(self.filter_sel_sig, eid);
        }
        self.subscribed_init = true;
        ectx.wake_up();
    }

    // (file-models update signal, cpp:392 — out of B-010 scope; existing wiring preserved)

    if self.dir_field_id.is_some() && ectx.IsSignaled(self.dir_text_sig) {
        let new_text = ctx.tree
            .with_behavior_as::<TextFieldPanel, _>(
                self.dir_field_id.expect("dir_field_id"),
                |p| p.text_field.GetText().to_string(),
            )
            .unwrap_or_default();
        // … apply per cpp:396-403 (set parent_dir, InvalidateListing)
    }

    if self.hidden_cb_id.is_some() && ectx.IsSignaled(self.hidden_check_sig) {
        let checked = ctx.tree
            .with_behavior_as::<CheckBoxPanel, _>(
                self.hidden_cb_id.expect("hidden_cb_id"),
                |p| p.check_box.IsChecked(),
            )
            .unwrap_or(false);
        // … apply per cpp:405-411
    }

    if ectx.IsSignaled(self.files_sel_sig) && !self.listing_invalid {
        let indices = ctx.tree
            .with_behavior_as::<ListBoxPanel, _>(
                self.files_lb_id.expect("files_lb_id"),
                |p| p.list_box.GetSelectedIndices().to_vec(),
            )
            .unwrap_or_default();
        self.selection_from_list_box(&indices);
        // … fire FSB selection_signal per cpp:413-417
    }

    if ectx.IsSignaled(self.files_trigger_sig) {
        let trig_idx = ctx.tree
            .with_behavior_as::<ListBoxPanel, _>(
                self.files_lb_id.expect("files_lb_id"),
                |p| p.list_box.GetTriggeredItemIndex(),
            )
            .flatten();
        if let Some(idx) = trig_idx {
            // … apply per cpp:419-432 (EnterSubDir or TriggerFile)
        }
    }

    if self.name_field_id.is_some() && ectx.IsSignaled(self.name_text_sig) {
        let name_text = ctx.tree
            .with_behavior_as::<TextFieldPanel, _>(
                self.name_field_id.expect("name_field_id"),
                |p| p.text_field.GetText().to_string(),
            )
            .unwrap_or_default();
        // … apply per cpp:434-467
    }

    if self.filter_lb_id.is_some() && ectx.IsSignaled(self.filter_sel_sig) {
        let sel = ctx.tree
            .with_behavior_as::<ListBoxPanel, _>(
                self.filter_lb_id.expect("filter_lb_id"),
                |p| p.list_box.GetSelectedIndex(),
            )
            .flatten();
        // … apply per cpp:469-477 (set selected_filter_index, InvalidateListing)
    }

    false
}
```

Conditional connect/IsSignaled gating (`self.X_id.is_some()`) mirrors C++'s gated `AddWakeUpSignal` calls (cpp:510-551 wrap each `AddWakeUpSignal` in the same hidden-feature flags: `ParentDirFieldHidden`, `HiddenCheckBoxHidden`, `NameFieldHidden`, `FilterHidden`). FilesLB is always created in C++ and Rust both, so unconditional.

The `subscribed_init && files_lb_id.is_some()` guard handles the FSB lifecycle: FSB is constructed before its children exist; `LayoutChildren` creates children lazily on first auto-expand. The guard ensures connect calls run only after PanelIds are populated. One additional Cycle pass after first LayoutChildren completes — within D-006's deferred-first-Cycle-init idiom.

The handler bodies (omitted with `// … apply per cpp:###` comments above for brevity) translate the C++ logic 1:1, reusing the existing helpers in this file (`selection_from_list_box`, `enter_subdir`, `trigger_file`). Only the call sites move from event-drain branches to IsSignaled branches; the helpers themselves are unchanged.

### 2.3 No emcore framework changes

The bucket adds no new APIs to `emCheckBox`, `emTextField`, `emScalarField`, `emListBox`, `emButton`, `emPanelTree`, `emEngineCtx`, or `emRecNodeConfigModel`. Every accessor used (`IsChecked`, `GetText`, `GetValue`, `GetSelectedIndices`, `GetSelectedIndex`, `GetTriggeredItemIndex`) and every dispatcher used (`with_behavior_as::<T, _>`) already exists.

---

## 3. Proposed D-009-polling-intermediary-replacement

> **Question.** When the Rust port has a polling intermediary (Cell / RefCell field set in one site, polled by another engine's Cycle to fire a signal or trigger a reaction) where C++ fires/calls directly, what is the canonical fix?
>
> **Recommendation.** Remove the intermediary. Thread `&mut EngineCtx` into the original mutation site (or expose a typed method on the owning type that takes ectx and fires synchronously per D-007). Delete the intermediate Cell/RefCell field and any polling block that drains it.
>
> **Sightings.**
> 1. **B-003** — `AutoplayFlags.progress` (resolved by D-002 §1 R-A — drop AutoplayFlags entirely; the outbound progress channel becomes a `Rc<RefCell<emAutoplayViewModel>>` reference whose `GetItemProgress()` is read in `Paint`).
> 2. **B-012** — `mw.to_reload` chain through `MainWindowEngine` (resolved by `mw.ReloadFiles(&self, ectx)` per D-007 — ectx-threaded mutator on the owning Window type, fires `file_update_signal` synchronously, deletes the Cell + polling block).
> 3. **B-010** — `FsbEvents` (resolved by direct widget-state read in `IsSignaled` branches; `with_behavior_as::<T, _>` typed downcast to read child widget state at signal-fire time).
> 4. **B-010** — `generation: Rc<Cell<u64>>` counter on `emCoreConfigPanel` (out of B-010 row scope; resolution TBD when its owning bucket reaches it — likely a config-changed signal on `emRecNodeConfigModel` plus per-group subscribe + on-fire `UpdateOutput`-style handler, mirroring C++ `emRecListener::OnRecChanged()`).
>
> **Composes with.** D-007 (mutator-fire shape — ectx-threaded mutator that fires synchronously is the canonical replacement); D-006 (subscribe shape — first-Cycle init at the consumer side).
>
> **Affected forward-buckets.** Any bucket whose row data shows the same Cell-set-then-polled topology. Working-memory session should re-grep `inventory-enriched.json` for `evidence_kind = polling` plus `Rc<Cell<...>>` adjacency once D-009 lands.

---

## 4. File-by-file plan

### 4.1 `crates/emcore/src/emCoreConfigPanel.rs`

For each of the 5 host-panel structs:

**`ButtonsPanel`** (rs:1509 — host of row 80):
- Add fields: `bt_reset_sig: SignalId`, `reset_id: Option<PanelId>`, `subscribed_init: bool`.
- In `create_children` (rs:1532): capture `btn.click_signal` before `create_child_with`; capture returned PanelId.
- Delete `btn.on_click` closure (rs:1539-1568).
- Add `Cycle` impl (in `impl PanelBehavior for ButtonsPanel` rs:1573): first-Cycle init connects `bt_reset_sig`; `IsSignaled` branch runs the original reset reaction body verbatim, including the `generation.set(generation.get() + 1)` line.

**`MouseMiscGroup`** (rs:295 — host of rows 299, 300, 301):
- Add fields: `stick_sig`, `emu_sig`, `pan_sig: SignalId`; `stick_id`, `emu_id`, `pan_id: Option<PanelId>`; `subscribed_init: bool`.
- In `create_children` (rs:325): capture each `*.check_signal` before `create_child_with`; capture returned PanelIds.
- Delete `stick.on_check`, `emu.on_check`, `pan.on_check` closures (rs:341-385).
- Add `Cycle` impl: first-Cycle connects all 3; 3 `IsSignaled` branches per Section 2.1, each reading `IsChecked()` via `with_behavior_as::<CheckBoxPanel, _>`.

**`MemFieldLayoutPanel`** (rs:763 — host of row 563):
- Add fields: `mem_sig: SignalId`, `mem_id: Option<PanelId>`, `subscribed_init: bool`.
- In `create_children` (rs:783): capture `sf.value_signal` before `create_child_with`; capture returned PanelId.
- Delete `sf.on_value` closure (rs:801-808).
- Add `Cycle` impl: 1 `IsSignaled` branch reading `GetValue()` via `with_behavior_as::<ScalarFieldPanel, _>`, applying `mem_val_to_cfg` and clamp.

**`CpuGroup`** (rs:991 — host of rows 746, 755):
- Add fields: `threads_sig`, `simd_sig: SignalId`; `threads_id`, `simd_id: Option<PanelId>`; `subscribed_init: bool`.
- In `create_children` (rs:1023): capture both signals; capture returned PanelIds.
- Delete on_value (rs:1039-1046) and on_check (rs:1066-1072) closures.
- Add `Cycle` impl: 2 `IsSignaled` branches per Section 2.1.

**`PerformanceGroup`** (rs:1131 — host of rows 773, 791):
- Add fields: `downscale_sig`, `upscale_sig: SignalId`; `downscale_id`, `upscale_id: Option<PanelId>`; `subscribed_init: bool`.
- In `create_children` (rs:1167): capture both signals; capture returned PanelIds.
- Delete both on_value closures (rs:1205-1212, 1233-1240).
- Add `Cycle` impl: 2 `IsSignaled` branches. The Upscale branch additionally invokes the existing `InvalidatePaintingOfAllWindows` (per C++ cpp:710); port that helper if absent today (mechanical add, panel-tree walk).

For all 5 panels: add `fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx) -> bool` to the `impl PanelBehavior for X` block. Each `Cycle` ends with `ectx.wake_up()` ONLY at the end of first-Cycle init — subsequent cycles are scheduled by the connected signals firing.

### 4.2 `crates/emcore/src/emFileSelectionBox.rs`

- **Delete** `FsbEvents` struct (rs:694-704).
- **Delete** `events: Rc<RefCell<FsbEvents>>` field on `emFileSelectionBox` (rs:739) and its `Default::default` initialiser (rs:779).
- **Add** to `emFileSelectionBox` struct: `subscribed_init: bool`, `dir_text_sig: SignalId`, `hidden_check_sig: SignalId`, `files_sel_sig: SignalId`, `files_trigger_sig: SignalId`, `name_text_sig: SignalId`, `filter_sel_sig: SignalId`. Initialise all signals to `SignalId::null()` in `new()`.
- **In `LayoutChildren`** (rs:~1080-1250): capture each widget's signal id one line before its `ctx.create_child_with(...)` call (6 sites). Delete the 7 closure setters and their `events.clone()` captures (`tf.on_text` for dir field rs:1101-1106; `cb.on_check` for hidden rs:1120-1125; `lb.on_selection` rs:1155-1161 and `lb.on_trigger` rs:1163-1168 for files; `tf.on_text` for name rs:1205-1210; `lb.on_selection` for filter rs:1230-1235).
- **Rewrite `Cycle`** (rs:1494): replace the event-drain body with the first-Cycle init + 6 `IsSignaled` branches per Section 2.2. Each branch reads widget state via `ctx.tree.with_behavior_as::<T, _>(panel_id, |p| ...)` and reuses existing helpers (`selection_from_list_box`, `enter_subdir`, `trigger_file`). The non-row file-models update signal at C++ cpp:392 stays as-is; B-010 does not touch it.

---

## 5. Tests

New behavioural-coverage file: `crates/emcore/tests/rc_shim_b010.rs` (RUST_ONLY: dependency-forced — no C++ test analogue; the C++ test surface is X11 integration).

**emCoreConfigPanel rows (9 tests, one per row):**
- Harness creates a panel under a real-scheduler `PanelCtx` (existing harness pattern from B-005/B-009).
- For each row: run a Cycle to perform first-Cycle init; programmatically toggle the widget (e.g., `cb.SetChecked(!cb.IsChecked(), ctx)` on the child via `with_behavior_as::<CheckBoxPanel, _>`); advance Cycle; assert `Config->X.GetValue()` reflects the change; assert TrySave was attempted.
- Reset row (#1) regression: click Reset; advance Cycle; assert `generation.get()` increased by 1 AND every Config field reset to its default.

**emFileSelectionBox rows (6 tests, one per row):**
- Harness creates an FSB; runs `LayoutChildren` + first-Cycle init.
- For each row: drive the widget (e.g., set text on dir field via `with_behavior_as::<TextFieldPanel, _>` then call `text_field.set_text(...)`); advance Cycle; assert FSB internal state changed (parent_dir updated, hidden_files_shown toggled, selected_names changed, triggered_file_name set, etc.).
- Multi-event ordering test: set dir field text AND toggle hidden in the same input slice; advance Cycle; assert both reactions fire AND in C++ source order (dir before hidden — cpp:396 vs cpp:405).

**Regression test (D-009 follow-up):** before B-010, `cargo-nextest ntr` passes. After B-010, the same pre-existing emCoreConfigPanel and emFileSelectionBox tests still pass (no behavioural regression beyond the closure→Cycle move).

`cargo check --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo-nextest ntr` all pass. Golden tests untouched (no pixel arithmetic in scope).

---

## 6. Sequencing

**Single PR.** All 9 emCoreConfigPanel host-panel conversions, the FsbEvents drop, and the new `rc_shim_b010.rs` test file land together. Splitting risks half-converted Cycle bodies (some closures + some IsSignaled in one panel) which is harder to review than a clean per-host swap.

Internal commit ordering for incremental-compile sanity:
1. **C1** — Add `Cycle` impls + `subscribed_init` flags + signal/id-cache fields to all 5 emCoreConfigPanel host panels and to FSB. Closures still run; new Cycle bodies are (effectively) dead code (signal subscriptions present but reactions duplicate the closure work — harmless because the closure already mutated).
2. **C2..C6** — Per emCoreConfigPanel host panel (5 commits, one each): delete the closures for that host; the new Cycle becomes the sole publication path.
3. **C7** — In FSB, delete the 7 closure setters in `LayoutChildren`; the new Cycle becomes the sole publication path.
4. **C8** — Delete `FsbEvents` struct and `events` field. Compile-clean now.
5. **C9** — Add `crates/emcore/tests/rc_shim_b010.rs` covering all 15 rows.

Alternative: per-panel feature flag for selectively running closure or Cycle reactions, simplifying bisect. Implementer's choice; no requirement.

---

## 7. Audit-data corrections (for reconciliation)

1. **Bucket sketch's "no Cycle override exists" note** (B-010-rc-shim-emcore.md row table notes for emCoreConfigPanel rows 299/300/301): refers to *Rust today* (correct: zero `fn Cycle` impls in emCoreConfigPanel.rs). Implies no C++ Cycle either, which is **wrong**. C++ has Cycle overrides on `emCoreConfigPanel` (cpp:42-52, handles ResetButton), `MouseMiscGroup` (cpp:~245-275, handles 3 checkboxes), `MaxMemGroup` (handles MemField), and `PerformanceGroup` (cpp:663-715, handles 4 widgets). Each uses `IsSignaled(widget->Get*Signal())` per the canonical pattern. Reconciler should patch the bucket sketch row notes to disambiguate "no Cycle in Rust" vs "no Cycle in C++."
2. **Bucket sketch open question §2** ("emFileSelectionBox uses a `RefCell<Events>` aggregator drained by a single `Cycle`. Does converting each event to its own signal-subscribe preserve the C++ Cycle ordering…?"): **resolved.** C++ has no aggregator. `emFileSelectionBox::Cycle` (cpp:385) is a flat list of `IsSignaled` branches in source order (cpp:396 dir, cpp:405 hidden, cpp:413 files-sel, cpp:419 files-trigger, cpp:434 name, cpp:469 filter). Each branch reads widget state directly via the widget's accessor. The Rust port's per-Cycle drain order in the new design mirrors C++ exactly. No aggregator needed.
3. **Bucket sketch open question §3** ("for the `Rc<Cell<u32>>` generation counter that tracks any-config-mutated: does the C++ original have a single config-changed signal that all widgets fan into, or per-control signals?"): **resolved.** C++ has neither. C++ uses `emRecListener::OnRecChanged()` per group (each group inherits from `emRecListener` and listens to a sub-rec or the whole config root) → calls `UpdateOutput()` to re-sync widget values *without* rebuilding the panel tree. The Rust generation-counter + LayoutChildren-rebuild is a Rust-only architectural choice that diverges from C++. Surfaced as **D-009 sighting #4**; out of B-010 row scope; tracked for a future bucket.
4. **Bucket sketch open question §4** ("D-002 flags the emAutoplay `AutoplayFlags { progress: Rc<Cell<f64>> }` adaptation question…"): **not relevant.** No B-010 row shares the AutoplayFlags shape; that question was already answered for B-003 in the D-002 catalog (R-A: drop AutoplayFlags).
5. **No row reclassifications.** All 15 rows confirmed P-004, accessor_status `present` (widget-side, same audit interpretation as B-011), disposition rule-1 convert.

---

## 8. Cross-bucket prereq edges

- **No hard prereqs inbound.** No bucket touches B-010's host panels.
- **No hard prereqs outbound.** B-010 doesn't gate any other bucket's row work.
- **Soft citation propagation:** D-009 promotion back-references B-003 and B-012's prior-sighting resolutions. The reconciler should add a "now formalised as D-009" citation pointer to those two buckets' design docs (no design changes; just a citation-update line in their reconciliation log entries).

---

## 9. Verification

Post-implementation:
1. `cargo check --workspace` — must pass.
2. `cargo clippy --workspace -- -D warnings` — must pass.
3. `cargo-nextest ntr` — full suite, must pass. New `rc_shim_b010.rs` exercises every row.
4. `cargo xtask annotations` — must pass. No new `DIVERGED:` blocks added.
5. `rg -n 'FsbEvents|events: Rc<RefCell<' crates/emcore/src/emFileSelectionBox.rs` — expected: zero hits.
6. `rg -n 'on_check\|on_value\|on_click' crates/emcore/src/emCoreConfigPanel.rs` — expected: zero hits (all 9 closures gone).
7. Manual: read each new `Cycle` body end-to-end; confirm `IsSignaled` branches match C++ source order (cpp:42 reset; cpp:245-275 mouse-misc; cpp:663-715 performance; cpp:385-501 FSB).

---

## 10. Reconciliation summary (for working-memory session)

- **Bucket status:** B-010 → **designed**.
- **Decision citations finalised:** D-002 (rule 1 for all 15 rows), D-006 (per-host first-Cycle init), **D-009-proposed** (in-bucket promotion).
- **D-009 promotion:** add D-009 entry to `decisions.md` per §3 phrasing. Back-propagate citations: B-003 (`AutoplayFlags.progress` resolution → "now formalised as D-009"), B-012 (`mw.to_reload` resolution → same). Update D-007's watch-list note to reference the now-promoted D-009 decision; the watch-list paragraph in D-007 can shrink to a one-line "promoted to D-009; see decisions.md."
- **Audit-data corrections:** 5 items per §7. Reconciler patches the B-010 bucket sketch and `inventory-enriched.json` row notes accordingly.
- **Pattern reclassifications:** none.
- **Cross-bucket prereq edges:** none new (no row reassignments; no hard prereqs created).
- **Out-of-bucket file edits:** none required (no model or framework files touched).
- **Residual drift surfaced for follow-up:** the `generation: Rc<Cell<u64>>` counter on `emCoreConfigPanel` (sighting #4 of D-009; out of row scope; tangled with row 80's reaction body which still bumps it). Reconciler logs as a candidate for a future bucket once D-009 is formalised and the working-memory session decides whether to spin up a dedicated counter-removal bucket or add the `emRecListener`-equivalent infrastructure as a separate framework lift.
- **D-007 watch-list note in `decisions.md`:** shrink to a back-pointer to D-009 (D-007's specific scope is mutator-fire shape; D-009 generalises the polling-intermediary problem; the two compose).

---

## 11. Open items for implementer

1. The Reset row's `generation.set(generation.get() + 1)` line moves verbatim from the deleted `on_click` closure into the new `IsSignaled(self.bt_reset_sig)` branch in `ButtonsPanel::Cycle`. Do NOT remove or refactor that line — it is load-bearing for the visible-Reset behaviour until the generation counter itself is removed by a downstream bucket.
2. Each `with_behavior_as::<T, _>(panel_id, |p| ...)` returns `Option<R>`; choose a sensible `unwrap_or(default)` per row. For `IsChecked` → `false` is safe (matches widget pre-state). For `GetText` → empty string is safe. For `GetSelectedIndices` → empty vec. For `GetTriggeredItemIndex` → `None` (skip the branch body).
3. The `InvalidatePaintingOfAllWindows` helper invoked by row 9's reaction (Upscale) per C++ cpp:710 may not yet be ported as a Rust helper. If absent, port it as a panel-tree walk that calls each `emWindow`'s repaint trigger; its C++ implementation (cpp:828-843) uses a `DirtyTrickToAccessProtectedMethod` shim, suggesting a small standalone helper port is acceptable.
4. The `subscribed_init` guard pattern with `*_id.is_some()` (FSB only) is present because FSB's children are constructed lazily on first auto-expand; the 5 emCoreConfigPanel host panels create children inside `LayoutChildren` too (per the existing `if ctx.child_count() == 0 { self.create_children(ctx); }` pattern), so they need the same guard. Add `<host>_id.is_some()` checks to the connect calls in their first-Cycle init.
5. If any host panel's `create_children` runs more than once (e.g., generation-bump rebuild), the cached `*_id` and `*_sig` get re-overwritten on each rebuild. That's correct (the new widgets have new signal IDs); but the `subscribed_init` flag must be reset to `false` whenever children are torn down (i.e., inside the `gen != self.last_generation && ctx.child_count() > 0 { for id in ctx.children() { ctx.delete_child(id); } self.last_generation = gen; }` block currently in `LayoutChildren`). Add `self.subscribed_init = false;` there. Otherwise after a Reset bumps generation, the old subscriptions still target dropped-but-still-known SignalIds and the new widgets are unsubscribed.
