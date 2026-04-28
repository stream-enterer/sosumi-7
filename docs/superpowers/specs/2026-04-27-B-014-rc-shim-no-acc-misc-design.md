# B-014-rc-shim-no-acc-misc — Design

**Bucket:** B-014-rc-shim-no-acc-misc
**Pattern (post-correction):** mixed — emAutoplay-1172 stays P-005 (R-A apply); emVirtualCosmos-575 reclassifies P-005 → P-001 (see Audit-data corrections)
**Scope:** `emmain` (1 row in `crates/emmain/src/emAutoplayControlPanel.rs`, 1 row in `crates/emmain/src/emVirtualCosmos.rs`)
**Cited decisions (per row):**
- emAutoplay-1172: D-002 §1 R-A (ratified during B-003)
- emVirtualCosmos-575: D-002 rule 1 (convert), D-006-subscribe-shape (first-Cycle init), D-007-mutator-fire-shape (ectx-threaded fire), D-008-signal-allocation-shape (lazy `EnsureChangeSignal`)
**Prereq buckets:** B-003 (the R-A application here uses the same shape as B-003's outbound-half resolution; B-003 need not land first because the two transforms touch disjoint code, but the design citation chain assumes B-003's R-A entry in `decisions.md` D-002 §1 is in place — it is, as of 2026-04-27)
**New global decisions:** none

---

## 0. Bucket overview

This is a "misc" bucket and is **mixed-pattern by design** — the two rows share neither pattern nor fix shape. Implementer should read this doc as **two independent fixes that happen to share a bucket**, not as one unified pattern application:

- **emAutoplay-1172** is an R-A application (drop the rc-shim outbound, replace with `Rc<RefCell<emAutoplayViewModel>>` paint-time read on `AutoplayCheckButtonPanel`). No new accessor; no subscribe; observable timing matches the B-003 precedent. Single PR or merged into B-003's PR — implementer's choice.
- **emVirtualCosmos-575** is a full rule-1 convert with the D-006 + D-007 + D-008 stack: add a real `GetChangeSignal()` accessor on `emVirtualCosmosModel`, fire it from `Reload()`, subscribe in `emVirtualCosmosPanel` via first-Cycle init, react in `Cycle()` by calling `update_children`. Existing `Notice(NF_VIEWING_CHANGED)` path is preserved unchanged (mirrors C++ `Notice` → `UpdateChildren`).

There is no shared infrastructure between the two fixes. Land them in either order.

---

## 1. Audit-data corrections

**emAutoplay-1172:** no audit data correction. P-005 classification holds (rc-shim consumer with missing accessor). The R-A resolution removes the shim *without* adding the accessor, so the row exits the inventory as "fixed-by-redesign," not as "rule-1 convert."

**emVirtualCosmos-575:** **P-005 → P-001 reclassification.** The audit's `rc_cell_shim` evidence-kind misread `model: Rc<RefCell<emVirtualCosmosModel>>` (the panel's routine model handle, analogous to C++ `emRef<emVirtualCosmosModel>`) as a click-handler shim routing around a signal. The actual drift mechanism is "wrong trigger" (panel reacts to `NF_VIEWING_CHANGED` notice instead of model `ChangeSignal`), with no shim involved. Per pattern-catalog definitions, this is P-001-no-subscribe-no-accessor (consumer doesn't subscribe + accessor doesn't exist).

**Working-memory updates required (handled by the reconciling session, not the implementer):**
- `inventory-enriched.json`: emVirtualCosmos-575 `pattern_id` P-005 → P-001; `evidence_kind` rc_cell_shim → absent.
- `decisions.md` D-002 "Affects" line: P-005 -1 (now P-005=1: just emAutoplay-1172).
- `pattern-catalog.md` P-005 row count: 2 → 1; P-001 row count: 82 → 83.
- `B-014` bucket sketch: row count 2 holds; per-row pattern column annotated.

