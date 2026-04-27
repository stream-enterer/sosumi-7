# B-019-stale-annotations — Design

**Bucket:** B-019-stale-annotations
**Pattern:** P-009-stale-annotation
**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Source bucket file:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-019-stale-annotations.md`
**Cited decisions:** D-001 (only as documentation cross-reference; see Triage row 9 — D-001 ultimately does *not* govern `emFileModel.rs:490`)
**Prereq buckets:** none for the annotation cleanup itself; cross-bucket masked-drift edges noted per row

---

## Goal

Resolve the 9 P-009 cleanup items from `preexisting-diverged.csv`: the 8 `DIVERGED:` annotations whose four-question re-validation failed, plus the 1 wrong-category annotation at `emFileModel.rs:490`.

Each item resolves to one of three actions per CLAUDE.md "Annotation Vocabulary":

1. **Remove annotation entirely** — the underlying code is a *below-observable-surface idiom adaptation*. Per CLAUDE.md: "Below-surface adaptations that preserve observable behavior and introduce no structural commitment are unannotated." Replace the `DIVERGED:` block with a plain prose comment if context-clarifying prose is warranted; otherwise delete the comment.
2. **Replace with corrected category** — the underlying divergence is genuinely forced but the original category was wrong.
3. **Remove annotation AND flag masked drift** — the `DIVERGED:` claim was a fig leaf over a real signal-wiring drift that lives in another bucket. The annotation goes away here; the drift-fix lands in the cited target bucket. Both must land in coordinated PRs (or the masked drift first, then this cleanup).

## Scope

In scope:
- Touching the 9 cited annotation blocks (text edits / deletions).
- Cross-referencing the masked-drift cleanup items to their target buckets via comment hand-off in this design doc and the receiving buckets' open-questions sections.

Out of scope:
- The underlying signal-wiring fixes themselves (those are owned by B-002, B-012, B-016 — see triage table).
- The `cargo xtask annotations` linter (sufficient as-is — its category-required check already catches the wrong-category class).
- Any new global decision (none surface from this bucket).

## Non-goals

- Re-litigating the four-question forced-divergence test on each annotation. The audit already ran that re-validation; this bucket consumes its verdicts.
- Generalizing P-009 into a recurring annotation-quality lint. Out of scope for B-019; could be a separate F010 follow-up.

## Per-item Triage Table

| # | File:line | Claimed category | Actual situation | Decision | Target bucket (if drift) |
|---|---|---|---|---|---|
| 1 | `crates/emfileman/src/emDirPanel.rs:117` | language-forced | emDirModel polling pattern; the annotation describes the polling-shape that B-016 already classifies as P-007 drift (`emDirPanel-37` at rs:344). Annotation is a fig leaf for the drift. | remove + flag masked drift | **B-016-polling-no-acc-emfileman** |
| 2 | `crates/emmain/src/emMainControlPanel.rs:35` | language-forced | `ClickFlags` `Rc<Cell<bool>>` shim is the exact P-004 pattern; B-012 owns all 7 widget-click-shim rows in this file. The doc-comment block at line 35 introduces the shim that B-012 will dismantle. | remove + flag masked drift | **B-012-rc-shim-mainctrl** |
| 3 | `crates/emmain/src/emMainControlPanel.rs:303` | language-forced | `flags.fullscreen.take()` consumer site for the same shim. Sits inside the Cycle handler that B-012's `emMainControlPanel-221` (rs:301) refactors. | remove + flag masked drift | **B-012-rc-shim-mainctrl** |
| 4 | `crates/emmain/src/emMainControlPanel.rs:320` | language-forced | `flags.reload.take()` consumer site, chained `mw.to_reload` Cell relayed through MainWindowEngine. Same B-012 row family (`emMainControlPanel-224`, rs:319). The annotation extends the rc-shim chain across two panels. | remove + flag masked drift | **B-012-rc-shim-mainctrl** (with note: relay through `emMainWindow.to_reload` is a second hop the bucket should consume) |
| 5 | `crates/emcore/src/emDialog.rs:35` | language-forced | `DialogCheckFinishCb` is a callback-slot type alias replacing C++ virtual `CheckFinish`. Rust *can* use trait dispatch — language-forced doesn't survive the four-question test (try-the-shape: traits compile). Observable behavior is identical (same dispatch order, same arguments). Below-surface idiom adaptation. | remove annotation | n/a |
| 6 | `crates/emcore/src/emDialog.rs:523` | language-forced | `on_cycle_ext` callback slot lets emFileDialog inject post-base Cycle logic. The C++ pattern is subclass override; Rust uses callback. Like row 5, traits would compile. Observable behavior identical. Below-surface adaptation. | remove annotation | n/a |
| 7 | `crates/emcore/src/emFileDialog.rs:68` | language-forced | `fsb_trigger_sig: SignalId` cache "exists solely because Rust tests need to fire the signal externally." Test-infrastructure exposure, not a port divergence. The annotation describes test ergonomics, not observable behavior. | remove annotation (replace with plain doc-comment explaining test-cache role) | n/a |
| 8 | `crates/emcore/src/emFileDialog.rs:140` | language-forced | Re-wake-by-returning-`true` because the base `DialogPrivateEngine::Cycle` body already ran before `on_cycle_ext`. Rust port-internal scheduling glue with no C++ counterpart shape — observable timing is preserved (same Cycle ordering, same wake semantics). Below-surface. | remove annotation | n/a |
| 9 | `crates/emcore/src/emFileModel.rs:490` | upstream-gap-forced | The block describes deferred PSAgent integration. CSV's `corrected_category = language-forced`. Per audit decision the underlying issue is the Rust `PriSchedModel` callback signature being incompatible with C++ `GotAccess→WakeUp` — that *is* language-forced (callback signature shape is a language/SDK constraint). PSAgent itself is upstream-ported but its callback shape can't admit the C++ pattern. **D-001 does NOT apply** (D-001 is `u64`/`SignalId` accessor flips in emfileman). The bucket file's "interacts with D-001" claim is wrong and is corrected here. | replace category text only (`upstream-gap-forced` → `language-forced`) | n/a |

**Summary:** 4 mask-drift removals (rows 1–4), 4 below-surface removals (rows 5–8), 1 category swap (row 9). Zero items require a "keep the annotation but rewrite justification" outcome.

## Per-item Action Detail

### Item 1 — `crates/emfileman/src/emDirPanel.rs:117`

**Action:** Remove the `DIVERGED:` block (lines 117–121). Keep the preceding doc-comment paragraph (lines 111–115). The deletion does not add or remove a doc paragraph; the panel's port-of-emDirPanel commentary remains.

**Diff sketch:**

```diff
 /// Directory grid panel.
 /// Port of C++ `emDirPanel` (extends emFilePanel).
 ///
 /// Displays directory entries in a grid layout. Lazily acquires emDirModel
 /// when viewed. Creates/updates emDirEntryPanel children from model entries.
