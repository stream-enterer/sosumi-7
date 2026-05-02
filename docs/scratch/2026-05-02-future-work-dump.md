# Future-work scratch dump — 2026-05-02

Scratch document. Catch-all for everything that surfaced during Tier-B
remediation, the FU-001 brainstorm, and adjacent context. Not curated;
not classified. Goal: nothing falls through.

This is **not** a bucket plan and **not** a roadmap. It's a memory aid.
When something here gets formally bucketed/spec'd/closed, leave the entry
and add a `→ closed by …` pointer rather than deleting it (so we can see
what we caught).

---

## A. Formally bucketed (have files / specs)

| Bucket | Status | File |
|---|---|---|
| FU-001 — emstocks reaction-body completion + emCheckBox click_signal mirror | Spec written 2026-05-02 (4a591a0a). Not implemented. | `docs/superpowers/specs/2026-05-02-FU-001-emstocks-reaction-bodies-design.md` |
| FU-002 — App-bound reaction wiring (mainctrl) | Bucket file only. **Brainstorm pending.** Architectural decision required first (App-threading model: thread `&mut App`, pending-action queue, or `EngineCtx::app()` registry). | `…/followups/FU-002-app-bound-reactions.md` |
| FU-003 — emView multi-view content/control split port | Bucket file only. **Brainstorm pending.** Large standalone upstream port, not a bucket-style sweep. | `…/followups/FU-003-emview-multiview-port.md` |
| FU-004 — D-009 polling-intermediary sweep | Bucket file only. **Brainstorm pending.** Discovery-led; first phase is enumeration. | `…/followups/FU-004-d009-polling-sweep.md` |
| FU-005 — `emFileModel` file-state-signal conflation fix | **Bucket file not yet written.** Created during FU-001 brainstorm when scan revealed `change_signal` is allocated but never fired and `GetFileStateSignal()` returns it (conflated with what should be a distinct ChangeSignal). 15 C++ fire sites; ~10 Rust state-mutation sites need D-007 ectx-threading audit. Cross-crate consumer audit needed. | (to be written) |

## B. Surfaced during FU-001 brainstorm — beyond FU-001's scope

- **emCheckBox embed-vs-mirror redesign.** Considered and rejected for FU-001 (would override B-012's deliberate mirror-sibling-port pattern, expand emCheckButton's API). Open question whether the widget chain (emButton → emCheckButton → emCheckBox; also emRadioBox, etc.) should be revisited at the hierarchy level. If yes: dedicated brainstorm. If no: document the mirror pattern as a project decision so future contributors don't re-litigate.
- **Mirror-sibling-port pattern is partially undocumented.** B-012 codified it in code comments but there is no decision-catalog entry or design-doc that names it as the canonical Rust idiom for C++ public IS-A. Recommend a `D-###` decision-catalog entry promoting the pattern, so it's discoverable without grepping for the comment block.
- **emCheckButton `DIVERGED: (language-forced)` annotation on `click_signal`** is defensible but the same logic could equally cite "preserved design intent." Annotation-vocabulary question: when a divergence is forced *and also* preserves design intent, which category wins for the lint? Worth a decision-catalog entry.
- **emCheckBox `DIVERGED: (language-forced)` on `check_signal`** uses the same justification; the mis-classification scrutiny in this session concluded it's not strictly language-forced (composition was untried). The annotation lint passed it because category-presence is checked, not category-accuracy. **Annotation-accuracy sweep** would surface this and others.
- **`emStocksFetchPricesDialog` ctor doesn't take a view.** C++ takes `GetView()`; Rust ctor takes only the model. Means `Raise()` is moot, focus/keyboard delivery may diverge, dialog isn't parented under a window in the panel tree. Affects FU-001 implementation (Raise stub) and any other dialog construction.
- **`emDialog::ShowMessage`** missing in Rust emcore. ShowWebPages error path is logged-and-swallowed in FU-001. emfileman / emstocks likely have other error-message-display sites with similar workarounds.
- **`widget_checkbox_toggle.widget_state.golden`** may need regeneration during FU-001 Phase 1 (depending on what state it captured). Establish convention: when adding a new signal field that didn't exist before, do golden tests need explicit "before/after" rationale in the commit message? Probably yes.

## C. Tier-B residuals tracked elsewhere — risk of being forgotten

