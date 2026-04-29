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
- The 7 P-004 rows: `emMainControlPanel-220..226` (rs:296, 301, 311, 315, 319, 328, 334).
- Removal of the `ClickFlags` struct (rs:39-46) and its `Rc` plumbing through `MainCtrlPanel`, `LMainPanel`, `LAbtCfgCmdPanel`, `LCloseQuitPanel` (rs:140, 372, 466, 710).
- Restructuring `emMainWindow::ReloadFiles` (rs:128) to `(&self, ectx: &mut EngineCtx<'_>)`; inlining the F5 hotkey caller at rs:269.
- Deleting `mw.to_reload` field (rs:78, 103) and the `MainWindowEngine` polling block (rs:382-390).
- Caching button `SignalId`s on `emMainControlPanel` (panel-level fields) so `Cycle` can subscribe and react without owning the buttons themselves.

Out of scope:
- The 3 P-002 rows (217/218/219) — owned by B-006.
- Any other panel-tree restructuring.
- New global decisions (none surface; see §"Coverage gaps").
- Annotation removal at rs:35/303/320 — B-019 owns.

## Non-goals

- Promoting buttons (`BtNewWindow`, `BtFullscreen`, `BtReload`, etc.) to direct `emMainControlPanel` fields. C++ has them as members; Rust currently nests them inside `LCloseQuitPanel` / `LMainPanel` / `LAbtCfgCmdPanel` sub-panels. Promoting is a larger structural change with no observable-behavior payoff. Caching the buttons' `click_signal: SignalId` (which is `Copy`) at the top-level panel is sufficient — the panel needs the signal id, not the button.
- Refactoring `MainWindowEngine` beyond deleting the to_reload polling block.

## Per-row Triage Table

All 7 rows are uniform — same C++ shape, same Rust shape, same disposition.

| # | Row ID | C++ site | C++ pattern | Rust today | Accessor verified | D-002 disposition |
|---|---|---|---|---|---|---|
| 1 | `emMainControlPanel-220` | cpp:220 (`AddWakeUpSignal(BtNewWindow->GetClickSignal())`) + cpp:262 (`IsSignaled` → `MainWin.Duplicate()`) | signal-subscribe | rs:296 (`flags.new_window.take()` after on_click sets cell) | `emButton.click_signal` exists at `emButton.rs:40`; `Copy` SignalId | rule 1 — convert |
| 2 | `emMainControlPanel-221` | cpp:221 + cpp:266 (`IsSignaled` → `MainWin.ToggleFullscreen()`) | signal-subscribe | rs:301 (`flags.fullscreen.take()`) | same | rule 1 — convert |
| 3 | `emMainControlPanel-222` | cpp:222 + cpp:270 (`IsSignaled` → `MainConfig->AutoHideControlView.Invert(); Save()`) | signal-subscribe | rs:311 (`flags.auto_hide_control_view.take()`) | same | rule 1 — convert |
| 4 | `emMainControlPanel-223` | cpp:223 + cpp:275 (`IsSignaled` → `MainConfig->AutoHideSlider.Invert(); Save()`) | signal-subscribe | rs:315 (`flags.auto_hide_slider.take()`) | same | rule 1 — convert |
| 5 | `emMainControlPanel-224` | cpp:224 + cpp:280 (`IsSignaled` → `MainWin.ReloadFiles()`) | signal-subscribe | rs:319 (`flags.reload.take()` → `mw.to_reload = true`; second hop in `MainWindowEngine::Cycle` rs:384) | same | rule 1 — convert; **plus** unwire the two-hop relay (see §"Reload row — two-hop relay") |
| 6 | `emMainControlPanel-225` | cpp:225 + cpp:284 (`IsSignaled` → `MainWin.Close()`) | signal-subscribe | rs:328 (`flags.close.take()`) | same | rule 1 — convert |
| 7 | `emMainControlPanel-226` | cpp:226 + cpp:288 (`IsSignaled` → `MainWin.Quit()`) | signal-subscribe | rs:334 (`flags.quit.take()`) | same | rule 1 — convert |

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

