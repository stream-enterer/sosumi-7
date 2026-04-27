# B-003-no-wire-autoplay — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm) — *with one escalation gate (E-1) the working-memory session must resolve before implementation begins.*
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-003-no-wire-autoplay.md`
**Pattern:** P-001-no-subscribe-no-accessor
**Scope:** emmain:emAutoplay, 3 rows
**Mechanical-vs-judgement:** judgement-heavy — wiring shape is mechanical (D-006), but a structural Rust-only adaptation (`AutoplayFlags`, P-004 rc-shim) sits between the ViewModel and the ControlPanel. D-002 explicitly defers escalation of this exact pattern.

## Goal and scope

Wire the missing P-001 sites in `emmain:emAutoplay`: both halves of the wire (model-side accessor + consumer-side subscribe). The bucket's 3 rows touch only `emAutoplay` scope:

1. `emAutoplayViewModel-accessor-model-change` — port `GetChangeSignal()` to the Rust `emAutoplayViewModel`.
2. `emAutoplayViewModel-accessor-model-state` — port the second ViewModel signal accessor. **Audit anomaly:** the C++ second signal is `ProgressSignal` (`emAutoplay.h:241`, `emAutoplay.cpp:911`), not "model-state." See §"Audit-data anomalies."
3. `emAutoplay-1171` (Rust site `emAutoplayControlPanel.rs:658`) — `emAutoplayControlPanel::new` (C++ `emAutoplayControlPanel.cpp:1171`) calls `AddWakeUpSignal(Model->GetChangeSignal())` and `AddWakeUpSignal(Model->GetProgressSignal())`. Wire both subscribes plus the matching `IsSignaled` branches in `Cycle`.

All 3 rows are designed using **D-006-subscribe-shape** (first-Cycle init + IsSignaled top-of-Cycle). The two accessor-side rows are filled in scope per **D-003-gap-blocked-fill-vs-stub** because both halves live in the same scope (`emAutoplay.rs` / `emAutoplayControlPanel.rs`). The wire-up requires structural changes to the existing `AutoplayFlags` rc-shim — see **E-1 escalation** before implementation.

## Cited decisions

- **D-006-subscribe-shape** — canonical wiring pattern (`subscribed_init` flag + `ectx.connect` in first `Cycle` + `IsSignaled` at top of `Cycle`).
- **D-003-gap-blocked-fill-vs-stub** — fill the two ViewModel accessor gaps in this bucket; both halves live in `emAutoplay.rs`.
- **D-002-rc-shim-policy** — open question 1 ("emAutoplay flags-passing pattern — does this fall under rule 1 or rule 2? emAutoplay has no C++ analogue (it's a Rust-only panel) so the rule needs adaptation") is the gating decision for this bucket. **D-002 explicitly defers this resolution to the working-memory session before bucket execution.** See **E-1** below.

## Audit-data anomalies (corrections)

The following are stale or mis-tagged; the design records them so the working-memory session can patch `inventory-enriched.json`:

1. **`emAutoplayViewModel-accessor-model-state`** — tagged "model-state signal accessor absent on view-model." The C++ second signal is **`ProgressSignal`** (`emAutoplay.h:241`/`:279`/`:318`), not a state signal. C++ `emAutoplayControlPanel.cpp:1172` confirms: `AddWakeUpSignal(Model->GetProgressSignal())`. The audit row should be renamed `emAutoplayViewModel-accessor-progress`. Same fix shape — port the accessor — but the SignalId field name is `progress_signal`, not `state_signal`.

2. **`emAutoplay-1171` row's "Rust site `emAutoplay.rs:658`" line number** — the actual port lives in `emAutoplayControlPanel.rs` (split file) at `line 544` (`emAutoplayControlPanel::new`), not at `emAutoplay.rs:658`. The bucket file's own "Open questions" §3 already flagged this: "the actual port lives in `emAutoplayControlPanel.rs`." Confirmed — the bucket file's Rust site annotation should be patched to `emAutoplayControlPanel.rs:544`.

3. **The rows tag accessor `missing` on `emAutoplayViewModel`.** Confirmed — the Rust `emAutoplayViewModel` (`emAutoplay.rs:802–816`) has no `change_signal` field, no `progress_signal` field, no accessors. Both signals are genuinely missing on the Rust port (the `ItemProgress: f64` field exists at line 809 but is unsignalled).

4. **The `emAutoplayConfig::GetChangeSignal()` accessor at `emAutoplay.rs:129`** is a *different* object (config-side, present and correct via composed `emConfigModel`). The bucket file's Open Questions §4 confirms no cross-wiring to ViewModel is needed — this aligns with C++: the C++ `emAutoplayViewModel` holds `emRef<emAutoplayConfig> Config` as a member (`emAutoplay.h:265`) and *internally* subscribes to it; that subscribe is part of the ViewModel's own Cycle and is not exposed at the ControlPanel layer. The B-003 wire is ControlPanel→ViewModel only. Anyway, that internal Config→ViewModel subscribe is itself a separate (potentially missing) wire, but it does **not** belong in B-003. If the working-memory session wants to track it, file as a new row; *do not* fold into B-003.

5. **The `emAutoplayControlPanel` is not currently a Cycle-driven `PanelBehavior`.** It has no `Cycle` method (`emAutoplayControlPanel.rs:655–701`); `PanelBehavior` is implemented but `Cycle` is never overridden. C++ `emAutoplayControlPanel::Cycle()` (`emAutoplay.cpp:1183–1222`) fans out across BtAutoplay/BtPrev/BtNext/BtContinueLast/SfDuration/CbRecursive/CbLoop signals plus the two Model signals. Adding Cycle to the Rust panel is a precondition for D-006. *This is in scope for B-003* (it's the consumer-side wire-up of row `emAutoplay-1171`), but the implementer must understand it's ~7 widget signals' worth of new Cycle code, not 2.

These corrections do not move any rows out of B-003.

## E-1 — Escalation gate (AutoplayFlags pattern)

**Per CONSTRAINTS line 4 of the brainstorm prompt:** *"emAutoplay is a Rust-only port consideration: D-002 explicitly flagged the AutoplayFlags pattern as needing escalation. If your design touches that pattern, escalate (do not silently resolve)."*

**This design touches the AutoplayFlags pattern unavoidably.** Wiring `emAutoplayControlPanel` to `emAutoplayViewModel` via signals replaces the click-handler-shim flow that AutoplayFlags currently implements. The two are alternative architectures for the same concern (delivering "user clicked Autoplay" / "user changed duration" from the widget into the Model). The B-003 wires cannot land without picking one — and that pick is what D-002 deferred.

**Surface of the AutoplayFlags pattern today** (`emAutoplayControlPanel.rs:88–119`):

```rust
pub struct AutoplayFlags {
    pub toggle: Cell<Option<bool>>,
    pub prev: Cell<bool>,
    pub next: Cell<bool>,
    pub continue_last: Cell<bool>,
    pub duration_value: Cell<Option<f64>>,
    pub recursive: Cell<Option<bool>>,
    pub loop_toggle: Cell<Option<bool>>,
    pub progress: Rc<Cell<f64>>,           // *outbound* — Model→Panel
}
```

The first seven are **inbound** (widget→Model). The eighth (`progress`) is **outbound** (Model→Panel — and it's the only thing currently letting the AutoplayCheckButtonPanel paint a progress bar; today it's unwired because the Rust ViewModel's `ItemProgress` is never plumbed into `flags.progress`).

**Critical observation: the inbound flags are produced but never consumed.** `grep` for `autoplay_flags.toggle.take`, `.continue_last.take`, etc. across the entire workspace returns *only* unit-test sites in `emAutoplayControlPanel.rs`. The `emMainControlPanel::Cycle` (`emMainControlPanel.rs:287–342`) polls `click_flags` (window-level actions like Reload/Close/Quit) but never `autoplay_flags`. **The inbound half of AutoplayFlags is pure dead code today.** The DIVERGED annotation at `emAutoplayControlPanel.rs:84` claims "polled by the parent panel" — that is factually wrong; nothing polls them.

**Three viable resolutions (decision belongs to working-memory session):**

- **R-A. Replace AutoplayFlags entirely with the C++ signal model.** Drop `AutoplayFlags`. Have `emAutoplayControlPanel` hold an `Rc<RefCell<emAutoplayViewModel>>` (justified per CLAUDE.md §Ownership rule (a) — cross-Cycle reference held by the panel). Widgets subscribe to widget-internal `SignalId`s (`emCheckButton::check_signal`, `emButton::click_signal`, `emScalarField::value_signal`, `emCheckBox::check_signal`); `Cycle` does the C++ fan-out exactly (`emAutoplay.cpp:1184–1218`), invoking `Model.borrow_mut().SetAutoplaying(...)` etc. Mirrors C++ shape exactly; removes a no-forced-category `DIVERGED` annotation; deletes ~30 lines of currently-dead Rust-only code. **Recommended.**

- **R-B. Keep AutoplayFlags as a *consumer-side* shim, but wire it to real signals on both ends.** ViewModel exposes `change_signal` and `progress_signal`. ControlPanel `Cycle` connects to each widget signal (replacing `on_check`/`on_click` callbacks that mutate flags) and writes to ViewModel inside the IsSignaled branch. The outbound `progress: Rc<Cell<f64>>` stays as the Model→AutoplayCheckButtonPanel rendering shim because the painted-button needs `f64` per-paint. Less invasive; preserves the `progress` Rc shim as a paint-time channel; the inbound seven flags get deleted. **Hybrid; preserves only the load-bearing half.**

- **R-C. Keep AutoplayFlags whole and wire it as a P-004 rc-shim.** Promote AutoplayFlags to "real" by adding a poll-site in `emMainControlPanel::Cycle` that drains the seven inbound flags into the ViewModel. Pure drift-preservation; the C++ structure (subscribe in ControlPanel) is replaced wholesale. Per D-002 rule 1 ("Convert if the C++ original uses a signal accessor and a subscribe at the consumer site") this is the wrong call: C++ here *does* use signal accessors and subscribes at the ControlPanel. **Rejected on D-002 grounds; included for completeness.**

**Design proceeds assuming R-A** (the option most consistent with D-002's stated default ("convert"), Port Ideology, and the dead-code finding). If the working-memory session selects R-B, the per-component fixes below remain; only the inbound widget-callback bodies and the `AutoplayFlags` definition change. If the working-memory session selects R-C, **abandon** the design below — the bucket then becomes a no-op for the consumer side and only the two accessor adds remain in scope.

The working-memory session must ratify R-A (or pick R-B/R-C) before implementation begins, and update D-002 with the resolution.

## Accessor groups

### G1 — `emAutoplayViewModel.GetChangeSignal()`

**C++ source.** `emAutoplay.h:221` (declared), `emAutoplay.h:268` (member), `emAutoplay.h:293` (inline accessor). Fired at `emAutoplay.cpp:661, 675, 688, 701, 710, 1126` — every public state-mutation site (`SetDurationMS`, `SetRecursive`, `SetLoop`, `SetAutoplaying`, `ContinueLastAutoplay`, plus internal `SaveLocation`).

**Rust state today.** No SignalId field, no accessor. The `emAutoplayViewModel` struct is constructed via `new()` taking no args (`emAutoplay.rs:820`), so it has no `ConstructCtx` from which to allocate a SignalId.

**Fix.**

```rust
pub struct emAutoplayViewModel {
    // ... existing fields
    pub(crate) change_signal: SignalId,
    pub(crate) progress_signal: SignalId,
    // ...
}

