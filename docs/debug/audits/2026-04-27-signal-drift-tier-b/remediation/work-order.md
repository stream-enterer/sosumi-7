# Signal-Drift Remediation — Work Order

**Generated:** 2026-04-27 from Phase 4 of the bookkeeping strategy.
**Total buckets:** 19
**Layers:** 1 (no cross-bucket prereqs — all 11 prereq edges in `inventory-enriched.json` are intra-B-009 consumer→accessor)

Buckets are ordered by topological layer over the prereq DAG (lower layer = no unmet prereqs). With a single layer, ordering reduces to mechanical-heavy first, then balanced, then judgement-heavy — mechanical work validates the underlying patterns cheaply before committing to judgement-laden buckets.

## Order

| # | Bucket | Layer | Mechanical-vs-judgement | Rows | Status | Design doc |
|---|---|---|---|---|---|---|
| 1 | B-005-typed-subscribe-emfileman | 0 | mechanical-heavy | 21 | designed | [d95d55a7](../../../../superpowers/specs/2026-04-27-B-005-typed-subscribe-emfileman-design.md) |
| 2 | B-006-typed-subscribe-mainctrl | 0 | mechanical-heavy | 3 | designed | [a13880c7](../../../../superpowers/specs/2026-04-27-B-006-typed-subscribe-mainctrl-design.md) |
| 3 | B-007-typed-subscribe-emcore | 0 | mechanical-heavy | 3 | designed | [8b220ebb](../../../../superpowers/specs/2026-04-27-B-007-typed-subscribe-emcore-design.md) |
| 4 | B-008-typed-subscribe-misc | 0 | mechanical-heavy | 3 | designed | [4c4141f1](../../../../superpowers/specs/2026-04-27-B-008-typed-subscribe-misc-design.md) |
| 5 | B-015-polling-emcore-plus | 0 | mechanical-heavy | 10 | designed | [b521b3f6](../../../../superpowers/specs/2026-04-27-B-015-polling-emcore-plus-design.md) |
| 6 | B-019-stale-annotations | 0 | mechanical-heavy | 9 | designed | [e7129430](../../../../superpowers/specs/2026-04-27-B-019-stale-annotations-design.md) |
| 7 | B-001-no-wire-emstocks | 0 | balanced | 71 | designed | [456fa5f7](../../../../superpowers/specs/2026-04-27-B-001-no-wire-emstocks-design.md) |
| 8 | B-002-no-wire-emfileman | 0 | balanced | 4 | designed | [7fb3decd](../../../../superpowers/specs/2026-04-27-B-002-no-wire-emfileman-design.md) |
| 9 | B-003-no-wire-autoplay | 0 | balanced | 3 | designed | [703fa462](../../../../superpowers/specs/2026-04-27-B-003-no-wire-autoplay-design.md) |
| 10 | B-004-no-wire-misc | 0 | balanced | 4 | pending | — |
| 11 | B-016-polling-no-acc-emfileman | 0 | balanced | 3 | pending | — |
| 12 | B-017-polling-no-acc-emstocks | 0 | balanced | 3 | pending | — |
| 13 | B-009-typemismatch-emfileman | 0 | judgement-heavy | 14 | pending | — |
| 14 | B-010-rc-shim-emcore | 0 | judgement-heavy | 15 | pending | — |
| 15 | B-011-rc-shim-autoplay | 0 | judgement-heavy | 7 | pending | — |
| 16 | B-012-rc-shim-mainctrl | 0 | judgement-heavy | 7 | pending | — |
| 17 | B-013-dialog-cells-emstocks | 0 | judgement-heavy | 4 | pending | — |
| 18 | B-014-rc-shim-no-acc-misc | 0 | judgement-heavy | 2 | pending | — |
| 19 | B-018-fileDialog-singleton | 0 | judgement-heavy | 1 | pending | — |

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
