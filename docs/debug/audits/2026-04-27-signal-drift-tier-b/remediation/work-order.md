# Signal-Drift Remediation — Work Order

**Generated:** 2026-04-27 from Phase 4 of the bookkeeping strategy.
**Total buckets:** 19
**Layers:** 1 (no cross-bucket prereqs — all 11 prereq edges in `inventory-enriched.json` are intra-B-009 consumer→accessor)

Buckets are ordered by topological layer over the prereq DAG (lower layer = no unmet prereqs). With a single layer, ordering reduces to mechanical-heavy first, then balanced, then judgement-heavy — mechanical work validates the underlying patterns cheaply before committing to judgement-laden buckets.

## Order

| # | Bucket | Layer | Mechanical-vs-judgement | Rows | Status | Design doc |
|---|---|---|---|---|---|---|
| 1 | B-005-typed-subscribe-emfileman | 0 | mechanical-heavy | 21 | merged at 91433733 (d15bbca0..91433733) | [d95d55a7](../../../../superpowers/specs/2026-04-27-B-005-typed-subscribe-emfileman-design.md) |
| 2 | B-006-typed-subscribe-mainctrl | 0 | mechanical-heavy | 3 | merged at f37adf01 (5963c688..f37adf01) | [a13880c7](../../../../superpowers/specs/2026-04-27-B-006-typed-subscribe-mainctrl-design.md) |
| 3 | B-007-typed-subscribe-emcore | 0 | mechanical-heavy | 3 | merged at 55d735bc (858524f1..55d735bc; includes the AcquireUpdateSignalModel gap-fix) | [8b220ebb](../../../../superpowers/specs/2026-04-27-B-007-typed-subscribe-emcore-design.md) |
| 4 | B-008-typed-subscribe-misc | 0 | mechanical-heavy | 3 | merged at 133de22e (c68360ef..133de22e) | [4c4141f1](../../../../superpowers/specs/2026-04-27-B-008-typed-subscribe-misc-design.md) |
| 5 | B-015-polling-emcore-plus | 0 | mechanical-heavy | 10 | designed | [b521b3f6](../../../../superpowers/specs/2026-04-27-B-015-polling-emcore-plus-design.md) |
| 6 | B-019-stale-annotations | 0 | mechanical-heavy | 9 | merged at 41599129 | [e7129430](../../../../superpowers/specs/2026-04-27-B-019-stale-annotations-design.md) |
| 7 | B-001-no-wire-emstocks | 0 | balanced | 71 | designed | [456fa5f7](../../../../superpowers/specs/2026-04-27-B-001-no-wire-emstocks-design.md) |
| 8 | B-002-no-wire-emfileman | 0 | balanced | 4 | designed | [7fb3decd](../../../../superpowers/specs/2026-04-27-B-002-no-wire-emfileman-design.md) |
| 9 | B-003-no-wire-autoplay | 0 | balanced | 3 | merged at eb9427db (12d3b4fe + 2ac6a627 + eb9427db) | [703fa462](../../../../superpowers/specs/2026-04-27-B-003-no-wire-autoplay-design.md) |
| 10 | B-004-no-wire-misc | 0 | balanced | 4 | designed | [3497069d](../../../../superpowers/specs/2026-04-27-B-004-no-wire-misc-design.md) |
| 11 | B-016-polling-no-acc-emfileman | 0 | balanced | 3 | designed | [d837346b](../../../../superpowers/specs/2026-04-27-B-016-polling-no-acc-emfileman-design.md) |
| 12 | B-017-polling-no-acc-emstocks | 0 | balanced | 3 | designed | [a27d2faa](../../../../superpowers/specs/2026-04-27-B-017-polling-no-acc-emstocks-design.md) |
| 13 | B-009-typemismatch-emfileman | 0 | judgement-heavy | 14 | merged at 50994e26 (3b56e00b..50994e26) | [0a7d7fd3](../../../../superpowers/specs/2026-04-27-B-009-typemismatch-emfileman-design.md) |
| 14 | B-010-rc-shim-emcore | 0 | judgement-heavy | 15 | designed | [09f08710](../../../../superpowers/specs/2026-04-27-B-010-rc-shim-emcore-design.md) |
| 15 | B-011-rc-shim-autoplay | 0 | judgement-heavy | 7 | merged at eb9427db (absorbed into B-003) | [cf9e1cc4](../../../../superpowers/specs/2026-04-27-B-011-rc-shim-autoplay-design.md) |
| 16 | B-012-rc-shim-mainctrl | 0 | judgement-heavy | 7 | designed | [bf6e9bd5](../../../../superpowers/specs/2026-04-27-B-012-rc-shim-mainctrl-design.md) |
| 17 | B-013-dialog-cells-emstocks | 0 | judgement-heavy | 4 | designed | [ec317565](../../../../superpowers/specs/2026-04-27-B-013-dialog-cells-emstocks-design.md) |
| 18 | B-014-rc-shim-no-acc-misc | 0 | judgement-heavy | 2 | merged at c2871547 (c89db09b..c2871547) | [d7d964d4](../../../../superpowers/specs/2026-04-27-B-014-rc-shim-no-acc-misc-design.md) |
| 19 | B-018-fileDialog-singleton | 0 | judgement-heavy | 1 | merged (false positive — no implementation; reclassification at 683153f1) | [04059bac](../../../../superpowers/specs/2026-04-27-B-018-fileDialog-singleton-design.md) |

Total rows: 187 (178 actionable + 9 cleanup).

## Status legend

- `pending` — not yet picked up.
- `in-design` — a fan-out brainstorm session is currently working on this bucket.
- `designed` — design doc returned and reconciled into the spine.
- `merged` — implementation merged to `main`.

## Reconciliation log

### 2026-04-27 — B-005 design returned (d95d55a7)