This is the second audit-classification correction in the rc-shim family (cf. B-013's P-005→P-004 reclassification). The audit's evidence-kind heuristic does not distinguish "rc-shim around a signal" from "rc routine model handle." Future P-005 buckets should verify the rc/cell occurrence is actually in a click-handler closure routing around a signal accessor before trusting the classification.

**Accessor-status verification (the B-006/B-007/B-008/B-013 heuristic-gap check):** held up this time.
- emAutoplay-1172: `emAutoplayViewModel` exposes `GetItemProgress()` (data, line 884) but no `GetProgressSignal()`. Verified missing.
- emVirtualCosmos-575: `emVirtualCosmosModel` (lines 213–380) exposes no signal accessor of any kind. Verified missing.

---

## 2. Per-row design

### 2.1 emAutoplay-1172 — apply D-002 §1 R-A

**C++ reference (verified):** `emAutoplay.cpp:1172` is `AddWakeUpSignal(Model->GetProgressSignal())` in `emAutoplayControlPanel`'s constructor; `Cycle()` at cpp:1216 reads `IsSignaled(Model->GetProgressSignal())` and calls `UpdateProgress()` (cpp:1374), which calls `BtAutoplay->SetProgress(Model->GetItemProgress())`.

**Rust today:**
- `emAutoplayControlPanel.rs:84+` defines `AutoplayFlags` with `progress: Rc<Cell<f64>>` (the outbound channel from `emAutoplayControlPanel` to `AutoplayCheckButtonPanel`'s paint).
- An existing DIVERGED annotation at the file head claims this is language-forced; B-003 brainstorm proved this annotation false (the inbound `Cell`s are dead code; the outbound `progress` is the only load-bearing piece and has a clean R-A path).

**Fix (R-A application, per B-003 precedent):**
1. Drop `AutoplayFlags` entirely (the inbound `Cell`s are dead per B-003 audit; the outbound `progress` is replaced below).
2. `AutoplayCheckButtonPanel` (the panel that previously consumed `progress`) gains an `Rc<RefCell<emAutoplayViewModel>>` field.
3. In `AutoplayCheckButtonPanel::Paint`, replace the `progress.get()` call with `model.borrow().GetItemProgress()`.
4. Remove the file-head DIVERGED annotation (B-003's R-A removes its justification).

**Observable behavior under R-A:** progress is read at paint time rather than pushed on signal-fire. This is the divergence B-003 ratified knowingly: paint time is the only moment progress visibly affects output, and the C++ subscribe + `UpdateProgress` chain ultimately lands the value in `BtAutoplay`'s state for the next paint. End-state per paint is identical; the intermediate "BtAutoplay's stored progress between paints" is below the observable surface. Per Port Ideology, idiom adaptation that preserves observable output is unannotated.

**No accessor added.** No `GetProgressSignal()` is added to `emAutoplayViewModel` — R-A obviates the need. If a future caller needs to subscribe to progress changes (none does today, including in the C++ source besides this one site), that future caller can promote the accessor as a separate change.

**No subscribe added.** No D-006/D-007/D-008 wiring on this row.

### 2.2 emVirtualCosmos-575 — full rule-1 convert (D-006 + D-007 + D-008)

**C++ reference (verified):** `emVirtualCosmos.cpp:575` is `AddWakeUpSignal(Model->GetChangeSignal())` in `emVirtualCosmosPanel`'s constructor. `Cycle()` (cpp:606) calls `UpdateChildren()` on `IsSignaled(...)`. `Notice(NF_VIEWING_CHANGED)` (cpp:613) *also* calls `UpdateChildren()` independently. The model fires `Signal(ChangeSignal)` from `Reload()` (cpp:226).

**Rust today:**
- `emVirtualCosmosModel` (line 213) has no `ChangeSignal` field, no `GetChangeSignal()` accessor, no fire site.
- `emVirtualCosmosPanel` (line 821) reacts only to `NF_VIEWING_CHANGED` notice flag (line 838) by setting `needs_update`, drained in `Cycle()` at line 829.

**Drift:** Rust observes only viewing-change events. If `Reload()` is invoked while viewing parameters are static (e.g., a future port of `emVirtualCosmosModel::Cycle()` reacting to `FileUpdateSignalModel`), the panel never updates.

**Fix (rule-1 convert per D-002, wired per D-006/D-007/D-008):**

**Step A — accessor + lazy allocation (D-008 A1).**

> **SUPERSEDED post-merge (2026-04-28, c2871547):** The split form below (`GetChangeSignal()` + `EnsureChangeSignal(ectx)`) was written against the pre-amendment draft of D-008 A1. Cluster convention (D-008 A1 amendment from B-003 merge `eb9427db`) prescribes the **combined form** `GetChangeSignal(&self, ectx: &mut EngineCtx<'_>) -> SignalId` — a single accessor that folds the lazy-alloc check into the call. B-014 implementation followed combined form per cluster convention; reviewer-approved. Future "no-acc" bucket designers: use combined form directly. Block below preserved as historical record.

Add to `emVirtualCosmosModel`:
```rust
// Field on the struct:
change_signal: Cell<SignalId>,  // SignalId::null() until first subscriber

// Initializer in Acquire's closure:
change_signal: Cell::new(SignalId::null()),

// Public API mirroring C++ const emSignal & GetChangeSignal() const + the
// lazy-alloc helper used by D-006 first-Cycle init:
pub fn GetChangeSignal(&self) -> SignalId {
    self.change_signal.get()
}

pub fn EnsureChangeSignal(&self, ectx: &mut EngineCtx<'_>) -> SignalId {
    let sig = self.change_signal.get();
    if sig.is_null() {
        let new_sig = ectx.create_signal();
        self.change_signal.set(new_sig);
        new_sig
    } else {
        sig
    }
}
```

**Step B — mutator fire (D-007).** `Reload()` is the only mutator that fires `ChangeSignal` in C++ (cpp:226 inside `Reload()`). In Rust, the analogous fire site is at the bottom of `Reload()`'s success path.

**Per-callsite enumeration (D-007 discipline):**

| Callsite | File:line | ectx available? | Disposition |
|---|---|---|---|
| `Acquire()` closure bootstrap | `emVirtualCosmos.rs:233` (`model.Reload()` inside `ctx.acquire(...)` closure) | ❌ no | **Per-callsite hybrid: keep Reload as `&mut self`, no fire.** At Acquire-time no panel has subscribed → `change_signal == SignalId::null()` → fire would be a documented no-op per D-008 A1. The composition of D-007/D-008 makes this benign-by-construction: the missing fire is semantically correct, not a hack. |
| (any future caller, e.g., a future port of `emVirtualCosmosModel::Cycle` reacting to `FileUpdateSignalModel`) | not yet ported | n/a | Future caller must thread `&mut EngineCtx<'_>` and call `ectx.fire(self.change_signal.get())` after the load. Document via `// CALLSITE-NOTE:` comment on `Reload`. |

The implementer **does not need to add ectx threading** to the existing Acquire-bootstrap callsite — D-008 lazy-alloc semantics absorb the case. The `// CALLSITE-NOTE:` comment is the only deliverable for the future-caller side.

**Step C — consumer subscribe (D-006 first-Cycle init).** `emVirtualCosmosPanel` gains:

```rust
// New field on the panel struct:
subscribed_init: bool,

// Initializer:
subscribed_init: false,
```

In `Cycle()`:
```rust
fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
    if !self.subscribed_init {
        let sig = self.model.borrow().EnsureChangeSignal(ectx);
        ectx.connect(sig, ectx.id());
        // C++ initial-load is observed via the first Cycle's IsSignaled fire from
        // FileUpdateSignalModel boot; in the Rust port the Acquire-bootstrap Reload
        // has already loaded items, so the first Cycle has nothing to do beyond
        // connect. No initial-load synthesis needed here.
        self.subscribed_init = true;
    }

    if ectx.is_signaled(self.model.borrow().GetChangeSignal()) {
        self.update_children(ctx);
    }

    if self.needs_update {
        self.needs_update = false;
        self.update_children(ctx);
    }
    false
}
```

The existing `notice(NF_VIEWING_CHANGED)` → `needs_update = true` path stays. It mirrors C++'s independent `Notice(NF_VIEWING_CHANGED) → UpdateChildren()` (cpp:613–617). Both paths converge on `update_children`.

### 2.3 Annotation hygiene

- **emAutoplay-1172:** remove the file-head DIVERGED block on `emAutoplayControlPanel.rs:1+` (justification removed by B-003 R-A; nothing replaces it because the R-A path is unannotated idiom adaptation).
- **emVirtualCosmos-575:** no annotations added or removed. The fix mirrors C++ structure exactly.

---

## 3. Test impact

- **emAutoplay-1172:** existing tests at `emAutoplay.rs:1124` and `:1756–1763` exercise `GetItemProgress()` / `SetItemProgress()` directly on the ViewModel — unaffected. `AutoplayCheckButtonPanel` paint behavior should be exercised end-to-end via existing golden test if one exists, or by a small unit test confirming `Paint` reads progress through the model handle. Implementer to scan existing tests.
- **emVirtualCosmos-575:** add a unit test that constructs `emVirtualCosmosModel`, subscribes a stub consumer via `EnsureChangeSignal`, calls `Reload()` with ectx, and confirms the signal fires. Existing `from_items(...)`-based panel tests at `emVirtualCosmos.rs:994+` and `:1014+` should continue passing without modification (they exercise `update_children` directly, not the signal path).

---

## 4. Out of scope (flagged for future buckets)

- **`emVirtualCosmosModel::Cycle()` port** — C++ has the model react to `FileUpdateSignalModel->Sig` and call `Reload()` from its own Cycle. Rust eagerly calls `Reload()` from Acquire instead. Closing this gap is a separate divergence (model-side wiring) that, when ported, will require threading ectx into the new Cycle and through Reload — at which point the future-caller side of D-007 enumeration kicks in. Not blocking B-014; surface as an inventory item if not already tracked.
- **emAutoplay `UpdateProgress` infrastructure** — C++ `emAutoplay.cpp:1370+` (`UpdateProgress` method). R-A obviates the port for the progress wiring specifically; if a future panel needs other progress-driven state, it can land then.

---

## 5. Implementation order (for the writing-plans step)

1. emVirtualCosmos accessor + lazy alloc (Step A).
2. emVirtualCosmos mutator fire + callsite-note (Step B).
3. emVirtualCosmos consumer subscribe + first-Cycle init (Step C).
4. emVirtualCosmos test scaffolding.
5. emAutoplay R-A application (drop `AutoplayFlags`, plumb model handle, switch paint read).
6. emAutoplay annotation removal.

Steps 1–4 are independent of 5–6; can proceed in either order or in parallel. Land as separate commits within one PR or as two PRs — implementer's choice.
