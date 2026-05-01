# B-012-rc-shim-mainctrl — Design

**Bucket:** B-012-rc-shim-mainctrl
**Pattern:** P-004-rc-shim-instead-of-signal
**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Source bucket file:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-012-rc-shim-mainctrl.md`
**Cited decisions:**
- D-002-rc-shim-policy (per-row triage; all 7 rows resolve to rule 1, convert).
- D-006-subscribe-shape (first-Cycle init block + `IsSignaled` checks at top of Cycle; merges with B-006's existing block).
- D-007-mutator-fire-shape (ectx threaded through `emMainWindow::ReloadFiles` to fire `file_update_signal` synchronously, replacing the `mw.to_reload` two-hop relay).
**Prereq buckets:**
- **B-019-stale-annotations** — lands first to remove camouflage `DIVERGED:` blocks at `emMainControlPanel.rs:35`, `:303`, `:320`. B-012 must not preserve any framing those annotations carried.
- **B-006-typed-subscribe-mainctrl** (soft prereq) — adds first-Cycle init block at `emMainControlPanel::Cycle` for 3 P-002 subscribes (rows 217/218/219). B-012's 7 click-signal subscribes drop into the same block. Merge order is non-blocking but the implementer of whichever bucket lands second merges its `ectx.connect(...)` calls into the existing block rather than creating a parallel one.

---

## Goal

Convert all 7 P-004 widget-click rc-shim consumers in `crates/emmain/src/emMainControlPanel.rs` (cpp:220-226 ↔ rs:296-334) to the canonical C++ shape: `AddWakeUpSignal(BtX->GetClickSignal())` mirrored as a first-Cycle `ectx.connect(...)` plus an `IsSignaled` reaction in `Cycle()`. Eliminate the intermediate `Rc<ClickFlags>` shim and its `on_click`-closure setters. For the reload row specifically, also unwire the second-hop `mw.to_reload → MainWindowEngine` polling relay by restructuring `emMainWindow::ReloadFiles` to take `&mut EngineCtx<'_>` and fire `file_update_signal` synchronously, mirroring C++ `MainWin.ReloadFiles()` at cpp:281.

## Scope

In scope:
- The 7 P-004 rows: `emMainControlPanel-220..226`. Reaction sites: rs:463, 469, 477, 481, 485, 492, 497 (the `flags.X.take()` branches in `emMainControlPanel::Cycle`).
- Removal of the `ClickFlags` struct and its `Rc` plumbing through `emMainControlPanel`, `LMainPanel` (rs:531), `GeneralPanel` (rs:631), `AboutCfgPanel` (rs:728), `CommandsPanel` (rs:881). The phantom `LCloseQuitPanel`/`LAbtCfgCmdPanel` referenced in earlier drafts do not exist — Rust flattens close/quit/reload/new-window into `CommandsPanel::create_children` (rs:919-1075). The auto-hide check buttons live as detached fields on `emMainControlPanel` itself (rs:179-183, allocated in `create_children` rs:302-320).
- Restructuring `emMainWindow::ReloadFiles` (rs:131) to `(&self, ectx: &mut EngineCtx<'_>)`; inlining the F5 hotkey caller at rs:272.
- Deleting `mw.to_reload` field (rs:78, init at rs:106) and the `MainWindowEngine` polling block (rs:389-397).
- Caching button `SignalId`s on `emMainControlPanel` (panel-level fields) so `Cycle` can subscribe and react without owning the buttons themselves.

Out of scope:
- The 3 P-002 rows (217/218/219) — owned by B-006.
- Any other panel-tree restructuring.
- New global decisions (none surface; see §"Coverage gaps").
- Annotation removal at rs:35/303/320 — B-019 owns.

## Non-goals

- Promoting buttons (`BtNewWindow`, `BtReload`, `BtClose`, `BtQuit`) to direct `emMainControlPanel` fields. C++ has them as members; Rust currently nests them inside `CommandsPanel` (rs:881-1078). `BtFullscreen` and the auto-hide check buttons are *already* fields on `emMainControlPanel` (rs:171, 179-183), so for those rows the signal handoff is trivial. Promoting the `CommandsPanel`-owned buttons is a larger structural change with no observable-behavior payoff. Caching the buttons' `click_signal: SignalId` (which is `Copy`; see `emSignal.rs:7` `pub struct SignalId(...)`) at the top-level panel is sufficient — the panel needs the signal id, not the button.
- Refactoring `MainWindowEngine` beyond deleting the to_reload polling block.

## Per-row Triage Table

All 7 rows are uniform — same C++ shape, same Rust shape, same disposition.