-///
-/// DIVERGED: (language-forced) C++ emDirPanel connects emDirModel as a FileModelState via
-/// SetFileModel. Rust drives loading directly in Cycle using
-/// `get_file_state()` to query the model's phase, because emDirModel does
-/// not implement FileModelState — it wraps emDirModelData directly without
-/// scheduler integration.
 pub struct emDirPanel {
```

**Cross-bucket hand-off:** B-016 owns the polling fix at `emDirPanel.rs:344` (`emDirPanel-37`). The structural choice this annotation tries to justify (driving loading from Cycle via polling instead of subscribe) IS the drift B-016 will eliminate. If B-016 lands first, this removal is purely cosmetic. If this bucket lands first, B-016 still owns the drift; nothing in B-016's scope changes.

### Item 2 — `crates/emmain/src/emMainControlPanel.rs:35`

**Action:** Remove the `DIVERGED:` block (lines 35–36). Keep the section banner and prose context (lines 33–34). The `ClickFlags` struct definition itself is untouched — B-012 will remove or repurpose it as part of its rc-shim conversion.

**Diff sketch:**

```diff
 // ── Click flags ──────────────────────────────────────────────────────────────
 // Shared state between button on_click callbacks and the Cycle method.
-// DIVERGED: (language-forced) C++ uses AddWakeUpSignal / IsSignaled. Rust uses Rc<Cell<bool>>
-// flags set by on_click callbacks and polled in Cycle.

 #[derive(Default)]
 struct ClickFlags {
```

**Cross-bucket hand-off:** B-012 (`B-012-rc-shim-mainctrl`) owns conversion of the 7 widget-click rows. After B-012 lands, `ClickFlags` itself likely deletes; this section banner becomes vestigial too. Coordinate ordering: B-012 first → this cleanup is no-op (the block is gone). This bucket first → B-012 still wholesale-replaces.

### Item 3 — `crates/emmain/src/emMainControlPanel.rs:303`

**Action:** Remove the inline `DIVERGED:` block (lines 303–305). Keep the closure body's `log::info!` line. The fullscreen-toggle no-op log itself isn't load-bearing; it's a B-012 row.

**Diff sketch:**

```diff
         if flags.fullscreen.take() {
             crate::emMainWindow::with_main_window(|mw| {
-                // DIVERGED: (language-forced) C++ has direct MainWin reference; Rust uses
-                // thread_local. ToggleFullscreen requires &mut App which
-                // we don't have in Cycle. Log for now.
                 log::info!("emMainControlPanel: Fullscreen toggle requested (requires App access)");
                 let _ = mw;
             });
         }
```

**Cross-bucket hand-off:** B-012's `emMainControlPanel-221` (rs:301) row owns this site. B-012 will replace the `flags.fullscreen.take()` polling with a click-signal subscribe; after that, the closure shape changes wholesale.

### Item 4 — `crates/emmain/src/emMainControlPanel.rs:320`

**Action:** Remove the inline `DIVERGED:` block (lines 320–322). Keep the `with_main_window` closure body that sets `mw.to_reload = true`.

**Diff sketch:**

```diff
         if flags.reload.take() {
-            // DIVERGED: (language-forced) C++ calls MainWin.ReloadFiles() directly via signal.
-            // Rust sets a flag on emMainWindow, polled by MainWindowEngine which
-            // has EngineCtx access to fire the file_update_signal.
             crate::emMainWindow::with_main_window(|mw| {
                 mw.to_reload = true;
             });
         }
```

**Cross-bucket hand-off:** B-012's `emMainControlPanel-224` (rs:319) owns this site. B-012 must additionally consume the `mw.to_reload` second-hop relay through MainWindowEngine — the rc-shim chain spans two panels, not one. **Flag for working-memory session:** add a note to B-012's open-questions section about the two-hop relay (this bucket file documents the hand-off).

### Item 5 — `crates/emcore/src/emDialog.rs:35`

**Action:** Remove the `DIVERGED:` block (lines 35–40). The type-alias `DialogCheckFinishCb` itself stays. Optionally, prepend a short prose comment explaining the callback signature requirement (`&mut DlgPanel + &mut EngineCtx`) if a future reader needs it; the prose loses the `DIVERGED:` framing.

**Diff sketch:**

```diff
 type DialogFinishCb = crate::emEngineCtx::WidgetCallbackRef<DialogResult>;
-/// DIVERGED: (language-forced) C++ `emDialog::CheckFinish` is a virtual method with no
-/// extra args — subclasses reach into self's fields directly. Rust uses
-/// a callback slot on `DlgPanel`; the closure needs `&mut DlgPanel` +
-/// `&mut EngineCtx<'_>` to read tree state (e.g. emFileDialog's fsb
-/// child panel) and spawn transient sub-dialogs. Matches `DialogCycleExt`
-/// (Phase 3.6 Task 2).
+/// Callback slot used in place of C++'s virtual `emDialog::CheckFinish`.
+/// Closure receives `&mut DlgPanel + &mut EngineCtx` so it can read tree
+/// state and spawn transient sub-dialogs (e.g. emFileDialog's overwrite
+/// confirmation). Matches `DialogCycleExt` (Phase 3.6 Task 2).
 pub(crate) type DialogCheckFinishCb =
     Box<dyn FnMut(&DialogResult, &mut DlgPanel, &mut crate::emEngineCtx::EngineCtx<'_>) -> bool>;
```

The replacement prose preserves the explanatory content without the `DIVERGED:` tag — this is "rewrite as plain prose" per CLAUDE.md "Annotation Vocabulary".

### Item 6 — `crates/emcore/src/emDialog.rs:523`

**Action:** Remove the `DIVERGED:` block (lines 523–541, leading to `pub(crate) on_cycle_ext: Option<DialogCycleExt>,`). Replace with plain prose explaining the callback's role.

**Diff sketch:**

```diff
-    /// DIVERGED: (language-forced) Rust mechanism for the C++ "emFileDialog::Cycle calls
-    /// emDialog::Cycle() first then runs its own logic" inheritance pattern
-    /// (emFileDialog.cpp:82). In C++, `emFileDialog` is a subclass and its
-    /// `Cycle()` override calls `emDialog::Cycle()` then continues. In Rust
-    /// there is a single engine type (`DialogPrivateEngine`) with no
-    /// inheritance; this callback slot lets emFileDialog inject post-base
-    /// Cycle logic without needing a separate engine or vtable dispatch.
+    /// Post-base-Cycle extension hook. `DialogPrivateEngine::Cycle` runs the
+    /// base body (close-signal observation, pending_result resolution,
+    /// auto-delete countdown), then invokes this callback. Used by
+    /// emFileDialog (Phase 3.6 Task 2) to layer file-dialog-specific Cycle
+    /// logic over the base — the Rust analogue of C++'s
+    /// `emFileDialog::Cycle()` calling `emDialog::Cycle()` first then
+    /// continuing (emFileDialog.cpp:82).
     ///
     /// `DialogPrivateEngine::Cycle` calls this AFTER the base cycle body
```

The C++ reference and ordering contract are preserved as plain prose; the `DIVERGED:` framing — which would falsely claim language-forced — drops.

### Item 7 — `crates/emcore/src/emFileDialog.rs:68`

**Action:** Remove the `DIVERGED:` sentence-fragment from the doc-comment (lines 67–72). Replace with a plain prose explanation that this is a test-only cache.

**Diff sketch:**

```diff
     /// Cached `fsb.file_trigger_signal` — `SignalId` is `Copy`, stable
-    /// across fsb lifetime. Used by the `file_trigger_signal()` test
-    /// accessor. DIVERGED: (language-forced) C++ `emFileDialog` does not expose a
-    /// `GetFileTriggerSignal` accessor; this cache exists solely because
-    /// Rust tests need to fire the signal externally without walking the
-    /// tree. The closure in `on_cycle_ext` captures the same SignalId via
-    /// move-capture at construction and does NOT read this field.
+    /// across fsb lifetime. Test-only cache: backs the
+    /// `file_trigger_signal()` accessor used by Rust unit tests to fire the
+    /// signal externally without walking the tree. The closure in
+    /// `on_cycle_ext` captures the same SignalId via move-capture at
+    /// construction and does NOT read this field; production code paths
+    /// never touch it. C++ `emFileDialog` has no equivalent accessor
+    /// because C++ tests reach the signal through different mechanics.
     fsb_trigger_sig: SignalId,
```

### Item 8 — `crates/emcore/src/emFileDialog.rs:140`

**Action:** Remove the `DIVERGED:` block (lines 140–149). Keep the comment context above (lines 125–138, the C++-equivalent pseudo-code) — it's load-bearing port commentary. Replace the `DIVERGED:` block with plain prose describing the re-wake mechanic.

**Diff sketch:**

```diff
-                // DIVERGED: (language-forced) Rust-specific re-wake. The base
-                // `DialogPrivateEngine::Cycle` body runs BEFORE on_cycle_ext
-                // (Phase-3.6 Task 2 ordering). State mutations this closure
-                // makes — setting `dlg.pending_result`, pushing
-                // `pending_actions` — are not visible to the base body this
-                // Cycle. We return `true` on any mutation to keep the engine
-                // awake so the next Cycle's base body observes them. C++
-                // doesn't need this: `Finish(POSITIVE)` in
-                // emFileDialog::Cycle finalizes via same-call-stack re-entry
-                // into emDialog::Cycle's finalize path.
+                // Re-wake protocol: the base `DialogPrivateEngine::Cycle` body
+                // runs BEFORE on_cycle_ext (Phase-3.6 Task 2 ordering), so
+                // state mutations this closure makes (`dlg.pending_result`,
+                // `pending_actions`) are not visible to the base body this
+                // Cycle. Return `true` on any mutation to keep the engine
+                // awake so the next Cycle's base body observes them. C++'s
+                // same-call-stack `Finish(POSITIVE)` re-entry achieves the
+                // same finalize ordering directly.
                 let mut stay = false;
```

### Item 9 — `crates/emcore/src/emFileModel.rs:490`

**Action:** Replace the category tag in the `DIVERGED:` block. Per CSV, `upstream-gap-forced` → `language-forced`. The justification text needs a one-line touch-up to match the new category — the underlying issue is callback-signature incompatibility (language-forced), not a missing upstream port (upstream-gap-forced).

**Diff sketch:**

```diff
-        // DIVERGED: (upstream-gap-forced) C++ Cycle calls StartPSAgent and
-        // UpdateMemoryLimit before the loop. PSAgent integration is
-        // deferred from F017 scope (Rust PriSchedModel callback signature
-        // is incompatible with C++ GotAccess→WakeUp; tracked separately).
-        // UpdateMemoryLimit signals memory pressure that no panel
-        // currently reads in the Rust port.
+        // DIVERGED: (language-forced) C++ Cycle calls StartPSAgent and
+        // UpdateMemoryLimit before the loop. The Rust `PriSchedModel`
+        // callback signature cannot admit the C++ `GotAccess → WakeUp`
+        // shape (the C++ pattern requires a member-function pointer + this
+        // pair that Rust closures don't express without trait-object
+        // wrapping that defeats the wake-up contract). PSAgent integration
+        // therefore deferred from F017 scope; tracked separately.
+        // UpdateMemoryLimit signals memory pressure that no panel
+        // currently reads in the Rust port.
```

**Note on D-001 cross-reference:** The bucket file (`B-019-stale-annotations.md` line 7, line 26, line 35) cites "interacts with D-001." D-001 governs `u64`/`SignalId` accessor type-mismatches in emfileman; it has no bearing on emFileModel PSAgent integration. The cross-reference appears to be a clustering artefact and should be **dropped** from the bucket file when this design lands. **Flag for working-memory session:** correct the bucket file's D-001 citation.

## Verification

After landing the diffs:

1. `cargo check --workspace` — annotation comments are doc-comments / line comments, so no semantic change; check should pass without diff in compilation behavior.
2. `cargo clippy --workspace -- -D warnings` — no new warnings expected (comments don't influence clippy).
3. `cargo xtask annotations` — must pass. The existing category-required check validates that every remaining `DIVERGED:` carries one of the four chartered categories. Item 9's category swap must validate as `language-forced`. Items 1–8's removals reduce the `DIVERGED:` count; the lint has nothing to assert there.
4. `cargo-nextest ntr` — full test suite. No behavior change; should be green.
5. Spot-check via grep that no `DIVERGED:` block referencing the rewritten lines remains:
   ```
   rg -n 'DIVERGED:' crates/emfileman/src/emDirPanel.rs crates/emmain/src/emMainControlPanel.rs crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs
   ```
   Expected: zero hits in those files for the cited line ranges. (Other DIVERGED blocks at other line ranges in those files may remain; only the cited blocks are in scope.)
6. `cargo xtask annotations` rerun confirms `emFileModel.rs:490` block parses with `language-forced`.

## Sequencing

The 9 items are independent at the file-edit level (they touch 5 distinct files; no two items touch the same line range). Three sequencing options:

**Option A — single PR.** All 9 edits land together. Mechanical, easy to review (≈40 lines deleted, ≈30 lines added in prose replacements, ≈4 lines edited). Recommended.

**Option B — split per file.** 5 PRs, one per file. Adds review overhead with no benefit.

**Option C — split mask-drift items vs below-surface items.** PR1 = items 5–9 (no cross-bucket coupling); PR2 = items 1–4 (mask-drift, coordinate with B-012/B-016). Allows the below-surface cleanup to land immediately while the mask-drift cleanup blocks on the receiving buckets if conservative ordering is preferred.

**Recommended: Option A.** The mask-drift items (1–4) do not break anything if they land before B-012/B-016 — the underlying drift remains until those buckets land, but it remained anyway and was merely camouflaged by the now-removed annotations. Removing the camouflage early is actively useful: subsequent re-audits see the drift cleanly. Cross-bucket coordination is informational (notify B-012 / B-016 designers of the hand-off), not blocking.

**Cross-bucket coordination tasks (working-memory session owns):**
- Update `B-012-rc-shim-mainctrl.md` open-questions section: note the two-hop relay through `emMainWindow.to_reload` surfaced by item 4.
- Update `B-016-polling-no-acc-emfileman.md` open-questions section: note that the structural justification at `emDirPanel.rs:117` is being removed, so the bucket should not preserve any "we're keeping the polling because emDirModel doesn't implement FileModelState" framing.
- Update `B-019-stale-annotations.md` to drop the D-001 cross-reference (item 9 analysis showed D-001 doesn't apply).

## Open Questions for Implementer

1. For items 5, 6, 7, 8: the design proposes replacing the `DIVERGED:` block with plain prose preserving the explanatory content. Implementer can choose to delete outright if the prose is judged not load-bearing. Both are conformant; preference is preserve-as-prose because the C++ cross-references and ordering contracts are useful to future readers.
2. Item 9's rewritten justification text is a sketch — implementer may tighten the wording, but must preserve the four-question forced-category claim that the callback signature shape is the language-forced constraint, not the deferred upstream port.
3. Should item 9 wait for the B-016 / B-002 bucket landings touching emFileModel? **No.** The annotation is at `emFileModel.rs:490` (PSAgent block); B-007 already addresses `emFileModel-103` at `emFileModel.rs:483` (`AcquireUpdateSignalModel` semantic mis-port) — different lines, different concerns. No collision.
4. Does this bucket need to update `cargo xtask annotations` to detect future wrong-category claims? **No.** The existing category-required check is sufficient post-cleanup. Detecting *wrong* categories (vs *missing*) requires either re-running the four-question test programmatically (out of scope; the test is judgemental) or a manual re-validation pass like Task 6 of the audit. Either way, not lint material.
5. Anything in `preexisting-diverged.csv` outside the `signal_related == 'true'` filter? Out of scope per the bucket charter, but the implementer may notice adjacent rows; route any new findings back to the working-memory session.

## Coverage gaps and new decisions

- **No new D-### proposals.** The existing decisions cover all situations; D-001 was the only candidate citation and turned out not to apply (item 9 analysis).
- **No coverage gaps.** Every mask-drift item maps cleanly to an existing B-001..B-018 bucket (B-012 owns rows 2/3/4; B-016 owns row 1).
- **One bucket-file correction:** drop the D-001 reference from `B-019-stale-annotations.md` (line 7 "Cited decisions" header, line 26 row note, line 35 open-question). Coverage-wise this is bookkeeping, not a gap.

## Reconciliation summary for working-memory session

- 4 cross-bucket masked-drift edges to register: row 1 → B-016, rows 2/3/4 → B-012.
- 1 two-hop relay note for B-012 (item 4: `mw.to_reload` second hop).
- 1 framing note for B-016 (item 1: structural justification removed).
- 1 self-correction for B-019 bucket file (drop D-001 citation; it doesn't apply to item 9).
- 0 new D-### entries needed.
- 0 coverage gaps.