- **D-006-subscribe-shape** added to `decisions.md` (resolved per fan-out's recommendation: first-Cycle init + IsSignaled top-of-Cycle, with deferred-queue B as documented fallback).
- **D-005** amended with a "see also D-006" pointer (D-005 picks reaction model, D-006 picks wiring shape; complementary).
- **Cross-bucket prereq surfaced:** B-005 → B-009. Two `emFileManControlPanel` rows in B-005 require B-009's accessor flip (D-001) before their Cycle-init connects can land. Remaining 19 B-005 rows are independent. Documented in B-005's `Prereq buckets:` line; not yet propagated to `inventory-enriched.json` row-level prereq_ids (the dependency is design-level, not row-level — a `B-005 cannot fully merge until B-009 merges` constraint, not a row-pair edge). Topo order in this table is unchanged because B-005 is already designed; merge order will sequence B-009 ahead of B-005's blocked rows when the time comes.
- **Stale prose** in B-005's bucket sketch was already cleaned up in 83eb06d3 before the design returned.
- **B-005 status:** in-design → designed.

### 2026-04-27 — B-006 design returned (a13880c7)

- **No new D-### entries** — D-006 covered the wiring shape verbatim.
- **Audit-data correction:** `emMainControlPanel-218` reclassified `gap-blocked → drifted`; `D-003` citation dropped. `GetWindowFlagsSignal` exists at `crates/emcore/src/emWindow.rs:1279`; audit-time tag was stale. D-003's "Affects" count amended from 16 → 15.
- **New row state — `resolved_by`:** `emMainControlPanel-217` annotated with `resolved_by` pointing to `crates/emmain/src/emMainWindow.rs:825` (`ControlPanelBridge`); the drift at the row's site is observably absorbed by an existing dependency-forced divergence at a different site. Row stays in bucket; design treats as no-action. Future buckets that surface similar drift-here-resolved-there cases use the same `resolved_by` field on the row.
- **Soft cross-bucket edge:** B-006 → B-012-rc-shim-mainctrl. Non-blocking. The 7 `BtNewWindow..BtQuit` click-flag polls in `emMainControlPanel.Cycle` will become D-006-shaped `IsSignaled` branches when B-012 lands. B-006 is observable-correct without it.
- **Implementation note from designer:** three widget handles (`bt_fullscreen`, `bt_auto_hide_control_view`, `bt_auto_hide_slider`) need to be hoisted from `LMainPanel`-local to `emMainControlPanel` fields as Step 1 of the implementation. Mechanical refactor; in-scope per the design doc.
- **B-006 status:** pending → designed.

### 2026-04-27 — B-007 design returned (8b220ebb)

- **No new D-### entries** — D-006 covered wiring shape.
- **Audit-data correction:** `emFileSelectionBox-64` reclassified `gap-blocked → drifted`; `D-003` dropped. Shared `FileModelsUpdateSignal` is actually ported as `App::file_update_signal` at `crates/emcore/src/emGUIFramework.rs:227`. D-003 affects count: 15 → 14.
- **Latent semantic mis-port surfaced:** `emFileModel::AcquireUpdateSignalModel` (`emFileModel.rs:343`) returned a dead per-model signal instead of the shared broadcast. B-007 design fixes inline as a bug (not annotated DIVERGED — Port Ideology says fidelity bugs are fixed, not annotated). Recorded on `emFileModel-103`'s row as a `reconciliation.note`.
- **Anchor-vs-implementation site mismatch:** `emImageFile-139`'s actual fix site is the SPLIT panel file `emImageFileImageFilePanel.rs`, not the audit anchor `emImageFile.rs:85`. Per-row design doc has the right site; bookkeeping note only.
- **No cross-bucket prereqs.** Designer noted `LoaderEngine` persistent-after-load in step 2 is a precedent that future `emFileModel`-derived ports (e.g., `emRecFileModel`) will reuse — track as a downstream pattern, not a prereq edge.
- **B-007 status:** pending → designed.

### 2026-04-27 — B-008 design returned (4c4141f1)

- **No new D-### entries.**
- **Audit-data correction:** `emMainPanel-69` reclassified `gap-blocked → drifted`; `D-003` dropped. Same `GetWindowFlagsSignal` stale tag pattern as B-006/`emMainControlPanel-218`. D-003 affects count: 14 → 13 (now 0 P-002 gap-blocked remaining).
- **Pattern observation:** every gap-blocked → drifted reclassification across B-006/B-007/B-008 has been a P-002 row whose accessor existed at audit time but was tagged missing. Now zero P-002 rows remain gap-blocked. Remaining 13 gap-blocked rows are all P-001/P-003. Worth noting in case the reclassification pattern continues into P-001 buckets (B-001..B-004) and the D-003 affects count keeps shrinking.
- **First hard cross-bucket prereq edge in inventory-enriched.json:** `emVirtualCosmos-104` (B-008) → `emFileModel-103` (B-007). B-008's `Prereq buckets:` line records the bucket-level edge.
- **Designer noted minor adjacency:** C++ `emMainPanel.cpp:68` also subscribes to `SliderTimer.GetSignal()` which Rust polls — not in B-008's row set, separate P-006/P-007 concern, untouched.
- **B-008 status:** pending → designed.

### 2026-04-27 — B-015 design returned (b521b3f6)

- **No new D-### entries.** D-005 + D-006 covered everything. D-006's per-bucket override clause already accommodated the `emFilePanel::SetFileModel` subscribe-at-SetFileModel-time variant (signal identity changes on model swap).
- **D-005 open question struck:** individual subscribes per child for `emColorField::Cycle` confirmed against C++ source (8 separate `AddWakeUpSignal` calls in `emColorField::AutoExpand`).
- **No audit-data corrections.** All 10 rows had accurate accessor-present tags.
- **Cross-bucket prereq:** soft edge `emMainPanel-68` → `emMainPanel-67` (B-008) — shared `emMainPanel::Cycle` body and `subscribed_init` field. Encoded in `inventory-enriched.json`. B-015's `Prereq buckets:` line records bucket-level edge to B-008.
- **Implementer-facing structural change:** `emFilePanel::SetFileModel` signature gains `&mut SchedCtx + EngineId`. Caller migration is bounded; flagged as open question in design doc.
- **B-015 status:** pending → designed.

### 2026-04-27 — B-019 design returned (e7129430)

- **No new D-### entries.** Designer found D-001 does not govern `cleanup-emFileModel-490` (PSAgent callback-signature divergence, unrelated to the `u64`/`SignalId` accessor flip). Citation dropped from B-019 sketch.
- **Mask-drift mapping captured:** four cleanup items have downstream non-blocking edges to other buckets (3 → B-012, 1 → B-016). Mapping recorded in B-019's "Reconciliation amendments" block; forward-pointer notes added to B-012 and B-016 sketches so their future fan-outs see the context.
- **Two-hop relay surfaced for B-012's design:** `cleanup-emMainControlPanel-320` involves a `mw.to_reload` chain through `emMainWindow` → `MainWindowEngine` → `file_update_signal`. B-012's design must address the second hop, not just the click-handler shim. Captured in B-012's inbound notes.
- **Sequencing recommendation from designer:** land B-019 single-PR before B-012/B-016 to remove camouflage. Non-blocking either direction; preference only.
- **No coverage gaps** — every mask-drift item maps to an existing bucket.
- **B-019 status:** pending → designed.

### 2026-04-27 — B-001 design returned (456fa5f7)

- **No new D-### entries.** Designer flagged a candidate (AutoExpand-deferred widget-subscribe two-tier init) but did not promote on a single occurrence. If a second bucket rediscovers it, promote.
- **No cross-bucket prereqs** — P-001 in emstocks subscribes only to `SignalId`-typed accessors; no P-003 dependency.
- **Audit-data refinements within bucket scope** — no row moves:
  - `emStocksListBox-53` is shape-equivalent to P-002 (accessor inherited from `emListBox`); stays in B-001.
  - 20 `emStocksControlPanel` rows + `-626` carry an additional "missing widget instance" drift the audit didn't separately classify; widget-add absorbed into bucket scope.
  - `emStocksFileModel-accessor-model-change`: delegating accessor (one-liner), not a new SignalId allocation.
- **9 accessor groups** organize the design (G1..G9). Largest: G2 Config.GetChangeSignal (6 consumers), G1 FileModel.GetChangeSignal (4 consumers).
- **Coverage flag for working-memory:** G3 (`PricesFetcher.GetChangeSignal`) accessor ported per D-003 but has no in-bucket consumer. If C++ has an `AddWakeUpSignal(...PricesFetcher.GetChangeSignal())` site the audit missed, it's a B-001 amendment candidate. No action taken now.
- **Two-tier init pattern recorded** in B-001's reconciliation notes. Local-only; promotion candidate if rediscovered.
- **B-001 status:** pending → designed.

### 2026-04-27 — B-002 design returned (7fb3decd)

- **No new D-### entries.** `set_link_model`-driven re-subscribe (row -72) is a within-D-006 local variant; not promoted on single occurrence.
- **emRec-hierarchy concern disproved.** Standalone `emRecFileModel<T>` (does not wrap `emFileModel<T>`) — fix is local, no cross-bucket prereq.
- **2 accessor groups:** G1 emTimer (1 row), G2 emRecFileModel change-signal infra (3 rows). G2 has a mechanical ripple: every `emRecFileModel::new` caller takes one extra `SignalId` arg.
- **Outbound opportunity (downstream simplification, not prereq):** once B-002 lands G2, B-001's G1 (emStocksFileModel delegating accessor) can simplify to inherit through `emRecFileModel<T>`. Same potential for emAutoplay/emVirtualCosmos. Tracked here for forward reference; no spine edit until those buckets are designed.
- **Possible audit gap flagged:** emFileLinkPanel's C++ subscribes to `UpdateSignalModel->Sig`, `GetVirFileStateSignal()`, `Config->GetChangeSignal()` — not in B-002's row set. Verify whether B-005 covers them; if not, audit-coverage amendment needed.
- **B-002 status:** pending → designed.

### 2026-04-27 — B-003 design returned (703fa462)

- **D-002 deferred question §1 resolved.** Working-memory ratified **R-A: drop AutoplayFlags entirely.** Designer's investigation found the seven inbound `Cell`s are produced but never consumed; existing `DIVERGED` annotation at `emAutoplayControlPanel.rs:84` claiming "polled by parent panel" is factually wrong. R-A matches C++; outbound `progress` replaces with `Rc<RefCell<emAutoplayViewModel>>` + `GetItemProgress()` in `Paint`. D-002 amended in place.
- **Row renamed in inventory-enriched.json:** `emAutoplayViewModel-accessor-model-state` → `emAutoplayViewModel-accessor-progress`. C++ second signal is `ProgressSignal`, not a state signal. No prereq references; safe rename.
- **2 accessor groups:** G1 ChangeSignal (6 emit sites), G2 ProgressSignal (1 emit site). `emAutoplay-1171` Cycle fan-out covers 2 model subscribes + 7 widget subscribes; `emAutoplayControlPanel` gains a `Cycle` method.
- **No new D-### entries.** No cross-bucket prereqs.
- **B-003 status:** pending → designed.

### 2026-04-27 — B-004 design returned (3497069d)

- **No new D-### entries.** Mutator-fire ectx-threading flagged as promotion candidate if rediscovered (B-008 hit similar shape on `Acquire`; one more sighting and we promote).
- **No hard cross-bucket prereqs.** Two soft forward-edges noted: B-004 G1 → B-015 (once `GetVirFileStateSignal` lands, derived-panel polling has a subscribe target — already in B-015 scope, can stub); B-004 G3 ↔ B-008 (input vs output edges of emVirtualCosmos; independent).
- **emBookmarks-1479 verified actionable** (vs the 21 unported emBookmarks rows). The 21 unported rows reference editing panels the Rust port has BLOCKED as read-only; row 1479 is on the ported `emBookmarkButton`.
- **emVirtualCosmos B-004/B-008 distinction confirmed** (B-008 wires input, B-004 wires output of emVirtualCosmosModel).
- **3 accessor groups** (G1 emFilePanel.GetVirFileStateSignal, G2 emBookmarkButton.GetClickSignal, G3 emVirtualCosmosModel.GetChangeSignal).
- **No inventory-enriched.json patches needed.**
- **B-004 status:** pending → designed.

### 2026-04-27 — B-016 design returned (d837346b)

- **No new D-### entries.**
- **Hard cross-bucket prereq encoded** in `inventory-enriched.json`: all 3 B-016 rows now point at `emFilePanel-accessor-vir-file-state` (B-004's G1 accessor row). PR staging: B-004 G1 first, B-016 as follow-up.
- **B-019 framing strike refined:** original "emDirModel doesn't impl FileModelState" framing was false on its own merits (it does, at `emDirModel.rs:413`). Drift is plain missing-accessor, not structural.
- **No row reclassifications.** All 3 rows correctly tagged `accessor missing`; B-004 G1 fills the gap.
- **Out-of-scope subscribe sites noted** in the design — C++ ctors subscribe to additional signals not in B-016's row set; design's Cycle init block is shaped to absorb them in a future bucket.
- **B-016 status:** pending → designed.

### 2026-04-27 — B-017 design returned (a27d2faa)

- **No new D-### entries.**
- **2 hard cross-bucket prereqs encoded:** row 2 → B-004 G1 (`emFilePanel::GetVirFileStateSignal`), row 1 → B-001 G3 (`emStocksPricesFetcher::GetChangeSignal`). Row 3 stands alone (uses already-ported `Scheduler::create_timer`).
- **B-001 G3 reconciliation flag resolved.** Consumer found in B-017 row 1 — accessor port stays in B-001, consumer wiring stays in B-017. B-001 sketch updated.
- **Audit-data correction:** bucket sketch's "emTimer::TimerCentral unported" framing is stale; TimerCentral is ported at `crates/emcore/src/emTimer.rs` with active consumers. Strike from B-017 framing.
- **Recommended PR staging:** B-004 G1 + B-001 G3 first, B-017 follows. Row 3 is natural pilot if review pressure forces staging.
- **B-017 status:** pending → designed.

### 2026-04-27 — B-009 design returned (0a7d7fd3)

- **Two new D-### entries promoted** based on B-009 brainstorm (third sighting of the mutator-fire ectx-threading pattern; B-008 and B-004 cited as prior sightings):
  - **D-007-mutator-fire-shape**: thread `&mut EngineCtx<'_>` through mutators; `ectx.fire(sig)` synchronously, matching C++ `emSignal::Signal()`. No-op when `sig == SignalId::null()`.
  - **D-008-signal-allocation-shape**: lazy allocation via `Ensure*Signal(&self, ectx) -> SignalId` (Cell-backed). A2 (eager via Acquire scheduler-threading) rejected for the same friction D-006 cited; A3 (scheduler in emContext) deferred as a future framework lift.
- **Citations back-propagated** to B-008 and B-004 sketches; their candidate-if-rediscovered flags struck.
- **B-005 ↔ B-009 unblock confirmed.** B-005's design becomes implementable at B-009 merge; the `// see D-001` annotations in B-005's design doc become obsolete. Implementation order: B-009 first.
- **Per-accessor consumer migration plan** captured in B-009 sketch (which consumers fold under each of the 3 flipped accessors).
- **Helper APIs (EnsureSelectionSignal/EnsureCommandsSignal/EnsureChangeSignal) in-scope** per D-008. Mutator-callsite migration in-scope per D-007. Neither introduces new audit rows.
- **Audit-data correction:** `emFileManControlPanel-522`'s "sub-engine" routing claim is a misread — direct `AddWakeUpSignal` on the panel's own engine. B-009 sketch updated.
- **Watch-list note on D-008:** A3 (scheduler in emContext) candidate when placeholder-occupant count grows. Current occupants tracked in D-008 entry: `emFileLinkModel`, `emFileManTheme`, `emFileManConfig`, `emFileModel`.
- **B-009 status:** pending → designed.

### 2026-04-27 — B-013 design returned (ec317565)

- **Bucket sketch was prejudged wrong.** Original framing assumed D-002 rule-2 keep-shim; brainstorm verified all 4 C++ sites use canonical `AddWakeUpSignal(GetFinishSignal()) + IsSignaled + GetResult`. Rule 1 (convert), trigger side.
- **Audit-data correction (third accessor-status heuristic gap, after B-006/B-007/B-008):** all 4 rows reclassified `pattern_id P-005 → P-004` and `accessor_status missing → present`. `emDialog.finish_signal` exists as a public field; audit heuristic missed it.
- **D-002 affects count amended:** P-004 +4 (29→33), P-005 −4 (6→2).
- **Half-convert design (no DIVERGED on the residual cell):** subscribe via per-dialog first-Cycle init; `IsSignaled` is the trigger; the `Rc<Cell<Option<DialogResult>>>` + `set_on_finish` callback stay as a result-delivery buffer. Per Port Ideology, that's idiom adaptation, not divergence — cell is `pub(crate)` internal state below the user-visible surface and observable behavior matches C++.
- **Watch-list note (no decision):** emDialog's lack of sync post-show `GetResult` is an architectural gap that affects every dialog consumer (emfileman, emmain, emFileDialog). Future bucket may close via `App::inspect_dialog_by_id` + `emDialog::GetResult`; B-013 explicitly does not. Same shape as D-008's A3 watch-list.
- **No new D-### entries. No prereq edges.**
- **Pattern: third occurrence of audit's accessor-status heuristic missing inherited/composed accessors.** B-006 (`GetWindowFlagsSignal`), B-007 (`FileModelsUpdateSignal` via `App::file_update_signal`), B-008 (`GetWindowFlagsSignal` again), B-013 (`emDialog.finish_signal`). Now four occurrences. Pattern is established but not promoted to a decision — it's an audit-data-quality issue, not a design choice. Future buckets should explicitly verify accessor existence in their first step.
- **B-013 status:** pending → designed.

### 2026-04-27 — B-014 design returned (d7d964d4)

- **No new D-### entries.**
- **emVirtualCosmos-575 reclassified P-005 → P-001** (and `evidence_kind` rc_cell_shim → absent). Audit's rc_cell_shim heuristic misread the routine `model: Rc<RefCell<>>` handle as a click-handler shim. Same precedent as B-013's reclassification. Fix shape unchanged.
- **B-014 becomes a mixed-pattern bucket** (1 P-005 + 1 P-001). Title retained for stability; row set unchanged. Two distinct dispositions: R-A drop for emAutoplay-1172 (per B-003 precedent), rule-1 convert for emVirtualCosmos-575 (D-006 + D-007 + D-008).
- **D-007 benign hybrid composition documented.** B-014's `emVirtualCosmosModel::Reload` callsite is inside `Acquire` bootstrap and lacks ectx. The D-008 lazy-allocation null-fire-noop semantics make this benign: at Acquire no subscriber exists, so `change_signal == SignalId::null()` and the omitted fire would be a no-op anyway. Mutator keeps no-ectx signature with a `// CALLSITE-NOTE:` for future post-Acquire callers. **D-007 amended in place** with a "Composition note (post-B-014)" capturing this as the first benign hybrid.
- **D-002 affects amended:** P-005 6 → 1 (now just emAutoplay-1172). pattern-catalog.md: P-001 82 → 83; P-005 2 → 1.
- **Accessor-status heuristic check:** held up at 4 sightings; B-014 verified both rows' tags correct.
- **No prereq edges.** Soft note on B-003 precedent for R-A; already ratified.
- **B-014 status:** pending → designed.

### 2026-04-27 — B-011 design returned (cf9e1cc4)

- **Thin design doc** (deferred-to-B-003). All 7 B-011 rows are the same widget closures that B-003's R-A removes by construction; B-011 carries no independent implementation work and merges jointly with B-003's PR.
- **Hard prereq edge encoded:** all 7 rows → `emAutoplay-1171` (B-003's centralizing row).
- **Audit-data correction (split-file line drift):** `rust_file` for all 7 rows patched from `emAutoplay.rs` → `emAutoplayControlPanel.rs` (same line numbers; B-003 anomaly §2 recurring).
- **"Accessor present" disambiguation:** audit tag is widget-side accessors, not model-side. Correct as-is; flagged for future audit-heuristic tightening.
- **No new D-### entries.** R-A precedent covers all 7 rows.
- **B-011 status:** pending → designed.

### 2026-04-27 — B-018 design returned (04059bac) — false positive

- **B-018 closes as a verified false positive.** No code changes. emFileDialog-196 reclassified `drifted → faithful` (verified observable equivalence to C++).
- **P-008 pattern retired** in pattern-catalog.md with category-error retirement note (audit trail preserved). The audit's "connect-with-poll-fallback" framing assumed `IsSignaled` was an independent state poll; in emCore it's a wakeup-cause probe that depends on the connect having woken the engine. Connect + IsSignaled-in-Cycle is the canonical pattern, not hybrid drift.
- **Counts updated:** drifted 165 → 164; faithful (in actionable enrichment) 0 → 1; total actionable rows still 178 by audit definition (the reclassified row stays in inventory-enriched.json with verdict `faithful` for traceability, distinct from the 8 originally-faithful rows excluded from enrichment).
- **No new D-### entries. No prereq edges.**
- **Latent gap noted for separate audit follow-up:** `CheckFinish`'s post-show else branch (`rs:532-543`) parks OD via `pending_actions` without calling `scheduler.connect(od.finish_signal, outer_engine_id)`. All current callers are `#[cfg(test)]`; production goes through `run_file_dialog_check_finish`. If a non-test caller appears, this becomes drift. Captured in B-018 sketch's reconciliation block.
- **Pattern retirement is a meta-event for the audit framework:** future re-runs should drop P-008 from the heuristic catalog and treat connect + IsSignaled-in-Cycle as faithful.
- **B-018 status:** pending → designed (false positive — no implementation work; immediately mergeable).

### 2026-04-27 — B-012 design returned (bf6e9bd5)

- **All 7 rows uniform rule-1 convert** (D-002). No rule-2 candidates; no reclassifications. Accessor verification held.
- **Two-hop relay unwound (row 224):** `mw.ReloadFiles(&self, ectx)` fires `file_update_signal` synchronously per D-007. Deletes `mw.to_reload` field + `MainWindowEngine::Cycle` polling block. F5 hotkey input-path bifurcation handled by inlining the 1-line direct-fire branch (input lacks ectx; per-callsite resolution rather than a parallel API shim).
- **Hard prereq edges encoded:** all 7 rows → `cleanup-emMainControlPanel-35` (ClickFlags removal); rows 221 and 224 also → their specific cleanup items. B-019 must land first.
- **Soft prereq B-006 ↔ B-012:** shared first-Cycle init block in `emMainControlPanel`. Second-to-land merges its connect calls into the first's block. Already noted in B-012 inbound section.
- **Watch-list candidate added to D-007:** "Rust interposed a polling intermediary where C++ fires directly" — 2 sightings now (AutoplayFlags.progress + mw.to_reload). Promote on 3rd sighting. Pattern + resolution recipe captured in D-007's watch-list block.
- **Residual drift note (out of scope, follow-up audit):** rows 221 (fullscreen) and 226 (quit) keep stubbed log-only reaction bodies. Subscription drift fixed by B-012; reaction-body drift remains.
- **No new D-### entries promoted.**
- **B-012 status:** pending → designed.

### 2026-04-27 — B-010 design returned (09f08710) — final bucket; Phase 5 closes design phase

- **All 15 rows uniform rule-1 convert (D-002).** No rule-2 candidates; no reclassifications. Accessor verification held across all 15.
- **D-009-polling-intermediary-replacement promoted** by this brainstorm. Sighting 3 (`FsbEvents`) and sighting 4 (`generation` counter on emCoreConfigPanel) pushed past the 3-sighting threshold. D-007's watch-list paragraph shrunk to a back-pointer to D-009. B-003 and B-012 sketches updated with "now formalised as D-009" notes.
- **FsbEvents dropped** per rule-1 convert; widgets read directly via `with_behavior_as::<T>(panel_id, |p| ...)` typed downcast (precedent emPanelTree.rs:1714).
- **Generation counter (sighting 4) deferred:** out of B-010 row scope; row 80's reset reaction body keeps the bump verbatim. Likely future fix shape: config-changed signal on `emRecNodeConfigModel` + per-group subscribe + `UpdateOutput` handler mirroring C++ `emRecListener::OnRecChanged()`.
- **Bucket open questions §2/§3/§4 all resolved.** Bucket sketch's "no Cycle override exists" note disambiguated (Rust-side only; C++ has 4 Cycle overrides).
- **No new D-### entries beyond D-009; no prereq edges; no row reassignments; no accessor-status revisions.**
- **B-010 status:** pending → designed.

---

### Phase 5 design phase complete

All 19 buckets are now `designed`. Spine state at completion:
- **Decisions ratified:** D-001 through D-009 (9 global decisions).
- **Patterns retired:** P-008 (false positive; B-018).
- **Pattern reclassifications:** P-005 → P-004 (4 rows, B-013), P-005 → P-001 (1 row, B-014); accessor-status corrections on 4 rows (B-006/B-007/B-008/B-013).
- **Audit verdict shifts:** drifted 162 → 164 (gap-blocked rows reclassified up + B-018 reclassified down); gap-blocked 16 → 13; faithful (in actionable enrichment) 0 → 1.
- **Cross-bucket prereq DAG:** non-trivial after design — B-005 → B-009; B-008 → B-007; B-015 → B-008; B-016 → B-004; B-017 → B-001 + B-004; B-011 → B-003; B-012 → B-019.
- **Watch-list items deferred:** A3 (scheduler in emContext); generation counter on emCoreConfigPanel; emDialog post-show sync GetResult; mutator-fire ectx-threading benign-hybrid composition (recorded as D-007 composition note); audit accessor-status heuristic gap (4 sightings, established but not promoted — methodology issue, not design).

Phase 5 reconciliation continues as implementation merges land. Status column transitions designed → merged per implementation PR.

---

### 2026-04-27 — first-wave merges land (B-018, B-003, B-019)

- **B-018 → merged at 683153f1.** No implementation work (false positive; reclassification commit only). 2812/2812 tests pass.
- **B-019 → merged at 41599129.** 9 stale annotations cleaned up. Spec-compliant per review. Single commit.
- **B-003 → merged at eb9427db (3 commits: 12d3b4fe wire + 2ac6a627 spec-review fixups + eb9427db ViewModel unification).** R-A applied; AutoplayFlags shim dropped; emAutoplayViewModel signals (GetChangeSignal, GetProgressSignal) added; AutoplayCheckButtonPanel uses Rc<RefCell<emAutoplayViewModel>> + GetItemProgress() in Paint. Spec review surfaced 3 Important deviations; all resolved before merge.

**Spine amendments from these merges:**

1. **D-008 A1 sanctioned in combined form** (`GetXxxSignal(&self, ectx) -> SignalId` instead of `Ensure*` + `Get*` split). Earlier draft split was speculative; combined form mirrors C++ name and folds allocation into the call. Operational rule 1 amended in place; deferred open question struck. Sanctioned post-B-003 merge `eb9427db`.
2. **emAutoplay-1171 `rust_file` patched** in `inventory-enriched.json`: `emAutoplay.rs` → `emAutoplayControlPanel.rs`. The `rust_evidence.file` field was already correct; top-level `rust_file` carried the split-file line drift.
3. **emAutoplayViewModel-accessor row rename** (`...-accessor-model-state` → `...-accessor-progress`) was already applied during B-003 brainstorm reconciliation (commit 31ddd60b). No action needed.

**Follow-on debt surfaced (not Phase 5 buckets — separate cleanup):**

1. **B-019 quality review found 7 pre-existing pseudo-DIVERGED blocks** in `crates/emcore/src/emDialog.rs` (lines 540, 555, 563, 568, 576, 588, 593) and `crates/emcore/src/emFileDialog.rs:655` using the form `DIVERGED (Phase 3.6 Task 3)` / `DIVERGED (Phase 3.6.1 Task 2)` — parenthetical-phase form lacks the required forced-divergence category. The annotation linter's regex doesn't catch this form. Pre-dates B-019. **Recommend a follow-on bucket** to classify each block (re-categorize or remove) and tighten the linter regex. Owner: working-memory session to scope.
2. **B-003 stub follow-up:** `update_controls` and `update_progress` reaction bodies in `emAutoplayControlPanel` are logging stubs marked `B-003-follow-up`. Tracked in code, no inventory row. Address in a downstream pass once stocks/dialog buckets ship.
3. **B-003 documented exception:** `emAutoplayControlPanel::autoplay_model_for_test` is `pub` (not `pub(crate)`) because integration tests link as external crate. Doc-comment explains the deliberate CLAUDE.md §Visibility exception. Note for future readers: a `test-support` feature on `emmain` could remove this exception.
4. **B-003 cosmetic minors skipped per design call:** redundant inner `#[cfg(any(test, feature = "test-support"))]` on `flush_signals_for_test`; top-of-file comment in `emAutoplayControlPanel.rs` lost some architectural context after `DIVERGED:` removal; `ContinueLastAutoplay` comment could cite `emAutoplay.cpp:710` explicitly. Not blocking; revisit if a reader hits friction.

**Status:** 3 of 19 buckets merged. 16 remain `designed` and ready for implementation per the prereq DAG.

### 2026-04-27 — B-011 status reconciled (no code; absorbed into B-003)

B-011's design called out that all 7 rows are removed by B-003's R-A construction (no separate implementation). B-003 merged at `eb9427db`. Flipping B-011 status `designed → merged` to reflect reality. **4 of 19 buckets merged.**

### 2026-04-27 — B-006 + B-007 merged (parallel wave, batch 1)

- **B-006 → merged at f37adf01** (linear history 5963c688..f37adf01). 3 rows; emMainControlPanel Cycle init only.
- **B-007 → merged at 55d735bc** (linear history 858524f1..55d735bc). 3 rows + the latent `AcquireUpdateSignalModel` gap-fix (re-points to the shared `App::file_update_signal` broadcast per design).
- Test suite: 2812 → 2821 (+9 tests across both buckets).
- **B-007 merge unblocks B-008** (next wave can pick it up).
- **6 of 19 buckets merged. 13 remain.**

### 2026-04-28 — B-014 merged (parallel wave, batch 2)

- **B-014 → merged at c2871547** (linear history c89db09b..c2871547; commits: c89db09b feat + c2871547 fixup).
- **Effective row count: 1.** emAutoplay-1172 (Row 2) was discovered to be already absorbed by the B-003 merge — `AutoplayFlags` already dropped, `AutoplayCheckButtonPanel` already holds `Rc<RefCell<emAutoplayViewModel>>` and reads `model.borrow().GetItemProgress()` at paint, file-head DIVERGED block already replaced with benign migration prose. No B-014 commit touched emAutoplay. Spec-reviewer independently verified observable equivalence. emVirtualCosmos-575 (Row 1) carried the implementation: rule-1 convert per D-006 + D-007 + D-008.
- **D-008 A1 combined-form precedent compounds.** B-014 design doc §2.2 Step A had specified the pre-amendment split form (`GetChangeSignal()` + `EnsureChangeSignal(ectx)`); cluster convention (D-008 A1 amendment from B-003 merge `eb9427db`) prescribes combined `GetChangeSignal(&self, ectx) -> SignalId`. Implementation followed combined form per cluster convention; reviewer-approved. **B-014 design doc §2.2 Step A annotated as superseded** to prevent future bucket designers from re-deriving the split form.
- **D-008 entry amended** with B-014 added as second combined-form precedent (B-003, B-014).
- Test suite: 2821 → 2823 (+2 tests in B-014).
- **Inherited debt (not new):** `emAutoplayControlPanel.rs:704` carries a `TODO(B-003-follow-up)` from B-003. Already inventoried in B-003's reconciliation block above; noting here for completeness.
- **Out-of-scope future work confirmed:** `emVirtualCosmosModel::Cycle()` port (C++ has the model react to `FileUpdateSignalModel->Sig` and call `Reload()` from its own Cycle; Rust eagerly Reloads from Acquire). When that gap closes, the future-caller side of D-007 enumeration kicks in, and the CALLSITE-NOTE on `Reload` is the contract. Not blocking.
- **7 of 19 buckets merged. 12 remain.**

### 2026-04-28 — B-009 merged (parallel wave, batch 3)

- **B-009 → merged at 50994e26** (linear history 3b56e00b..50994e26; commits: 3b56e00b feat + 301f33fc test + 50994e26 fixup).
- **3 accessors flipped (D-001):** `emFileManModel::GetSelectionSignal`, `emFileManModel::GetCommandsSignal`, `emFileManViewConfig::GetChangeSignal`. 11 consumers migrated.
- **D-008 A1 combined-form precedent compounds (third sighting).** B-009 design doc was authored against the pre-amendment split form (`Ensure*Signal` + `Get*Signal`); cluster convention (B-003 + B-014 precedent) was re-applied. Design doc annotated as superseded.
- **D-007 mutator-signature shape broadened.** Design tables specified `&mut EngineCtx<'_>`; landed shape uses `&mut impl SignalCtx`. Both `EngineCtx` and `SchedCtx` impl `SignalCtx`. Forced because `PanelBehavior::Input` only receives `PanelCtx` (no `EngineCtx`). D-007 amended to record the broader trait-bound shape.
- **B-005 unblocked.** B-005's hard prereq edge (two rows blocked on D-001's accessor flip) is now satisfied; the `// see D-001` annotations in B-005's design doc are obsolete and have been struck.
- Test suite: 2823 → 2837 (+14 tests; 13 new in `tests/typemismatch_b009.rs` + 1 inline-test net adjustment).
- **Acknowledged deviations (reviewer-approved):**
  - emFileManControlPanel-326 (Selection) and -522 (Commands) reactions are `changed = true` placeholders. C++ `UpdateButtonStates` (cpp:366) and the commands-react block (cpp:533) are not yet ported. Branches wired and observed; comments at the sites flag the gap. Tracked as follow-on debt D1/D2.
  - Per-row click-through coverage compromise: 3 representative click-throughs (selection / change / commands) + 4 construction-only tests for the other panels, vs. design's per-row suggestion. Risk-acceptable per spec reviewer.
  - `#[cfg(test)] pub(crate) cached_*_signal()` getter on `emFileManModel` for 3 tests that need to observe the pre-allocation `null` state (combined-form accessor would allocate). Production fields stay private.
- **Follow-on debt (in code, not inventory rows):**
  1. **D1.** Port `UpdateButtonStates` and replace `changed = true` placeholder at `crates/emfileman/src/emFileManControlPanel.rs:324-330` (C++ `emFileManControlPanel.cpp:366`).
  2. **D2.** Port the commands-react block and replace `changed = true` placeholder at `crates/emfileman/src/emFileManControlPanel.rs:337-342` (C++ `emFileManControlPanel.cpp:533`).
  3. **D3.** `emFileManControlPanel::Input` carries 9 `_ctx.as_sched_ctx().expect("…requires full PanelCtx reach")` calls. Comment at site explains why; helper extraction blocked by borrow-checker. Refactor candidate if `PanelCtx` API gains an ergonomic `SchedCtx` accessor.
  4. **D4.** D-008 A3 watch-list (scheduler in `emContext`) — placeholder occupants unchanged: emFileLinkModel, emFileManTheme, emFileManConfig, emFileModel. B-009 did not grow the list.
- **8 of 19 buckets merged. 11 remain.**

### 2026-04-28 — B-005 merged (parallel wave, batch 4)

- **B-005 → merged at 91433733** (linear history d15bbca0..91433733; 4 commits: d15bbca0 feat-emFileLinkPanel + 02dd7305 feat-emFileManControlPanel + 2d606ac3 tests + 91433733 fixup).
- **D-001 prereq (B-009) consumed.** B-005's 2 cross-bucket-blocked rows landed cleanly on the post-B-009 combined-form accessors. Combined `GetSelectionSignal(ectx)` / `GetCommandsSignal(ectx)` / `GetChangeSignal(ectx)` calls used at the connect sites; "see D-001" caveats are now historical.
- **D-006 first-Cycle init shape ratified by implementation.** Three implemented sightings (B-014, B-009, B-005). Codify in pattern-catalog.md §P-002 as the canonical remediation shape for P-002 going forward.
- Test suite: 2837 → 2860 (+23 click-through tests).
- **Port-completeness fix (in-scope upscoping):** `emRadioButton` had no `click_signal` field — design doc cited `rb.borrow().check_signal` as if the field existed. Spec-review surfaced this; implementer added `pub click_signal: SignalId` to `emRadioButton` (matches C++ `emButton::GetClickSignal()` inheritance) and propagated per-radio subscribes at all 4 radio groups. This closed a real port gap; no DIVERGED annotation residue. **Cross-crate API change:** `emRadioButton::new` signature gained `cc: &mut impl ConstructCtx` first; emstocks (2 files) and eaglemode tests (6 files) updated. Reviewer-approved.
- **Acknowledged deviations (reviewer-approved):**
  - `PanelCtx::invalidate_painting` does not exist — substituted `self.needs_update = true` in `emFileLinkPanel-53` reaction. C++ cpp:90-93 also relies on the child-panel rebuild downstream rather than explicit InvalidatePainting on this branch. Inline equivalence comment added.
  - Extra `CommandsSignal` Cycle branch (no-op placeholder) — out of B-005's 21 rows but kept per existing B-009 audit-data note. Not a regression.
- **Follow-on debt (in code, not inventory rows):**
  1. **D1 (carryover from B-009):** `UpdateButtonStates` (cpp:451-499) still a placeholder beyond `sync_from_config`. Selection_signal / chg_sig branches lack full button-enable/disable refresh. Not regressed by B-005.
  2. **D2:** `save_button` post-Input requires `SetEnableSwitch(IsUnsaved())`; not wired.
  3. **D3:** `select_all` Cycle branch uses cached `dir_path` shortcut instead of C++ active-dir-panel walk (pre-existing).
  4. **D4 (new from B-005):** `sync_from_config_with_construct` introduced for construction-time AR-radio rebuild that needs `ConstructCtx` for per-radio click_signal allocation; the Cycle-time `sync_from_config` skips AR rebuild when `PanelCtx` has no scheduler. Production unaffected; consider unifying in a later sweep.
  5. **D5 (new from B-005):** `emRadioButton::new` signature changed (cross-crate API change documented above).
- **Process note:** PM session declined a second spec-review pass after fixup and skipped a separate code-review subagent dispatch. Combined-reviewer template (`COMBINED-REVIEWER-TEMPLATE.md`, committed `dbb54bad`) is now the recommended replacement for the legacy two-subagent flow on subsequent buckets.
- **9 of 19 buckets merged. 10 remain.**

### 2026-04-28 — B-005 retroactive combined review (post-merge)

Combined-reviewer template dispatched against `d15bbca0..91433733`. Result: **APPROVED FOR MERGE** (retroactive). All hard gates green: fmt + clippy -D warnings + nextest 2860/2860 + xtask annotations. Spec-compliance and code-quality both APPROVED.

- **Deviations (cluster-convention disposition, reviewer-approved):** per-radio `click_signal` panel-side subscribes (4 groups) widened beyond design's group-only shape — mirrors C++ `AddWakeUpSignal` per-radio at cpp:381-413. `emRadioButton::new` cross-crate API change is forced (port-completeness for the missing `click_signal` field).
- **Promotion candidates (watch for next sighting):**
  - **Per-radio `click_signal` subscribe pattern** at panel — 4 sightings *inside* B-005 (sort_radios, nss_radios, theme_style_radios, theme_ar_radios). Promotion candidate for a D-### "per-button widget signal subscribe" canonical pattern if it recurs in B-008/B-012/B-016/B-017.
  - **`#[doc(hidden)] pub *_for_test()` test-surface idiom** — 16 accessors in `emFileManControlPanel.rs:264-374`. Watch for B-006/B-007/B-008 sightings; promote to a feature-gated module or `expose_for_test!` macro if it recurs.
- **New debt items (D6–D9, accept-as-known-debt; not blocking subsequent buckets):**
  - **D6.** `crates/emfileman/tests/typed_subscribe_b005.rs` (569 lines) — repeated `make_control_panel` + `with_scheduler` PanelCtx + `sched_ctx_for(eid).fire(sig)` + `flush_signals_for_test` + `cycle_panel` + `cleanup` boilerplate across ~14 tests. Far exceeds the 30-line repeated-stub-engine threshold. Extract a `B005Harness` helper that wraps fire+flush+cycle into one call.
  - **D7.** `crates/emfileman/src/emFileManControlPanel.rs:251-296` — `sync_from_config` vs `sync_from_config_with_construct` body duplication; drift risk. Folds with D4. Fold construct path into a single helper (closure or `&mut dyn ConstructCtx`) for radio rebuild.
  - **D8.** `emFileManControlPanel.rs:439` — `Cycle` is ~325 lines; consider extracting branch helpers (`react_theme()`, `react_sort_radios()`, etc.) for readability without changing semantics.
  - **D9.** Q3 (mutator effect) coverage gap on save_button (-342) / select_all (-343) / paths_clip (-346) / names_clip (-347) tests — currently smoke-only `does_not_panic`, verify Q1+Q2 (subscribed + Cycle observes) but not Q3 (actual side-effect: file write, clipboard contents). Test-coverage debt.
  - **D10.** `emFileManControlPanel.rs:765-826` — `Input()` is 14 `if widget.Input(...) { return true; }` blocks; collapse via slice/loop helper. Pre-existing-style but worth folding.
- **No fix-forward action.** All findings disposition is accept-as-known-debt. No follow-up commit.

### 2026-04-28 — B-008 merged (parallel wave, batch 5)

- **B-008 → merged at 133de22e** (linear history c68360ef..133de22e). 3 rows: emMainPanel-67 (EOI subscribe), emMainPanel-69 (WindowFlags subscribe), emVirtualCosmos-104 (FileUpdate broadcast subscribe).
- **Cross-bucket prereq (B-007 → B-008 row 104) honored.** B-007's `emFileModel::AcquireUpdateSignalModel` re-pointing landed first; row 104's broadcast subscribe consumed the post-B-007 shape cleanly.
- **D-006 first-Cycle init shape compounded** (fourth implemented sighting after B-014/B-009/B-005). emMainPanel rows -67/-69 added a single combined `subscribed_init` block in `emMainPanel::Cycle`.
- Test suite: 2860 → 2865 (+5 tests; 3 inline click-through in emMainPanel/emVirtualCosmos + 2 accessor-stability tests in `crates/emmain/tests/typed_subscribe_b008.rs`).
- **Cost-pilot signal (EP-vs-SDD).** B-008 ran via `/executing-plans` in a separate session (vs. the subagent-driven-development flow used on prior buckets). Implementer reports "felt 30–50% lighter" for this bucket size — single combined-reviewer dispatch at the end was the only subagent; no per-task fresh-subagent context-prime cost; no cargo-error iterations. Caveat: bucket was uniform-shape mechanical-heavy (3 well-trodden D-006 wires). Larger or judgement-heavy buckets may not see the same savings as parent-session context grows past usefulness. Worth re-evaluating on B-001/B-010/B-012.
- **Acknowledged deviations (cluster-convention disposition, reviewer-approved):**
  - **Row -104 hosting choice (implementer's pick).** Design left this open (open-question 4): a separate `VirtualCosmosUpdateEngine` driving engine vs. panel-side first-Cycle subscribe in `emVirtualCosmosPanel::Cycle` alongside the existing B-014 ChangeSignal subscribe. Implementation chose panel-side. Rationale: avoids threading a `ConstructCtx` through `emVirtualCosmosModel::Acquire`, and the panel is the model's sole consumer. Observable contract preserved (broadcast wake → `Reload()` → `ChangeSignal` fire → `update_children`).
  - **Row -104 synthesized ChangeSignal fire.** After `Reload()`, the Cycle code synthesizes `ectx.fire(chg)` so the existing ChangeSignal-driven `update_children` runs in the same time slice. Mirrors C++ `Signal(ChangeSignal)` inside `Reload`.
- **Promotion candidates (watch for next sighting):**
  - **Panel-side broadcast subscribe for non-engine emModel.** First sighting in this cluster (B-008 row -104, where `emVirtualCosmosModel` is not an `emEngine` and so cannot host its own subscribe — the panel hosts a `Reload() + fire(ChangeSignal)` block in *its* Cycle, mirroring C++ model::Cycle's behavior on the panel side). Promotion candidate for a D-### "panel-hosted model-Cycle proxy" pattern if it recurs.
- **New debt items (D11–D13, accept-as-known-debt; not blocking subsequent buckets):**
  - **D11.** Test-scaffold extract-helper debt — three click-through tests across `emMainPanel.rs` and `emVirtualCosmos.rs` duplicate ~60 lines of `EngineCtx + tree + scheduler` stub setup. Same shape recurring across B-005/B-006/B-007/B-008. Candidate for a shared `mock_engine_ctx_for_panel` test helper (compounds with B-005 D6).
  - **D12.** `crates/emmain/tests/typed_subscribe_b008.rs::row_67_eoi_signal_field_addressable` is compile-time only (inner `_shape_check` fn). Behavioral coverage lives in inline `b008_row_67_eoi_signal_finalises_slider_hide`. Either remove the compile-time test or upgrade to a runtime assertion.
  - **D13.** Out-of-scope cross-edge confirmed: `emMainPanel-68` (`SliderTimer.GetSignal()`) is still polled — B-015's territory. B-008 left the polling code untouched per design.
- **Process note:** Combined-reviewer template ran end-of-bucket and approved cleanly with the deviations above. No fixup commit needed.
- **10 of 19 buckets merged. 9 remain.**