| # | Row ID | C++ site | C++ pattern | Rust today | Accessor verified | D-002 disposition |
|---|---|---|---|---|---|---|
| 1 | `emMainControlPanel-220` | cpp:220 (`AddWakeUpSignal(BtNewWindow->GetClickSignal())`) + cpp:262 (`IsSignaled` → `MainWin.Duplicate()`) | signal-subscribe | rs:463 (`flags.new_window.take()` after `on_click` sets cell). Button is `emButton`, owned by `CommandsPanel` (rs:919+) | `emButton.click_signal` at `emButton.rs:40`; `SignalId: Copy` (`emSignal.rs:7`) | rule 1 — convert (subscribe to `click_signal`) |
| 2 | `emMainControlPanel-221` | cpp:221 + cpp:266 (`IsSignaled` → `MainWin.ToggleFullscreen()`) | signal-subscribe | rs:469 (`flags.fullscreen.take()`). Button is `emCheckButton`, field of `emMainControlPanel` (rs:171, allocated rs:284-294) | `emCheckButton` inherits `emButton::click_signal` (verify accessor at `emCheckButton.rs`); **must subscribe to inherited click signal, not `check_signal`** — see §"Check-button rows" | rule 1 — convert |
| 3 | `emMainControlPanel-222` | cpp:222 + cpp:270 (`IsSignaled` → `MainConfig->AutoHideControlView.Invert(); Save()`) | signal-subscribe | rs:477 (`flags.auto_hide_control_view.take()`). Button is `emCheckButton`, detached field of `emMainControlPanel` (rs:181, allocated rs:303-313). **No `on_check`/`on_click` is wired today** — the `Cell` is write-zero/read-zero; conversion to `IsSignaled` is correct because `emCheckButton::Input` already fires the signal directly | `emCheckButton` click signal — see §"Check-button rows" | rule 1 — convert |
| 4 | `emMainControlPanel-223` | cpp:223 + cpp:275 (`IsSignaled` → `MainConfig->AutoHideSlider.Invert(); Save()`) | signal-subscribe | rs:481 (`flags.auto_hide_slider.take()`). Button is `emCheckButton`, detached field (rs:183, allocated rs:316-324). **No `on_check`/`on_click` wired today** — same shape as row 222 | `emCheckButton` click signal — see §"Check-button rows" | rule 1 — convert |
| 5 | `emMainControlPanel-224` | cpp:224 + cpp:280 (`IsSignaled` → `MainWin.ReloadFiles()`) | signal-subscribe | rs:485 (`flags.reload.take()` → `mw.to_reload = true`; second hop in `MainWindowEngine::Cycle` rs:389-397). Button is `emButton`, owned by `CommandsPanel` | `emButton.click_signal` | rule 1 — convert; **plus** unwire the two-hop relay (see §"Reload row — two-hop relay") |
| 6 | `emMainControlPanel-225` | cpp:225 + cpp:284 (`IsSignaled` → `MainWin.Close()`) | signal-subscribe | rs:492 (`flags.close.take()`). Button is `emButton`, owned by `CommandsPanel` | `emButton.click_signal` | rule 1 — convert |
| 7 | `emMainControlPanel-226` | cpp:226 + cpp:288 (`IsSignaled` → `MainWin.Quit()`) | signal-subscribe | rs:497 (`flags.quit.take()`). Button is `emButton`, owned by `CommandsPanel` | `emButton.click_signal` | rule 1 — convert |

**Rule-2 candidates:** none. No row has a post-finish/post-cycle member-field assignment shape; every row is a click signal at construction-time subscribe site. (Per the B-013 lesson, this was verified by reading C++ cpp:217-226 and cpp:262-290 directly — every site is `AddWakeUpSignal(...->GetClickSignal())` paired with `IsSignaled(...)` in `Cycle`.)

**Forced-category candidates:** none. The shim is project-internal Rust convenience, not language-/dependency-/upstream-gap-/performance-forced. Per Port Ideology, unannotated drift is fidelity-bug.

## Architecture

### Subscribe shape (D-006)

Merge into B-006's first-Cycle init block at `emMainControlPanel::Cycle`. After B-006 lands, that block contains 3 `ectx.connect(...)` calls for the P-002 signals. B-012 adds 7 more, one per button.

```rust
fn Cycle(
    &mut self,
    ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
    _ctx: &mut PanelCtx,
) -> bool {
    if !self.subscribed_init {
        let eid = ectx.id();
        // B-006 subscribes (rows 217/218/219) — already in place.
        crate::emMainWindow::with_main_window(|mw| {
            ectx.connect(mw.GetWindowFlagsSignal(), eid);
        });
        ectx.connect(self.config.borrow().GetChangeSignal(), eid);
        // B-012 subscribes (rows 220-226) — new.
        ectx.connect(self.bt_new_window_sig, eid);
        ectx.connect(self.bt_fullscreen_sig, eid);
        ectx.connect(self.bt_auto_hide_control_view_sig, eid);
        ectx.connect(self.bt_auto_hide_slider_sig, eid);
        ectx.connect(self.bt_reload_sig, eid);
        ectx.connect(self.bt_close_sig, eid);
        ectx.connect(self.bt_quit_sig, eid);
        self.subscribed_init = true;
    }

    // B-006 reactions (rows 217/218/219) — already in place.
    // ...

    // B-012 reactions (rows 220-226) — new.
    if ectx.IsSignaled(self.bt_new_window_sig) {
        crate::emMainWindow::with_main_window(|mw| { /* MainWin.Duplicate() — TODO when ported */ });
    }
    if ectx.IsSignaled(self.bt_fullscreen_sig) {
        crate::emMainWindow::with_main_window(|mw| { /* mw.ToggleFullscreen(app) — needs App; see §"App-bound reactions" */ });
    }
    if ectx.IsSignaled(self.bt_auto_hide_control_view_sig) {
        // MainConfig->AutoHideControlView.Invert(); MainConfig->Save();
        let mut cfg = self.config.borrow_mut();
        cfg.AutoHideControlView = !cfg.AutoHideControlView;
        cfg.Save();
    }
    if ectx.IsSignaled(self.bt_auto_hide_slider_sig) {
        let mut cfg = self.config.borrow_mut();
        cfg.AutoHideSlider = !cfg.AutoHideSlider;
        cfg.Save();
    }
    if ectx.IsSignaled(self.bt_reload_sig) {
        crate::emMainWindow::with_main_window(|mw| mw.ReloadFiles(ectx));
    }
    if ectx.IsSignaled(self.bt_close_sig) {
        crate::emMainWindow::with_main_window(|mw| mw.Close());
    }
    if ectx.IsSignaled(self.bt_quit_sig) {
        // mw.Quit(app) — needs App; see §"App-bound reactions"
    }

    false
}
```