The `bt_*_sig: SignalId` fields are populated when each child sub-panel creates its button. The button creation sites currently live in `LCloseQuitPanel::create_children` (rs:728+) for close/quit/reload and analogous sites for the others. Pattern: when creating each button, capture `btn.click_signal` and write it back into a slot the top-level panel can read at first-Cycle (e.g., via the existing `Rc<ClickFlags>` plumbing repurposed as `Rc<RefCell<ButtonSignals>>`, or via a new typed channel, or — since the button creation runs synchronously inside the panel's child-panel construction — by walking the panel tree once at end-of-init).

**Recommended implementation shape:** replace `Rc<ClickFlags>` with `Rc<Cell<ButtonSignals>>` where `ButtonSignals` is a plain copyable struct of `SignalId`s. Each child sub-panel writes its button signals into the cell at construction. The top-level `emMainControlPanel` reads the cell once at first Cycle (after children exist) and copies the values into its own fields. `ButtonSignals` itself is *not* a shim — `SignalId` is `Copy` and the cell holds a one-shot init handoff, not running state. (If the borrow checker is amenable, a one-shot `Option<ButtonSignals>` field set during child creation works equivalently and is more direct.)

This is the exact shape D-006 already endorses: panel-level signal field, populated by construction, subscribed at first Cycle. The `Rc` plumbing footprint is the same as today's `ClickFlags`; only the payload type changes.

### App-bound reactions

Three reactions need `&mut App` (not just `&mut EngineCtx`):
- `MainWin.Duplicate()` — not yet ported (current code logs and returns).
- `MainWin.ToggleFullscreen(app)` — `App` access for `app.windows.get_mut(...)`.
- `MainWin.Quit(app)` — `App` access for shutdown sequence.

Today these are all stubbed-out behind `with_main_window` closures that don't actually call the App-bound methods (rs:301-309 logs only; rs:334-338 logs only). The current code has the same gap: it can't reach App from inside `Cycle`. Converting to signal-subscribe doesn't make this worse and doesn't make it better — the App-access gap is independent of the shim removal.

**Action:** preserve current reaction bodies for the App-bound rows. Replace `flags.X.take()` with `ectx.IsSignaled(self.bt_X_sig)`; leave the inner closure body unchanged (logs + the stubbed work). When App access becomes reachable from Cycle (separate work), the closure bodies fill in. **No `DIVERGED:` annotation should be added** — the gap is not new, and there's no forced-category claim to make. If a future linter pass needs to flag the stub, that's a pre-existing concern outside B-012's row set.

The two App-bound stub sites (fullscreen, quit) **remain drifted** after B-012 in the sense that the *reaction body* is still a no-op log. B-012 fixes the *subscription* drift (which is what P-004 measures); the App-access gap is a separate axis. Reconciliation log entry should note this so a follow-up bucket or audit pass can re-check the reaction-body drift.

### Reload row — two-hop relay

The `flags.reload.take() → mw.to_reload = true → MainWindowEngine::Cycle polls → fires file_update_signal` chain compresses to a synchronous `mw.ReloadFiles(ectx)` call from inside `emMainControlPanel::Cycle`, mirroring C++ cpp:281 exactly.

**Changes to `emMainWindow.rs`:**

1. Restructure `ReloadFiles` (rs:128):
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
   The `mw` struct gains `pub(crate) file_update_signal: SignalId` cached at construction (the value is already captured by `MainWindowEngine` at rs:344/1106; mw caches it from the same source).

2. Inline the F5 hotkey caller at rs:269. The input-path handler has `app: &mut App`, not `&mut EngineCtx`, so the unified `ReloadFiles` signature is unreachable from there. Replace:
   ```rust
   self.ReloadFiles(app);
   ```
   with:
   ```rust
   app.scheduler.fire(app.file_update_signal);
   ```
   The 1-line inline preserves observable behavior. This bifurcation is a deliberate design choice: D-007 gives ectx-threading on Cycle-path mutators; the input path has a different lifetime contract (app-bound, not Cycle-bound). One canonical `ReloadFiles(&self, ectx)` matches the C++ name; the input handler open-codes the fire to avoid a parallel `ReloadFilesFromInput(&self, app)` shim that would itself be a project-internal divergence.

   **Alternative noted, not chosen:** keep both signatures under different names (`ReloadFiles(&self, ectx)` + `ReloadFilesFromInput(&self, app)`). Rejected because the input path is a 1-line `scheduler.fire`, and keeping a method for it adds a Rust-only API surface that has no C++ analogue.

3. Delete `mw.to_reload` field (rs:78, 103) and the `MainWindowEngine::Cycle` polling block (rs:382-390).

**Verification of MainWindowEngine survival:** `MainWindowEngine::Cycle` retains `close_signal` observation (rs:352-356), `title_signal` observation (rs:359-372), `startup_done` tracking (rs:375-380), and `to_close` self-delete (rs:393-396). Engine is not removed; only the to_reload block.

> Now formalised as D-009-polling-intermediary-replacement (sighting #2).

### Removal of `ClickFlags`

After all 7 reactions migrate to `IsSignaled`, the `ClickFlags` struct (rs:39-46) is dead. Remove the struct, the field on `MainCtrlPanel` (rs:140), and the `Rc<ClickFlags>` parameters threaded through `LMainPanel::new` (rs:383), `LAbtCfgCmdPanel::new` (rs:477), `LCloseQuitPanel::new` (rs:718). Replace with `Rc<RefCell<Option<ButtonSignals>>>` (or equivalent) per §"Subscribe shape" above.

The `on_click` closures on each button (rs:747, 762, 783, 805, 821, etc.) — currently set to write into `ClickFlags` cells — get **deleted entirely**. Click signals fire automatically when the button registers a click; no closure is needed to "publish" anything. The button's existing `emButton::Input` handling fires `click_signal` directly.

Test-only assertions about `ClickFlags` (rs:955-969) get deleted along with the struct.

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
4. `cargo xtask annotations` — must pass. No new `DIVERGED:` blocks added by this work; B-019's removals are independent.
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
- **Out-of-bucket file edits:** `emMainWindow.rs` lines 78, 103, 128, 269, 382-390 — restructure `ReloadFiles`, delete `to_reload` field and polling block, inline F5 hotkey caller. These are not in any other bucket's row set; they ride with B-012 because the relay deletion is the second hop of B-012's reload row.

## Coverage gaps and new decisions

- **Coverage gaps:** none. All 7 rows resolve under existing decisions.
- **New D-### proposals:** none ratified. One watch-list candidate (above); does not promote at two sightings. *(SUPERSEDED post-B-010: that candidate was promoted to D-009 by B-010's brainstorm `09f08710`; B-012's resolution is now sighting 2 of D-009.)*
- **Bucket-file corrections to apply:** confirm `B-012-rc-shim-mainctrl.md` open-questions section; the three open questions are all resolved by this design (Q1: rule-1 uniform; Q2: no escalations; Q3: each row reuses no shared subscriber, but all subscribe in the same panel's Cycle — single subscription per click signal).

## Open Questions for Implementer

1. The recommended `Rc<RefCell<Option<ButtonSignals>>>` handoff is one viable shape; if the panel-tree construction order admits a more direct path (e.g., the child sub-panels return `ButtonSignals` from their `new`), prefer the more direct path. The constraint is: at first Cycle, `self.bt_*_sig` fields must be populated.
2. The two App-bound stub rows (fullscreen, quit) currently log; preserve the logs verbatim. If the implementer notices a reachable App-access path that wasn't there before, file as a follow-up — do not in-line a fix in this PR.
3. The `mw.file_update_signal` cache field added on `mw` is a small piece of new state. Verify at construction (mw is built where `MainWindowEngine` is built, rs:1106) that the same `app.file_update_signal` value is captured into both — they must agree by construction or the relay-replacement breaks.
4. If the F5 hotkey inline at rs:269 conflicts with surrounding refactoring (e.g., a later bucket factors input-path scheduler access into a helper), prefer the helper over the open-code; equivalent observable behavior either way.