impl emAutoplayViewModel {
    /// Port of C++ emAutoplayViewModel constructor (emAutoplay.cpp:632-657).
    /// Allocates SignalIds via ConstructCtx; replaces the no-arg new().
    pub fn new<C: emcore::emEngineCtx::ConstructCtx>(cc: &mut C) -> Self {
        Self {
            // ... existing field defaults
            change_signal: cc.create_signal(),
            progress_signal: cc.create_signal(),
        }
    }

    /// Port of C++ emAutoplayViewModel::GetChangeSignal (emAutoplay.h:293).
    pub fn GetChangeSignal(&self) -> SignalId {
        self.change_signal
    }

    /// Port of C++ emAutoplayViewModel::GetProgressSignal (emAutoplay.h:318).
    pub fn GetProgressSignal(&self) -> SignalId {
        self.progress_signal
    }

    /// Helper invoked at every change-emit site. Mirrors C++ Signal(ChangeSignal).
    fn signal_change(&self, ectx: &mut emcore::emEngineCtx::EngineCtx<'_>) {
        ectx.signal(self.change_signal);
    }

    fn signal_progress(&self, ectx: &mut emcore::emEngineCtx::EngineCtx<'_>) {
        ectx.signal(self.progress_signal);
    }
}
```

**Mutator-site update.** The existing setters (`SetDurationMS` line 842, `SetRecursive` line 850, `SetLoop` line 858, `SetAutoplaying` line 866, `ContinueLastAutoplay` line 920, `SaveLocation` line 987) currently have no `EngineCtx` parameter. They must take `&mut EngineCtx` and call `self.signal_change(ectx)` after every state change (mirror C++ call sites listed above). Same for `SetItemProgress` (line 888) and `signal_progress`. **This changes the public signature of every public mutator on `emAutoplayViewModel`** — caller sites in `emMainWindow.rs:294, 1212` need updates. Mechanical.

**Caller update at `emMainWindow.rs:1212`.** `crate::emAutoplay::emAutoplayViewModel::new()` — replace with `emAutoplayViewModel::new(&mut cc)` where `cc` is a `ConstructCtx` available at MainWindow construction. `emMainWindow::new` already runs in a context where `ConstructCtx` is reachable (verify against `emMainWindow.rs` construction path before implementation; if not reachable, defer initialization to the first `Cycle` of MainWindow's engine and use `subscribed_init`-style two-phase init).

**Rows depending on G1 (consumer subscribes):**
- `emAutoplay-1171` half-A (ControlPanel.Cycle subscribes ChangeSignal) — see below.

### G2 — `emAutoplayViewModel.GetProgressSignal()`

Identical fix shape to G1. Fired at `emAutoplay.cpp:911` only (`SetItemProgress`). The Rust `SetItemProgress` (line 888) currently mutates `ItemProgress` without notifying; per Audit anomaly §1, this is the second missing accessor.

**Rows depending on G2:**
- `emAutoplay-1171` half-B (ControlPanel.Cycle subscribes ProgressSignal).
- The currently-orphaned `AutoplayFlags.progress: Rc<Cell<f64>>` channel that drives the AutoplayCheckButtonPanel paint code (`emAutoplayControlPanel.rs:186`) needs a producer. Under R-A: the ControlPanel's IsSignaled(ProgressSignal) branch reads `model.GetItemProgress()` and writes to `flags.progress.set(p)` (or, if R-A removes flags entirely, writes directly to the AutoplayCheckButtonPanel via panel-tree access). Under R-B: same plus keep flags struct.

## Per-panel consumer wiring

### emAutoplayControlPanel (1 row, 7 widget subscribes plus 2 Model subscribes)

C++ `emAutoplayControlPanel::Cycle` (`emAutoplay.cpp:1183–1222`) is the spec. Add `subscribed_init: bool` field, hold `Rc<RefCell<emAutoplayViewModel>>` (per R-A; cross-Cycle reference is justified per CLAUDE.md §Ownership rule (a)), and implement `Cycle` per D-006:

```rust
pub struct emAutoplayControlPanel {
    layout: emPackLayout,
    border: emBorder,
    look: Rc<emLook>,
    children_created: bool,
    /// Cross-Cycle reference to ViewModel; per CLAUDE.md §Ownership (a).
    model: Rc<RefCell<emAutoplayViewModel>>,
    /// First-Cycle init flag for D-006-subscribe-shape.
    subscribed_init: bool,
    /// First-Cycle-after-AutoExpand init flag for widget signals (mirrors B-001 §Sequencing).
    subscribed_widgets: bool,
    /// Widget instance SignalId snapshots, captured at child creation for IsSignaled checks.
    /// Populated in `create_children` when widgets are first instantiated.
    widget_signals: Option<WidgetSignalIds>,
}