The `bt_*_sig: SignalId` fields are populated as follows:

- **Rows 221, 222, 223** (`bt_fullscreen`, `bt_auto_hide_control_view`, `bt_auto_hide_slider`): the buttons are *already* fields on `emMainControlPanel` (rs:171, 181, 183), allocated in `emMainControlPanel::create_children` (rs:284-324). The signal id is read directly from the owned `Rc<RefCell<emCheckButton>>` at first Cycle — no handoff needed.
- **Rows 220, 224, 225, 226** (`bt_new_window`, `bt_reload`, `bt_close`, `bt_quit`): the buttons are owned by `CommandsPanel` (rs:919-1075). These need a handoff: capture `btn.click_signal` at button-construction time inside `CommandsPanel::create_children` and write it into a one-shot slot the top-level panel reads at first Cycle.

**Recommended handoff shape:** replace `Rc<ClickFlags>` with `Rc<Cell<ButtonSignals>>`. Concretely:

```rust
#[derive(Clone, Copy, Default)]
struct ButtonSignals {
    new_window: SignalId,
    reload: SignalId,
    close: SignalId,
    quit: SignalId,
}
```

`SignalId: Copy` is verified at `emSignal.rs:7` (`pub struct SignalId(...)` with derive), so `Cell<ButtonSignals>` compiles. `CommandsPanel::create_children` writes a fully-populated `ButtonSignals` into the cell at the end of construction. `emMainControlPanel` reads the cell once at first Cycle (after `create_children` has run via `LayoutChildren`) and copies the values into its own four `bt_*_sig` fields. `ButtonSignals` itself is *not* a running shim — the cell holds a one-shot init handoff, not state polled across ticks. (If the borrow checker is amenable, a one-shot `Option<ButtonSignals>` field set during child creation works equivalently and is more direct.)

This is the exact shape D-006 already endorses: panel-level signal field, populated by construction, subscribed at first Cycle. The `Rc` plumbing footprint is the same as today's `ClickFlags`; only the payload type changes.

### Check-button rows (221, 222, 223) — signal-kind discipline

Three of the seven rows use `emCheckButton`, not `emButton`:

- Row 221 `bt_fullscreen` (rs:171, allocated rs:284-294)
- Row 222 `bt_auto_hide_control_view` (rs:181, allocated rs:303-313)
- Row 223 `bt_auto_hide_slider` (rs:183, allocated rs:316-324)

`emCheckButton` exposes its own `check_signal` field at `emCheckButton.rs:31`. C++ cpp:221-223 subscribes to `BtX->GetClickSignal()` — the *click* signal inherited from `emButton`, which fires only on user click. The `check_signal` (or its C++ equivalent `emCheckBox::CheckSignal`) fires on any state transition, including programmatic `SetChecked` calls. Row 218 (B-006) reacts to config changes by calling `SetChecked` on these same buttons; subscribing row 221/222/223 to `check_signal` would create a feedback loop (config-change → SetChecked → check_signal → reaction → config-change).

**Required:** subscribe to the inherited *click* signal on all three rows. Implementer must verify `emCheckButton` exposes the inherited `emButton::click_signal` (either as a `pub` field via struct embedding, or via an accessor). If the accessor is missing, adding it is a prerequisite of B-012 (one-line `pub fn` returning `self.button.click_signal`) and not itself a divergence — the C++ `emButton::GetClickSignal()` is already public.

**On rows 222/223 specifically (no current click-publisher):** `create_children` (rs:303-324) constructs `BtAutoHideControlView` and `BtAutoHideSlider` with no `on_click` / `on_check` callback at all — there is nothing wired to `flags.auto_hide_*.take()`, so the `Cell`s are write-zero/read-zero and the existing reaction bodies (rs:477, 481) never fire. Conversion to `IsSignaled(click_signal)` is correct *because* `emCheckButton::Input` already fires `click_signal` directly when the user clicks; the design does not need to add an `on_click` plumbing step. State this so the implementer doesn't go hunting for missing callback wiring.

### App-bound reactions

Three reactions need `&mut App` (not just `&mut EngineCtx`):
- `MainWin.Duplicate()` — not yet ported (current code logs and returns).
- `MainWin.ToggleFullscreen(app)` — `App` access for `app.windows.get_mut(...)`.
- `MainWin.Quit(app)` — `App` access for shutdown sequence.

Today these are all stubbed-out behind `with_main_window` closures or bare logs that don't actually call the App-bound methods. Verified sites: row 220 (`new_window`, rs:463-466 logs only — `Duplicate` not yet ported); row 221 (`fullscreen`, rs:469-475 logs only inside a `with_main_window` no-op closure); row 226 (`quit`, rs:497-501 logs only). The current code has the same gap: it can't reach App from inside `Cycle`. Converting to signal-subscribe doesn't make this worse and doesn't make it better — the App-access gap is independent of the shim removal.

**Action:** preserve current reaction bodies for the App-bound rows. Replace `flags.X.take()` with `ectx.IsSignaled(self.bt_X_sig)`; leave the inner closure body unchanged (logs + the stubbed work). When App access becomes reachable from Cycle (separate work), the closure bodies fill in. **No `DIVERGED:` annotation should be added** — the gap is not new, and there's no forced-category claim to make. If a future linter pass needs to flag the stub, that's a pre-existing concern outside B-012's row set.