- **B-003 follow-ups** at `emAutoplayControlPanel.rs:715` and `:727`. Need full port of C++ `UpdateControls` and `UpdateProgress`. Subscriptions wired; reaction bodies are stubs.
- **B-012 follow-ups** at `emMainControlPanel.rs:562, 570, 610` (Duplicate / Fullscreen / Quit). Subscriptions wired; reaction bodies stubbed pending App-threading decision (FU-002).
- **emRecFileModel.rs:366** explicitly returns `SignalId::default()` for `GetFileStateSignal` with the comment "consumers that need it subscribe through the wrapping `emFileLinkModel`/`emStocksFileModel` if those expose one." This was the workaround pre-FU-005; the conflation fix should retire this.
- **`emStocksFileModel.rs:149` UPSTREAM-GAP delegate** for `GetFileStateSignal`. Closed by FU-005.
- **`emStocksPricesFetcher.rs:78` and `:425` UPSTREAM-GAP comments.** Closed by FU-005.
- **B-004 row 1479** `DIVERGED: (upstream-gap-forced)` at `emBookmarks.rs:724` — single-view navigation only. Closed by FU-003.
- **emCoreConfigPanel reset-closure D-009 polling sighting** noted in B-010 design but not bucketed. Closed by FU-004.
- **B-007/B-008/B-006/B-013 audit-quality false gap-blocked tags** — each design doc has an "Audit-data corrections" section. The procedural lesson (verify accessor existence before classifying as gap-blocked) was not promoted to the methodology doc. **Recommend updating `methodology.md` with this verification step** before the next audit.

## D. UPSTREAM-GAP markers — full inventory (cross-check periodically)

Files containing `UPSTREAM-GAP`:
- `crates/emcore/src/emViewPort.rs` — what gap? (not investigated this session)
- `crates/emcore/src/emTreeDump.rs` — what gap? (not investigated this session)
- `crates/emstocks/src/emStocksFileModel.rs:149` — closed by FU-005.
- `crates/emstocks/src/emStocksPricesFetcher.rs:78,425` — closed by FU-005.

Action: spend one pass reading the 4 marker locations to confirm understanding. Two emcore markers are unknown to current planning.

## E. Annotation / port-fidelity hygiene

- **278 `DIVERGED:` entries** across the codebase. The lint validates each carries a category tag; it does NOT validate the category is correct. Sweep: read every DIVERGED, confirm the cited forced-divergence category is real (the four: language-forced, dependency-forced, upstream-gap-forced, performance-forced). Likely turns up several "this is actually preserved design intent" or "this isn't actually forced" mis-classifications. Gradient task; don't need to do all at once.
- **`RUST_ONLY:` markers** also need similar accuracy review. (Not counted this session.)
- **`SPLIT:` markers** for files split due to "one primary type per file" — verify each is still load-bearing as the codebase evolves.
- **`UPSTREAM-GAP:` markers** as listed above; confirm each is still a gap (some may have been closed by intervening work).
- **The DIVERGED-vs-preserved-design-intent boundary** is currently a judgment call in CLAUDE.md ("When unsure whether a difference is forced or design intent: assume design intent, match C++ exactly, and mark the point of departure explicitly"). The annotation vocabulary doesn't have a way to mark "I deliberately preserved design intent" — only forced divergence and Rust-only. Recommend a `PRESERVED-INTENT:` annotation? or keep the convention "no annotation = matches C++" and let absence speak. Open question.

## F. Workflow / process observations from this session

- **FU-002 lesson — bucket-file "architectural decision" framing.** FU-002's bucket file said "first phase: architectural decision (a/b/c)." Research showed all three options were preempted by an existing pervasive pattern (`pending_actions` queue). Future bucket files should frame architectural-decision phases as **"verify whether an existing pattern applies"** before listing fresh options. Saves a spec round.

- **"Verify before recommending" is the de facto rule now.** I escalated to (b) embed-not-mirror without checking emCheckButton's existing structure — the verification flipped the recommendation. Worth memorializing: when a design choice could go either way, prefer the path that's already established in the codebase, and *check what's already established* before recommending the alternative.
- **Adversarial review pass before SDD dispatch** was a workflow innovation during Tier-B — caught real issues in design docs before implementer time was spent. Should be promoted into the SDD skill or at least the project methodology.
- **Per-task nextest skipping in SDD loops** is now the codified pattern (memory entry exists). Pre-commit hook is the per-commit gate; full nextest at the end of phase.
- **The "sequential brainstorms" pattern** for follow-up buckets (this session) — small, focused, one-at-a-time. Working well; preserve.
- **Memory entries should be reviewed periodically.** Some are time-stamped (F010 status as of 2026-04-25, F018 status as of 2026-04-26, etc.) and may be stale. Quick reread + update or remove.

## G. Pre-existing project state (from memory + recent commits)

- **F010 cluster: 13/19 merged.** Baseline 2897. Emcore track complete at 05d95e16 (zoom-in CTD fixed). **6 app buckets deferred** — not enumerated this session; check the F010 docs.
- **F018 Phase 3 deferrals:** Task 1 done at d6091a8a (4 widgets); 6 widget tasks + Layer 2 + Phase B + Phase C remain.
- **nextest baseline skips** (`.config/nextest.toml`): grew by 2 plugin_invocation tests in F010 Task 2.1 (commit 1a3f2f62, f010-investigation branch). Root cause: unbuilt test_plugin cdylib. Either build the cdylib or remove the skips.
- **Plugin cdylib ABI trap** documented at `docs/debug/investigations/plugin-cdylib-abi-trap.md`. The diagnostic pattern (when a `Box<dyn Trait>`-backed call returns garbage, check `.so` timestamp vs source) should be promoted to a debugging-guide entry if not already.
- **emtest-panel open items** at `docs/emtest-panel-open-items.md` — not reviewed this session; may overlap with F018.