struct WidgetSignalIds {
    bt_autoplay_check: SignalId,    // emCheckButton::check_signal
    bt_prev_click: SignalId,
    bt_next_click: SignalId,
    bt_continue_last_click: SignalId,
    sf_duration_value: SignalId,
    cb_recursive_check: SignalId,
    cb_loop_check: SignalId,
}

impl PanelBehavior for emAutoplayControlPanel {
    // ... existing Paint, LayoutChildren, etc.

    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, _ctx: &mut PanelCtx) -> bool {
        let eid = ectx.id();

        if !self.subscribed_init {
            ectx.connect(self.model.borrow().GetChangeSignal(), eid);
            ectx.connect(self.model.borrow().GetProgressSignal(), eid);
            self.subscribed_init = true;
        }
        if !self.subscribed_widgets && let Some(sigs) = &self.widget_signals {
            ectx.connect(sigs.bt_autoplay_check, eid);
            ectx.connect(sigs.bt_prev_click, eid);
            ectx.connect(sigs.bt_next_click, eid);
            ectx.connect(sigs.bt_continue_last_click, eid);
            ectx.connect(sigs.sf_duration_value, eid);
            ectx.connect(sigs.cb_recursive_check, eid);
            ectx.connect(sigs.cb_loop_check, eid);
            self.subscribed_widgets = true;
        }

        // C++ source order — emAutoplay.cpp:1184–1218.
        if let Some(sigs) = &self.widget_signals {
            if ectx.IsSignaled(sigs.bt_autoplay_check) {
                let checked = /* read BtAutoplay.IsChecked() via panel-tree lookup */;
                self.model.borrow_mut().SetAutoplaying(ectx, checked);
            }
            if ectx.IsSignaled(sigs.bt_prev_click) {
                self.model.borrow_mut().SkipToPreviousItem(ectx);
            }
            if ectx.IsSignaled(sigs.bt_next_click) {
                self.model.borrow_mut().SkipToNextItem(ectx);
            }
            if ectx.IsSignaled(sigs.bt_continue_last_click) {
                self.model.borrow_mut().ContinueLastAutoplay(ectx);
            }
            if ectx.IsSignaled(sigs.sf_duration_value) {
                let val = /* read SfDuration.GetValue() */;
                let ms = DurationValueToMS(val);
                self.model.borrow_mut().SetDurationMS(ectx, ms);
            }
            if ectx.IsSignaled(sigs.cb_recursive_check) {
                let checked = /* read CbRecursive.IsChecked() */;
                self.model.borrow_mut().SetRecursive(ectx, checked);
            }
            if ectx.IsSignaled(sigs.cb_loop_check) {
                let checked = /* read CbLoop.IsChecked() */;
                self.model.borrow_mut().SetLoop(ectx, checked);
            }
        }

        if ectx.IsSignaled(self.model.borrow().GetChangeSignal()) {
            self.update_controls(/* ... */);  // mirror C++ UpdateControls (cpp:1394-…)
        }
        if ectx.IsSignaled(self.model.borrow().GetProgressSignal()) {
            self.update_progress(/* ... */);  // mirror C++ UpdateProgress (cpp:1467-…)
        }

        false
    }
}
```

**Reading widget state from inside Cycle.** The C++ pattern `BtAutoplay->IsChecked()` requires Rust-side access to the widget instance. Two viable paths:
- (a) Hold direct `Option<emCheckButton>`-style refs on the panel struct (mirrors C++ which holds `emCheckButton * BtAutoplay`).
- (b) Look up widget child panels by `PanelId` and extract state via the panel-tree's typed-behavior accessor.

Path (a) is the C++ shape and the smallest viable structure; B-005's typed-subscribe wiring uses path (a). Pick (a) — promote the locals inside `create_children` to fields.

**`update_controls` / `update_progress`** mirror C++ `UpdateControls`/`UpdateProgress` (`emAutoplay.cpp` ~ lines 1394 and 1467 — implementer locates exactly). These read Model state and push back into the widgets (e.g., `BtAutoplay->SetChecked(Model->IsAutoplaying())`). Out-of-scope for the wire-up itself but must exist for the `IsSignaled(ChangeSignal)` branch to be useful. If full UpdateControls porting is large, the implementer may stage it as a follow-up — the wire is what B-003 owns; the reaction body just needs *any* observable side effect (logging is acceptable for a stop-gap). Flag for working-memory if so staged.

**Replacing `on_check` / `on_click` callbacks.** The current widget closures (`emAutoplayControlPanel.rs:580–584`, `:625–629`, etc.) write into `AutoplayFlags`. Under R-A these closures are **removed**; widget signals are picked up through `IsSignaled` in `Cycle`. Under R-B these closures stay (continue writing into flags) — but the design loses fidelity. R-A is recommended.

### `AutoplayFlags::progress` outbound channel

If R-A: delete `AutoplayFlags` entirely. The `AutoplayCheckButtonPanel` (`emAutoplayControlPanel.rs:165–192`) needs a different `progress` source. Two options:
- (a-i) Hold `Rc<RefCell<emAutoplayViewModel>>` on `AutoplayCheckButtonPanel`; read `model.borrow().GetItemProgress()` in `Paint`. Minimal code; same Rc pattern as the parent.
- (a-ii) Have the parent's `update_progress` push the value into a small `Rc<Cell<f64>>` owned solely between parent and the AutoplayCheckButtonPanel. (Equivalent to R-B's surviving channel.)

Pick (a-i) — same Rc<RefCell<ViewModel>> shape applies; smallest delta from the rest of R-A.

If R-B: leave `AutoplayFlags::progress` in place and add the `update_progress` parent-side write to `flags.progress.set(model.GetItemProgress())`.

## Sequencing

**Within the bucket:**

1. **Land G1+G2 accessor adds** on `emAutoplayViewModel`. Mechanical: add SignalId fields, accessors, signal-emit calls in mutators. Update `emAutoplayViewModel::new` signature to take `&mut C: ConstructCtx`. Update caller at `emMainWindow.rs:1212`. Tests: subscribe to each signal, call mutator, assert wake.
2. **Land widget-instance promotion to fields** on `emAutoplayControlPanel` (`BtAutoplay`, `BtPrev`, `BtNext`, `BtContinueLast`, `SfDuration`, `CbRecursive`, `CbLoop`). No Cycle changes yet.
3. **Land AutoplayFlags removal (R-A) or shrink (R-B)** plus `Rc<RefCell<emAutoplayViewModel>>` plumbing through `emMainControlPanel`.
4. **Land `Cycle` implementation** with D-006 init blocks + IsSignaled branches.
5. **Stage 5 may be split** if `UpdateControls`/`UpdateProgress` body porting is large; the wire and a stub reaction can land first.

**Cross-bucket prereqs.** **None.** B-003 does not consume any P-003 type-mismatch accessor; G1/G2 are both `SignalId`-typed by construction.

**Cross-bucket impact (potentially).** R-A removes the `AutoplayFlags` struct, which is referenced by `emMainControlPanel` (`emMainControlPanel.rs:141, 184, 218, 373, …`) for plumbing only. `emMainControlPanel` does not consume the flags. The R-A migration is a `s/AutoplayFlags/Rc<RefCell<emAutoplayViewModel>>/g` plumbing change in that file. Mechanical, in-bucket. **No row leaks into another bucket.**

## Verification strategy

**Behavioral test** (per D-006 / B-005 precedent):

`crates/emmain/tests/typed_subscribe_b003.rs` — fires each subscribed signal, runs Cycle, asserts the documented reaction.

Pattern:
```rust
let mut h = Harness::new();
let mw = h.create_main_window();
// ChangeSignal: mutating ViewModel via setter wakes the panel.
mw.autoplay_view_model.SetDurationMS(&mut h.ectx(), 10000);
h.run_cycle();
assert!(/* control panel update_controls ran — observable via duration display refresh */);