The two App-bound stub sites (fullscreen, quit) **remain drifted** after B-012 in the sense that the *reaction body* is still a no-op log. B-012 fixes the *subscription* drift (which is what P-004 measures); the App-access gap is a separate axis. Reconciliation log entry should note this so a follow-up bucket or audit pass can re-check the reaction-body drift.

### Reload row — two-hop relay

The `flags.reload.take() → mw.to_reload = true → MainWindowEngine::Cycle polls (rs:389-397) → fires file_update_signal` chain compresses to a synchronous `mw.ReloadFiles(ectx)` call from inside `emMainControlPanel::Cycle`, mirroring C++ cpp:281 exactly.

**Changes to `emMainWindow.rs`:**

1. Restructure `ReloadFiles` (rs:131):
   ```rust
   // Before:
   pub fn ReloadFiles(&self, app: &mut App) {
       app.scheduler.fire(app.file_update_signal);
   }
   // After:
   pub fn ReloadFiles(&self, ectx: &mut emcore::emEngineCtx::EngineCtx<'_>) {
       ectx.fire(self.file_update_signal);
   }
   ```
   The `mw` struct gains `pub(crate) file_update_signal: SignalId` cached at construction (the value is already captured by `MainWindowEngine` at rs:351 and assigned at rs:1117 from `app.file_update_signal`; `mw` caches it from the same source).

   **D-008 fallback (per Open Question 3 / adversarial review I-3):** `mw::new` (rs:96) currently runs from a context that does *not* receive `app.file_update_signal` — only `MainWindowEngine::new` does, at the construction site rs:1117. If the construction-site enumeration confirms the signal cannot be threaded into `mw::new` without substantial refactoring, fall back to D-008 lazy allocation: declare `pub(crate) file_update_signal: Cell<Option<SignalId>>` initialized `Cell::new(None)` and ensure-on-fire inside `ReloadFiles(ectx)` — `ectx` provides scheduler access for allocation, and the value is then memoized for subsequent calls. This preserves the synchronous-fire semantics of D-007. Choose between eager-cache and lazy-allocate based on whether the signal is reachable at `mw::new` time; both are acceptable.

2. Inline the F5 hotkey caller at rs:272. The input-path handler has `app: &mut App`, not `&mut EngineCtx`, so the unified `ReloadFiles` signature is unreachable from there. Replace:
   ```rust
   self.ReloadFiles(app);
   ```
   with:
   ```rust
   app.scheduler.fire(app.file_update_signal);
   ```
   The 1-line inline preserves observable behavior. This bifurcation is a deliberate design choice: D-007 gives ectx-threading on Cycle-path mutators; the input path has a different lifetime contract (app-bound, not Cycle-bound). One canonical `ReloadFiles(&self, ectx)` matches the C++ name; the input handler open-codes the fire to avoid a parallel `ReloadFilesFromInput(&self, app)` shim that would itself be a project-internal divergence.

   **Alternative noted, not chosen:** keep both signatures under different names (`ReloadFiles(&self, ectx)` + `ReloadFilesFromInput(&self, app)`). Rejected because the input path is a 1-line `scheduler.fire`, and keeping a method for it adds a Rust-only API surface that has no C++ analogue.

3. Delete `mw.to_reload` field (declared rs:78, initialized rs:106) and the `MainWindowEngine::Cycle` polling block (rs:389-397, the `let to_reload = with_main_window(|mw| mw.to_reload)…` block).

**Verification of MainWindowEngine survival:** `MainWindowEngine::Cycle` retains `close_signal` observation (rs:358-363), `title_signal` observation (rs:366-378), `startup_done` tracking (rs:382-387), and `to_close` self-delete (rs:399-403). Engine is not removed; only the to_reload block.

