# B-014-rc-shim-no-acc-misc — mixed P-005/P-001 misc (R-A drop + rule-1 convert)

**Pattern:** mixed — P-005-rc-shim-no-accessor (emAutoplay-1172) + P-001-no-subscribe-no-accessor (emVirtualCosmos-575, reclassified by B-014 brainstorm). Bucket title retained for stability; row set unchanged.
**Scope:** misc (emAutoplay, emVirtualCosmos)
**Row count:** 2
**Mechanical-vs-judgement:** judgement-heavy
**Cited decisions:** D-002-rc-shim-policy §1 R-A (emAutoplay-1172 — drop AutoplayFlags, ratified during B-003), D-002 rule 1 (emVirtualCosmos-575 — convert), D-006-subscribe-shape (emVirtualCosmos panel wiring), D-007-mutator-fire-shape + D-008-signal-allocation-shape (emVirtualCosmos accessor + Reload fire).
**Prereq buckets:** none.

**Status:** merged at `c2871547` (linear history `c89db09b..c2871547`; 2026-04-28).

**Post-merge notes (2026-04-28):**
- **Effective row count: 1.** emAutoplay-1172 (Row 2) was discovered already absorbed by the B-003 merge — `AutoplayFlags` already dropped, `AutoplayCheckButtonPanel` already holds `Rc<RefCell<emAutoplayViewModel>>` with `GetItemProgress()` reads in Paint, file-head DIVERGED block already replaced. No B-014 commit touched emAutoplay; spec-reviewer verified observable equivalence.
- **emVirtualCosmos-575 implemented per design** with one deviation: D-008 A1 accessor shape used the cluster's combined form `GetChangeSignal(&self, ectx) -> SignalId` rather than the design doc's split `GetChangeSignal()` + `EnsureChangeSignal(ectx)`. Cluster convention amendment from B-003 merge `eb9427db` (re-applied here) supersedes the design doc. Reviewer-approved.
- **Test suite:** 2821 → 2823 (+2 B-014 tests).
- **Design doc §2.2 Step A annotated as superseded** to prevent future "no-acc" bucket designers from re-deriving the split form.

**Reconciliation amendments (2026-04-27, post-design d7d964d4):**
- **emVirtualCosmos-575 reclassified:** `pattern_id P-005 → P-001`, `evidence_kind rc_cell_shim → absent`. Audit misread `model: Rc<RefCell<emVirtualCosmosModel>>` (routine model handle, analogous to C++ `emRef<>`) as a click-handler shim. Actual mechanism is "wrong trigger" (`NF_VIEWING_CHANGED` notice instead of `ChangeSignal`). Fix shape unchanged; audit truth cleaner. Row stays in B-014; boundaries frozen.
- **emAutoplay-1172 disposition:** apply D-002 §1 R-A by precedent (already ratified during B-003). Drop `AutoplayFlags` shim; give `AutoplayCheckButtonPanel` `Rc<RefCell<emAutoplayViewModel>>`; read `GetItemProgress()` in `Paint`. No accessor added, no subscribe added.
- **emVirtualCosmos-575 disposition:** rule-1 convert. Add `GetChangeSignal` + `EnsureChangeSignal` (D-008 A1 lazy alloc); fire from `Reload()` (D-007 — but see benign-hybrid note); subscribe via D-006 first-Cycle init in panel; existing `NF_VIEWING_CHANGED → update_children` path preserved (mirrors C++ `Notice`).
- **D-007 benign hybrid recorded.** Single production callsite of `Reload` is inside `Acquire`'s bootstrap closure and lacks ectx. At Acquire-time no panel has subscribed → `change_signal == SignalId::null()` → fire would be a no-op per D-008. Mutator keeps no-ectx signature with a `// CALLSITE-NOTE:` for future post-Acquire callers. D-007 amended in place to record this composition.
- **Accessor-status heuristic check:** held up — both rows' missing-accessor tags verified correct. No 5th sighting of the heuristic gap; pattern frequency stable at 4.
- **D-002 affects amended:** P-005 −1 (now =1, just emAutoplay-1172). pattern-catalog.md: P-001 row count 82 → 83; P-005 2 → 1.

## Pattern description

Same rc-shim consumer pattern as P-004 (consumer routes around the signal via `Rc<RefCell<>>` / `Rc<Cell<>>` shared state) but the upstream signal accessor is also missing, so the fix needs accessor design plus shim removal. In this bucket the two affected scopes are emAutoplay (a Rust-only panel with no direct C++ analogue for its flag-passing shape) and emVirtualCosmos (whose model exposes no signal at all).

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emAutoplay-1172 | src/emMain/emAutoplay.cpp:1172 | crates/emmain/src/emAutoplay.rs:104 | missing | emAutoplayViewModel has no GetProgressSignal accessor; C++ UpdateProgress (emAutoplay.cpp:1370+) unported. |
| emVirtualCosmos-575 | src/emMain/emVirtualCosmos.cpp:575 | crates/emmain/src/emVirtualCosmos.rs:821 | missing | Model exposes no GetChangeSignal and never fires one; panel reacts to NF_VIEWING_CHANGED instead. |

## C++ reference sites

- src/emMain/emAutoplay.cpp:1172
- src/emMain/emVirtualCosmos.cpp:575

## Open questions for the bucket-design brainstorm

- D-002 explicitly defers the emAutoplay flags-passing pattern (`AutoplayFlags { progress: Rc<Cell<f64>> }`): does it fall under rule 1 (convert) or rule 2 (keep as post-finish handoff)? emAutoplay has no C++ analogue so the rule needs adaptation — escalate to working-memory before bucket execution.
- For emAutoplay, should the missing `UpdateProgress` infrastructure (emAutoplay.cpp:1370+) be ported as part of this bucket, or split into a prerequisite porting bucket? (Echoes D-003 gap-blocked-fill-vs-stub framing even though this row is not gap-tagged.)
- For emVirtualCosmos, the C++ model has no signal either — is the Rust panel's NF_VIEWING_CHANGED path observably equivalent (in which case this row is a re-classification toward "no drift") or is there a still-missing C++-side change signal to mirror?
- If emVirtualCosmos turns out to be observably equivalent, does it leave this bucket entirely (annotation only) or does the rc-shim still need removal on Port-Ideology grounds?