// Click signal: invoking widget click drives ViewModel.
let panel = h.find_autoplay_control_panel();
h.click_check_button(panel.bt_autoplay_id);
h.run_cycle();
assert!(mw.autoplay_view_model.IsAutoplaying());

// ProgressSignal: mutating progress wakes the panel; AutoplayCheckButtonPanel paints with new progress.
mw.autoplay_view_model.SetItemProgress(&mut h.ectx(), 0.5);
h.run_cycle();
let painted = h.last_paint(panel.bt_autoplay_id);
assert!(painted.contains_progress_bar(0.5));
```

For accessor rows (G1/G2): the same harness firing pattern doubles as the unit-level "accessor returns same SignalId across calls" sanity check — if the ID weren't stable, the subscribe→fire→wake cycle would break.

**No new pixel-level golden tests.** The drift surface is signal flow, not paint. Existing emmain goldens (if any cover the autoplay control panel) remain the regression backstop for paint output. The progress-bar paint is currently driven by an unwired channel; R-A's restoration of that channel may *change* observable paint where previously the bar was always 0 — flag for working-memory if a golden trips.

**Annotation checks.** R-A removes one DIVERGED annotation block (`emAutoplayControlPanel.rs:84` — "C++ uses AddWakeUpSignal/IsSignaled. Rust uses Rc<Cell<...>> flags…"). That annotation has no forced-category tag (would-be classified as fidelity-bug under current annotation lint). R-A's removal is itself a clean-up and should be confirmed in the implementation PR. Run `cargo xtask annotations` after.

## Open items deferred to working-memory session

1. **E-1 — AutoplayFlags resolution.** **BLOCKING.** Pick R-A / R-B / R-C and update **D-002** with the resolution (D-002's deferred open question §1 is exactly this). Recommended: R-A. Without a decision, the bucket cannot proceed past accessor adds (steps 1–2 of Sequencing).
2. **Audit-data corrections** to land in `inventory-enriched.json`:
   - Rename `emAutoplayViewModel-accessor-model-state` → `emAutoplayViewModel-accessor-progress`; correct the field name expectation.
   - Patch `emAutoplay-1171` row's Rust-site line number from `emAutoplay.rs:658` to `emAutoplayControlPanel.rs:544`.
   - Note that the row's "wire" expands to 7 widget subscribes plus 2 Model subscribes (the audit row count of 1 is correct as a single C++ Cycle fan-out).
3. **`UpdateControls`/`UpdateProgress` port scope.** If the implementer stages these as follow-ups (stub reactions in B-003, full port later), file a follow-up row. Not a B-003 prereq.
4. **AutoplayCheckButtonPanel paint progress source.** Confirm pick of (a-i) over (a-ii) under R-A. Pure code-style choice; no global decision needed.
5. **emMainWindow ViewModel construction site.** If `ConstructCtx` is not reachable at `emMainWindow.rs:1212`, the design needs a two-phase init for ViewModel SignalId allocation. Verify on first PR; surface as an amendment if blocked.
6. **Internal Config→ViewModel subscribe** (audit anomaly §4) is a separate missing wire that the audit may not have captured. Out of B-003 scope; suggest the working-memory session re-scan `emAutoplayConfig::GetChangeSignal()` consumers after B-003 lands.

## Success criteria

- `emAutoplayViewModel` exposes `GetChangeSignal()` and `GetProgressSignal()` accessors returning stable SignalIds; every C++ `Signal(ChangeSignal)` / `Signal(ProgressSignal)` site (C++ cpp:661/675/688/701/710/911/1126) has a corresponding Rust signal-emit call in the matching mutator.
- `emAutoplayControlPanel` implements `Cycle` per D-006, with first-Cycle subscribe to both Model signals and (after AutoExpand) all 7 widget signals; `IsSignaled` branches mirror C++ `emAutoplayControlPanel::Cycle` source order exactly.
- AutoplayFlags is resolved per E-1 (R-A: removed; R-B: shrunk to outbound progress channel only; R-C: rejected).
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- New `crates/emmain/tests/typed_subscribe_b003.rs` covers all 3 rows (2 accessor + 1 wire fan-out).
- B-003 status in `work-order.md` flips `pending → designed`.
- D-002 updated with the AutoplayFlags resolution (closes its open question §1).