> Now formalised as D-009-polling-intermediary-replacement (sighting #2).

### Removal of `ClickFlags`

After all 7 reactions migrate to `IsSignaled`, the `ClickFlags` struct is dead. Remove the struct, the `click_flags` field on `emMainControlPanel`, and the `Rc<ClickFlags>` parameter threaded through `LMainPanel::new` (rs:545+), `GeneralPanel::new` (rs:645+), `AboutCfgPanel::new` (rs:734+), and `CommandsPanel::new` (rs:896+). Replace with `Rc<Cell<ButtonSignals>>` (or `Rc<RefCell<Option<ButtonSignals>>>`) per §"Subscribe shape" above — the new payload only needs to traverse to `CommandsPanel`, since rows 221/222/223 read their signal directly from the buttons owned by `emMainControlPanel`.

The `on_click` closures on the four `CommandsPanel`-owned buttons (rows 220, 224, 225, 226) — currently set to write into `ClickFlags` cells — get **deleted entirely**. Click signals fire automatically when the button registers a click; no closure is needed to "publish" anything. The button's existing `emButton::Input` handling fires `click_signal` directly. Rows 222 and 223 (auto-hide check buttons) have no `on_click`/`on_check` to remove — none was ever wired (see §"Check-button rows").

Test-only assertions about `ClickFlags` get deleted along with the struct.

## Tests

Behavioral coverage:
- The 5 fully-actionable rows (`new_window`, `auto_hide_control_view`, `auto_hide_slider`, `reload`, `close`) gain unit tests: simulate a button click, advance the engine, assert the side effect (config field flipped, `mw.to_close` set, `file_update_signal` fired, etc.).
- The 2 App-bound stub rows (`fullscreen`, `quit`) get tests that assert the subscription fires (i.e., the click signal arrives in the panel's Cycle and the panel observes it via `IsSignaled`); the stubbed log is exercised. When App access lands, those tests get extended.

Reload-relay specific:
- Existing `MainWindowEngine` tests that exercise the to_reload polling (if any) must be removed or migrated.
- New test: panel reload-button click → `file_update_signal` fires synchronously the same Cycle (no one-tick defer). Mirrors C++ behavior.
- New test: F5 hotkey through `emMainWindow::HandleInput` still fires `file_update_signal` (inlined `app.scheduler.fire(...)` path).

`cargo-nextest ntr` must remain green. Golden tests are unaffected (no pixel-output path touched).

## Sequencing

**Single PR.** All 7 row conversions, the `ClickFlags` removal, and the `mw.to_reload` relay deletion land together. Splitting risks leaving the Cycle body half-converted (some flag-takes, some IsSignaled-checks) which is harder to reason about than a clean swap.

**Order relative to B-006:** non-blocking. Whichever lands first creates the first-Cycle init block; the second adds its `ectx.connect(...)` calls into that block. If B-012 lands first, it creates the block with 7 click-signal connects; B-006 then adds 3 more. If B-006 lands first, the block has 3; B-012 adds 7 more. The merge is mechanical at the implementation level.

**Order relative to B-019:** B-019 must land first (per the bucket sketch's inbound notes and B-019's own design). B-019 strips the camouflage `DIVERGED:` blocks at rs:35, rs:303, rs:320; B-012 then performs the structural conversion without re-introducing the framing. If B-012 lands before B-019, the camouflage annotations would be stale (point at code that no longer exists) but not actively harmful — B-019 cleans up. Preferred order: B-019 → B-012.

## Verification

Post-implementation:

1. `cargo check --workspace` — must pass.
2. `cargo clippy --workspace -- -D warnings` — must pass.
3. `cargo-nextest ntr` — full suite, must pass.
4. `cargo xtask annotations` — must pass. No new `DIVERGED:` blocks added by this work; B-019's removals are independent. Note: the existing `DIVERGED:` block in `CommandsPanel::new` (rs:908-910, `emPackGroup` vs `emLinearLayout`) is unrelated to B-012 and survives. Additionally, `CommandsPanel::create_children` (rs:1024-1029) has a free-text comment about flattening C++'s `lCloseQuit` sub-layout into the main commands layout; this is a pre-existing structural divergence without an annotation. B-012 inherits but does not introduce or remove it; reconcile separately if needed (B-019 may already have triaged this; if not, file a follow-up).
5. `rg -n 'ClickFlags|to_reload' crates/emmain/` — expected: zero hits (struct, field, all references gone).
6. `rg -n 'flags\.[a-z_]+\.take\(\)' crates/emmain/src/emMainControlPanel.rs` — expected: zero hits.
7. Manual: read `emMainControlPanel::Cycle` end-to-end; the body should be 10 `IsSignaled` checks (3 from B-006 + 7 from B-012) inside the post-init region, no flag polling.

## Reconciliation summary for working-memory session

- **Bucket status:** B-012 → designed.
- **Decision citations finalized:** D-002 (rule 1, all 7 rows), D-006 (subscribe shape, merges with B-006's block), D-007 (ectx threading on `ReloadFiles`).
- **Audit-data corrections:** none — all 7 "accessor present" verdicts confirmed via `emButton.rs:40`. No row reclassifies.
- **Pattern reclassifications:** none.
- **Cross-bucket prereq edges:**
  - **B-019 → B-012** (hard): camouflage annotations must drop before structural conversion.
  - **B-006 ↔ B-012** (soft): shared first-Cycle init block; second-to-land merges into the first's block.
- **New D-### proposals:** none from B-012's row set.
> **SUPERSEDED post-B-010 design return (2026-04-27):** The watch-list pattern below was promoted to **D-009-polling-intermediary-replacement** by B-010's brainstorm (commit `09f08710`) after sightings 3 (`FsbEvents`) and 4 (`generation` counter) crossed the 3-sighting threshold. B-012's `mw.to_reload` resolution is now sighting 2 of D-009. Block below preserved as historical record.

- **Candidate-if-rediscovered pattern (watch-list, not promoted):** **"Rust interposed a polling intermediary where C++ calls directly."** Two sightings to date:
  1. B-003 / D-002 §1 R-A — `AutoplayFlags.progress: Rc<Cell<f64>>` with no consumer (resolved by drop).
  2. B-012 — `mw.to_reload: bool` polled by `MainWindowEngine` to fire `file_update_signal` (resolved by ectx-threading on `ReloadFiles`).
  Two is not enough to promote. Flag for a third sighting; if a future bucket finds the same shape, propose a D-### covering "polling intermediaries that should be direct fires" (likely sibling to D-007 — D-007 covers model-mutator ectx threading; this would cover non-model intermediary deletion). Working-memory session owns the watch-list note in `decisions.md` if desired.
- **Reaction-body residual drift (App-bound rows):** the fullscreen and quit reactions stay stubbed (logs only) after B-012. The *subscription* drift is fixed; the *reaction-body* drift (App access from Cycle) is a separate axis not in B-012's scope. Reconciliation log should note this so a follow-up audit or bucket can pick up the reaction-body gap.
- **Out-of-bucket file edits:** `emMainWindow.rs` lines 78 (field decl), 106 (field init), 131 (`ReloadFiles` body), 272 (F5 hotkey caller), 389-397 (polling block). Restructure `ReloadFiles`, delete `to_reload` field and polling block, inline F5 hotkey caller. These are not in any other bucket's row set; they ride with B-012 because the relay deletion is the second hop of B-012's reload row.

## Coverage gaps and new decisions

- **Coverage gaps:** none. All 7 rows resolve under existing decisions.
- **New D-### proposals:** none ratified. One watch-list candidate (above); does not promote at two sightings. *(SUPERSEDED post-B-010: that candidate was promoted to D-009 by B-010's brainstorm `09f08710`; B-012's resolution is now sighting 2 of D-009.)*
- **Bucket-file corrections to apply:** confirm `B-012-rc-shim-mainctrl.md` open-questions section; the three open questions are all resolved by this design (Q1: rule-1 uniform; Q2: no escalations; Q3: each row reuses no shared subscriber, but all subscribe in the same panel's Cycle — single subscription per click signal).

## Open Questions for Implementer

1. The recommended `Rc<RefCell<Option<ButtonSignals>>>` handoff is one viable shape; if the panel-tree construction order admits a more direct path (e.g., the child sub-panels return `ButtonSignals` from their `new`), prefer the more direct path. The constraint is: at first Cycle, `self.bt_*_sig` fields must be populated.
2. The two App-bound stub rows (fullscreen, quit) currently log; preserve the logs verbatim. If the implementer notices a reachable App-access path that wasn't there before, file as a follow-up — do not in-line a fix in this PR.
3. The `mw.file_update_signal` cache field added on `mw` is a small piece of new state. `MainWindowEngine::new` captures `app.file_update_signal` at rs:1117; `mw::new` (rs:96) currently does not receive `app`. Two paths:
   - **Eager cache:** thread `file_update_signal` into `mw::new` at the call site that constructs `MainWindowEngine`. Requires touching the construction-site signature.
   - **Lazy cache (D-008 fallback):** `Cell<Option<SignalId>>` on `mw`, populated on first `ReloadFiles(ectx)` via `ectx`'s scheduler. Choose this if the eager threading cascades into too many call sites.
   Verify at construction-site enumeration time and choose; both preserve synchronous-fire semantics.
4. If the F5 hotkey inline at rs:272 conflicts with surrounding refactoring (e.g., a later bucket factors input-path scheduler access into a helper), prefer the helper over the open-code; equivalent observable behavior either way.

---

## Adversarial Review — 2026-05-01

Verified against current source: `crates/emmain/src/emMainControlPanel.rs` and `emMainWindow.rs`; C++ `src/emMain/emMainControlPanel.cpp:217-290`. All 7 P-004 row claims at the C++ side are accurate (`AddWakeUpSignal(BtX->GetClickSignal())` paired with `IsSignaled(...)` reactions in `Cycle`).

### Summary
- Critical: 2 | Important: 4 | Minor: 3 | Notes: 2

### Findings

1. **[Critical] Subscope mismatch — `LCloseQuitPanel` and `LAbtCfgCmdPanel` do not exist in Rust.** The design's Scope (lines 26-28), Architecture §"Removal of `ClickFlags`" (line 183), and §"Subscribe shape" (line 125) all refer to `LCloseQuitPanel`, `LAbtCfgCmdPanel`, `LMainPanel`. Verified by `rg LCloseQuitPanel|LAbtCfgCmdPanel crates/emmain/src/` → zero hits. The actual Rust panel hierarchy in `emMainControlPanel.rs` is: `emMainControlPanel` → `LMainPanel` (line 531) → `GeneralPanel` (line 631) → {`AboutCfgPanel` (line 728), `CommandsPanel` (line 881)}. Close/Quit/Reload/NewWindow/Fullscreen are all flattened into `CommandsPanel::create_children` (lines 919-1075), not split into a `LCloseQuitPanel`. The auto-hide buttons live as detached fields on `emMainControlPanel` itself (rs:179-183), not in any sub-panel. The design's removal-list at rs:466, rs:710, and rs:728+ targets phantom code. **Fix:** rewrite Scope and §"Removal" to reference `CommandsPanel` (rs:919+) for buttons 220/221/224/225/226 and `emMainControlPanel::create_children` (rs:272-343) for the `BtAutoHideControlView`/`BtAutoHideSlider` allocation sites (rows 222/223). This also reshapes the `ButtonSignals` handoff: `bt_fullscreen_sig`, `bt_auto_hide_*_sig` are reachable directly from the top-level panel because the buttons are already its fields; only `bt_new_window_sig`, `bt_reload_sig`, `bt_close_sig`, `bt_quit_sig` need a handoff out of `CommandsPanel`.

2. **[Critical] Rows 222 (auto_hide_control_view) and 223 (auto_hide_slider) have no on_click wiring today.** Inspecting `create_children` (rs:302-320) and `CommandsPanel::create_children` (rs:919-1075): the auto-hide check buttons are created with no `on_check` callback at all. The `flags.auto_hide_control_view.take()` and `flags.auto_hide_slider.take()` branches at rs:477-483 therefore can never observe `true` — those `Cell`s are write-zero/read-zero. The bucket sketch row table (lines 31-32) calls them "rc_cell_shim" but they are not even shimmed; they are pure dead code on the click-publication side. The design's per-row triage (lines 50-51) describes them as `flags.auto_hide_control_view.take()` shim sites but doesn't note that no producer exists. **Implication:** rows 222/223 remain reaction-body-incomplete in C++-fidelity terms even after B-012 wires the subscription — no click-publishing handler exists today and the design doesn't add one. The intended C++ behavior (`MainConfig->AutoHideControlView.Invert(); Save()`) requires the implementer to either (a) wire `on_check` in `create_children` *or* convert to the `IsSignaled(check_signal)` shape directly, with no shim involvement. The latter is what B-012 should do, but the design as written deletes the take-shim and converts to `IsSignaled`, which works because `emCheckButton::Input` already fires `check_signal` directly (rs:70). State this explicitly in the design so the implementer doesn't go hunting for missing `on_check` plumbing.

3. **[Important] Wrong line numbers in §"Reload row — two-hop relay" and §Verification.** Design says "delete `MainWindowEngine` polling block (rs:382-390)" (line 28, 175) — the actual block is at `emMainWindow.rs:389-397`. F5 hotkey caller is at `rs:272`, not rs:269 (line 27, 163, 250). `mw.to_reload` field is at rs:78 (correct) but its initialization is at rs:106, not rs:103 (line 28). `MainWindowEngine` survival items are at rs:358-363 (close), rs:366-378 (title), rs:382-387 (startup_done), rs:399-403 (to_close) — not 352-356/359-372/375-380/393-396 as written (line 177). **Fix:** scrub all line numbers against current HEAD.

4. **[Important] Fullscreen row (221) uses `emCheckButton`, not `emButton` — design conflates two signal kinds.** C++ cpp:221 is `AddWakeUpSignal(BtFullscreen->GetClickSignal())`. In Rust, `BtFullscreen` is an `emCheckButton` (rs:171, 286-294). `emCheckButton` exposes `check_signal` (emCheckButton.rs:31) but the parallel "click_signal" is whatever the underlying `emButton` exposes. Verify: does `emCheckButton` re-expose a click signal independent of `check_signal`, or does the click reduce to a check toggle? In C++ `emCheckButton` inherits `emButton::ClickSignal`. Implementer must confirm Rust `emCheckButton` either exposes the inherited `click_signal` field or that the design switches row 221 to subscribe to `check_signal`. The reaction (`MainWin.ToggleFullscreen()`) responds to clicks, not to checked-state transitions — so subscribing to the click signal is C++-faithful; subscribing to `check_signal` would fire on programmatic `SetChecked` calls in row 218's reaction, creating a feedback loop. **Required:** explicit per-row note that rows 221/222/223 (all check buttons) must subscribe to the *click* signal, not the check signal, and verify that accessor exists on `emCheckButton`. (Design currently assumes uniform `emButton.click_signal` across all 7 rows — line 48 — which is wrong by row count: 4 rows are buttons, 3 are check-buttons.)

5. **[Important] D-008 lazy-allocation may apply to `mw.file_update_signal` cache.** Open Question 3 (line 251) flags that `mw` gains `file_update_signal: SignalId` cached at construction. But `mw` is constructed at rs:1117 (per design) — the actual line in source `mw` instantiation isn't visible in the snippets read. If `mw::new` runs without `app.file_update_signal` in scope (Open Question 3 already worries this), the design needs an explicit construction-site verification step or fall back to D-008 lazy allocation. The design currently presents only the eager-cache option. **Fix:** add a sub-bullet under "Changes to `emMainWindow.rs`" stating that if construction-site enumeration shows `file_update_signal` is not yet allocated when `mw` is built, use `Cell<SignalId>` initialized null and lazy-allocate on first `ReloadFiles(ectx)` call per D-008.

6. **[Important] Design recommends `Rc<Cell<ButtonSignals>>` but `SignalId` is `Copy`-but-the-struct-isn't-trivially-Copy and `Cell` requires `Copy`.** Line 127: `Rc<Cell<ButtonSignals>>` only works if `ButtonSignals: Copy`. Confirm `SignalId` is `Copy` (the design asserts so on line 39 — verify in source). If `ButtonSignals` is `{ a: SignalId, b: SignalId, ... }` of `Copy` fields, the struct can derive `Copy`. Implementer needs the `derive(Clone, Copy)` line in the design or this will fail compile. **Fix:** add concrete struct definition with derives.

7. **[Minor] Quit row (226) reaction body claim contradicts current code.** Line 140 says "rs:334-338 logs only." Actual code at rs:497-501 shows `flags.quit.take()` already does only `log::info!`, no closure body. The "App access from Cycle is independent of shim removal" claim (line 138) is correct — but cite rs:497-501 not rs:334-338.

8. **[Minor] Annotation lint test missing from Verification step.** Step 4 (line 217) says `cargo xtask annotations` "must pass. No new `DIVERGED:` blocks added." But §"Architecture / App-bound reactions" line 140 explicitly says "**No `DIVERGED:` annotation should be added**" for fullscreen/quit reaction-body residual. Good. However, the existing `DIVERGED:` block in `CommandsPanel::new` at rs:908-910 (`emPackGroup` vs `emLinearLayout`) is unrelated and survives. State that.

9. **[Minor] CommandsPanel children deviate from C++ structure (lCloseQuit not modeled).** rs:1024-1029 explicitly notes "Close and Quit are in a sub-layout in C++ (lCloseQuit), but here we flatten them into the main commands layout." This is a pre-existing structural divergence with a comment but no `DIVERGED:` annotation. B-012 inherits this shape. Either the audit already accepted it (B-019's annotation pass should have triaged) or it is stale. Implementer should not introduce new framing here; flag for reconciliation.

### Notes (not findings)

- N1. Cross-bucket: B-010 reconciliation (D-009 promotion `09f08710`) introduced "remove polling intermediary; thread ectx into mutation site" as the canonical recipe. B-012's `mw.to_reload` deletion is a textbook D-009 application; design correctly cites this (lines 213-214, 232).
- N2. Scope-split between subscription drift (B-012) and reaction-body drift (separate audit) is unambiguous in the design (lines 31-35, 142, 238 — Out of scope, App-bound stub note, residual). Rows 221 (fullscreen) and 226 (quit) keep stubbed reaction bodies; subscription wiring is fixed. The known-residual statement in the prompt is accurately captured.

### Recommended Pre-Implementation Actions

1. **Rewrite §Scope, §"Removal of ClickFlags", §"Subscribe shape" to reference real Rust panel names** (`CommandsPanel`, `GeneralPanel`, `AboutCfgPanel`) — not the phantom `LCloseQuitPanel` / `LAbtCfgCmdPanel`. (Finding 1.)
2. **Add per-row note on signal kind** for the 3 check-button rows (221/222/223): subscribe to click signal, not check signal; verify `emCheckButton` exposes the inherited click signal accessor before bucket dispatch. (Finding 4.)
3. **Note rows 222/223 have no current click-publisher** — conversion to `IsSignaled(click_signal)` is correct because `emCheckButton::Input` fires the signal directly; no on_check shim to remove. (Finding 2.)
4. **Concretize `ButtonSignals` struct definition** with `#[derive(Clone, Copy, Default)]` and a verified `SignalId: Copy` cite. (Finding 6.)
5. **Re-pin all line numbers** against current HEAD (`rs:389-397` polling block, `rs:272` F5 caller, `rs:497-501` quit log, `rs:106` to_reload init). (Findings 3, 7.)
6. **Add D-008 fallback note** for `mw.file_update_signal`: if construction-site enumeration shows the signal isn't reachable, use lazy allocation (Cell<SignalId>::null + ensure-on-fire). (Finding 5.)
7. **Verify `mw` construction site** (the actual file:line where `emMainWindow::new` is called from `MainWindowEngine` setup) and document the `file_update_signal` handoff explicitly so Open Question 3 is closed at design time, not implementer time.

---

## Amendment Log — 2026-05-01

Adversarial Review (above) was folded into the design body. Original Adversarial Review preserved verbatim; design body updated as follows.

### Critical findings — fixed

- **C-1 (phantom panel names).** §Scope, §Non-goals, §"Subscribe shape", §"Removal of `ClickFlags`", §"Reconciliation summary" rewritten to reference the real Rust hierarchy: `emMainControlPanel` → `LMainPanel` (rs:531) → `GeneralPanel` (rs:631) → `{AboutCfgPanel` (rs:728), `CommandsPanel` (rs:881)}. Buttons 220/224/225/226 live in `CommandsPanel::create_children` (rs:919-1075); 221 is a field on `emMainControlPanel` (rs:171); 222/223 are detached fields (rs:181, 183). Phantom `LCloseQuitPanel`/`LAbtCfgCmdPanel` references removed.
- **C-2 (rows 222/223 have no on_click).** Per-row triage table now states explicitly that rows 222/223 have no `on_click`/`on_check` wired today; the `Cell` is write-zero/read-zero, so the existing reaction body never fires. New §"Check-button rows" subsection states that conversion to `IsSignaled(click_signal)` works *because* `emCheckButton::Input` fires the signal directly — no callback plumbing to remove.

### Important findings — fixed

- **I-1 (stale line numbers).** §Scope, §"Reload row — two-hop relay", §Verification, §"Reconciliation summary": polling block updated to rs:389-397; F5 caller to rs:272; `to_reload` init to rs:106; `MainWindowEngine` survival items to rs:358-363/366-378/382-387/399-403; `ReloadFiles` body to rs:131; reaction-body sites to rs:463/469/477/481/485/492/497.
- **I-2 (check buttons vs plain buttons).** New §"Check-button rows" section: rows 221/222/223 must subscribe to the inherited *click* signal, not `check_signal`, to avoid feedback loops with row 218's programmatic `SetChecked`. Implementer-prerequisite note: verify `emCheckButton` exposes the inherited accessor; add a 1-line `pub fn` if missing (not a divergence — C++ `emButton::GetClickSignal()` is public).
- **I-3 (D-008 fallback for `mw.file_update_signal`).** §"Reload row — two-hop relay" sub-bullet and Open Question 3 expanded: if construction-site enumeration shows the signal cannot be threaded into `mw::new` (rs:96), fall back to D-008 lazy `Cell<Option<SignalId>>` allocated on first `ReloadFiles(ectx)`.
- **I-4 (`ButtonSignals: Copy`).** §"Subscribe shape" now contains the concrete struct definition with `#[derive(Clone, Copy, Default)]` and an explicit cite to `emSignal.rs:7` confirming `SignalId: Copy`. The handoff payload reduced to four signals (rows 220/224/225/226), since rows 221/222/223 read directly from owned buttons.

### Minor findings — addressed

- Quit-row reaction-body cite corrected from rs:334-338 to rs:497-501 in §"App-bound reactions".
- Annotation lint note added in §Verification step 4: pre-existing `DIVERGED:` block at rs:908-910 (`CommandsPanel::new` `emPackGroup` vs `emLinearLayout`) and the un-annotated `lCloseQuit` flatten-comment at rs:1024-1029 both survive B-012; reconcile separately.

### Preserved verbatim

The "Adversarial Review — 2026-05-01" section above this log is unchanged.