## H. Audit-quality observations from Tier-B

- **Heuristic-misclassified rows (4 instances).** B-006/B-007/B-008/B-013 each had rows tagged "gap-blocked" by the audit's automated heuristic, but the accessor actually existed (inherited or composed). Each design doc has a corrections section. Pattern: the heuristic missed inheritance/composition. **Update methodology before next audit.**
- **Cross-bucket prerequisite resolution** (B-005 → B-009, B-017 row 1 ← B-001 G3) all resolved by ordering, but the inventory-enriched edges weren't formally tracked between buckets. Recommend a small machine-readable cross-ref in `inventory-enriched.json` (already partially present) for future audits.
- **Tier-B status final entry** in work-order.md says "19/19 buckets resolved" — but it's worth one more audit pass to confirm no row was silently dropped.

## I. Widget-hierarchy / cross-cutting questions

- **Mirror-sibling-port pattern** applied at: emButton, emCheckButton (now), emCheckBox (FU-001 will add), emRadioBox (?), emTextField (?). **Sweep needed** to find every C++-inherited accessor that didn't get mirrored. Likely candidates: any signal accessor on a C++ base class that derived classes inherit. The B-012 codification is recent; missed sites are likely.
- **B-001-followup's `subscribed_init` + `subscribed_widgets` two-tier init** was a local pattern in emstocks but B-001 noted "If a second bucket rediscovers, promote to D-###." Not yet promoted; not yet rediscovered (as far as I can tell). Watch for it.
- **Panel-as-proxy-engine pattern** (B-017 row 1, B-001-followup Phase E) — when a panel allocates signal/timer for an embedded model that can't self-register. Used twice now; may warrant a D-### entry.
- **`DropOnlySignalCtx`** is the chartered single exception for Rust-Drop language-forced sites. Currently 1 use site. Watch for misuse / scope creep.

## J. Other items mentioned in passing

- **emProcess error path silently logged.** FU-001 ShowWebPages will add this; consider sweeping other emProcess call sites for the same pattern, especially anywhere a missing-config or process-spawn-failure was previously a panic.
- **`#[cfg(any(test, feature = "test-support"))]` test-only setters** (e.g., `set_checked_for_test`) — there are several. Sweep to confirm they're still needed and not load-bearing in production.
- **Inventory.md** at `docs/debug/audits/2026-04-27-signal-drift-tier-b/inventory.md` — verify all 212 rows are now visibly closed (or have a successor pointer).
- **`docs/debug/marker-audit-summary.md`** exists; not reviewed this session. May overlap with annotation-hygiene work.
- **`docs/CORRESPONDENCE.md`** and `docs/VERIFICATION.md` — not reviewed; should confirm these are current and not stale references.

## K. "Decisions waiting to be made"

- **App-threading model** for FU-002: which of the three options (`&mut App` in ctx / pending-action queue / Rc<RefCell<App>> registry) is canonical? Needs decision-catalog entry once chosen.
- **Annotation vocabulary** for "preserved design intent": current convention says don't annotate, but this session showed that's a missed opportunity for `DIVERGED:` reviewers (no signal that "this looks like a divergence but is intentional C++-mirroring"). Open.
- **Mirror vs embed for widget hierarchy:** mirror codified de facto by B-012 + FU-001, but never formally chosen. Promote to `D-###` to settle.
- **Cross-cutting `D-###` for panel-as-proxy-engine pattern** if the third occurrence shows up.

---

## Heuristics for using this list

- When starting a new bucket / brainstorm, scan section B and C for items that legitimately fold in.
- When closing a bucket, sweep this file for entries that the closure resolves; mark them `→ closed by …`.
- Periodically (monthly?) reread the whole file. Promote anything that's grown teeth into a real bucket; delete anything that's been resolved-and-recorded elsewhere.
- New surfaced items go in the natural section, or in K if it's a decision-shaped thing rather than a work-shaped thing.

## Source / origin notes

This dump was compiled at the close of the FU-001 brainstorm on 2026-05-02. It mixes:
- Items from the FU-001 design conversation that surfaced beyond FU-001's scope.
- The post-Tier-B residual audit (`docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/work-order.md` and design-doc Amendment Logs).
- Pre-existing memory entries about other ongoing efforts (F010, F018, plugin cdylib).
- One pass over `UPSTREAM-GAP` / `DIVERGED:` / `RUST_ONLY:` markers in the source.

Not exhaustive. Add to this file rather than starting a parallel one.
