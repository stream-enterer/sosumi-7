# B-002-no-wire-emfileman — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm); amended 2026-05-01 to fold adversarial-review findings (see § Amendment Log)
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-002-no-wire-emfileman.md`
**Pattern:** P-001-no-subscribe-no-accessor
**Scope:** emfileman, 4 rows
**Mechanical-vs-judgement:** balanced — judgement concentrates in one accessor (G2: emRecFileModel change-signal infrastructure); the other three rows are mechanical once that accessor lands.

## Goal and scope

Wire the four missing P-001 sites in emfileman: both halves of the wire (model-side accessor + consumer-side subscribe). The four rows split across two accessor groups and one panel-local infrastructure port (emTimer wakeup):

1. **G1 — emTimer-driven idle wakeup on `emDirPanel` — CLOSED as behavioral equivalence (2026-05-01 amendment).** C++ uses an `emTimer` with `AddWakeUpSignal(timer.GetSignal())` to clear key-walk state after 1000 ms idle. C++ `emDirPanel::ClearKeyWalkState` (`src/emFileMan/emDirPanel.cpp:350-355`) is purely `delete KeyWalkState; KeyWalkState=NULL;` — no paint, no invalidation, no observable side effect. Rust's lazy `Instant::now()` comparison on next `Input` (`emDirPanel::key_walk`) is observably equivalent on every trajectory: nothing user-visible depends on *when* the internal struct is freed, only that the next keystroke after >1 s starts a fresh search (which both implementations achieve). Row `emDirPanel-432` is therefore re-classified as below-surface adaptation (no `DIVERGED:` annotation needed; the difference does not cross the observable surface). G1 ships zero rows. See § Amendment Log entry 1.
2. **G2 — `emRecFileModel<T>::GetChangeSignal()` accessor + signal infrastructure.** The C++ class hierarchy is `emRecFileModel : public emFileModel`, where `emFileModel` owns `change_signal`. The Rust port broke this: `emRecFileModel<T>` (`emcore/src/emRecFileModel.rs:15`) is a *standalone* port that does not embed `emFileModel<T>` ("Standalone Rust port… Does not wrap `emFileModel<T>` to avoid self-referential borrow-checker constraints"). It has no `change_signal` field. `emFileLinkModel` composes `emRecFileModel` and exposes only `GetFileStateSignal`. Fix is to add change-signal infrastructure to `emRecFileModel` and a delegating accessor on `emFileLinkModel`. Three of the four bucket rows depend on G2.

All four rows are wired using **D-006-subscribe-shape** (first-Cycle init + IsSignaled top-of-Cycle). The accessor add lands in scope per **D-003-gap-blocked-fill-vs-stub**.

## Cited decisions

- **D-006-subscribe-shape** — canonical wiring pattern (`subscribed_init` flag + `ectx.connect` in first Cycle + `IsSignaled` at top). emFileLinkPanel reuses the *existing* `subscribed_init` block installed by B-005 at `crates/emfileman/src/emFileLinkPanel.rs:71, 88, 186-196`; the model-change branch is added inside it (no new flag).
- **D-007-mutator-fire-shape** — `signal_change(&self, ectx: &mut impl SignalCtx)` fires inside each mutator after the state transition. Forces threading `&mut impl SignalCtx` through `set_unsaved_state_internal`, `GetWritableMap`, `TryLoad`, `Save`, `update`, `hard_reset`, `clear_save_error`. Bootstrap-only callers use the B-014 `// CALLSITE-NOTE:` escape hatch (no-op at null SignalId).
- **D-008-signal-allocation-shape** — A1 (`Cell<SignalId>` lazy-allocated in combined-form `GetChangeSignal(&self, ectx)`). `new(path)` keeps its current signature; no Acquire-signature ripple. Mirrors B-009 `emFileManViewConfig.rs:365, 419-444` and B-014 `emVirtualCosmosModel`.
- **D-009-polling-intermediary-replacement** — *not applicable*. B-002 introduces no `Cell`/`RefCell`-as-poll-buffer intermediary; the only `Cell` is the D-008 SignalId slot, which is allocation state, not a polling queue.
- **D-003-gap-blocked-fill-vs-stub** — fill the missing G2 accessor in this bucket; both halves live in emfileman+emcore scope. The emRec hierarchy is *partially ported* (the standalone `emRecFileModel<T>` is in tree); the fix adds the missing signal field, not a new model. Therefore D-003's "fill in scope" rule applies.
- **D-001** — *not* cited; G2 returns `SignalId`, not `u64`. No type-mismatch in this bucket.

## Audit-data anomalies (corrections)

The following audit observations are stale or under-specified. Rows remain in B-002; the design records the correction so the working-memory session can patch `inventory-enriched.json`.

1. **`emFileLinkPanel-56`** — audit notes "Rust exposes only `GetFileStateSignal`," which is correct, but classifies the missing accessor as `emRecFileModel::GetChangeSignal`. It is more precisely `emFileModel::GetChangeSignal` *as inherited via emRecFileModel*. In Rust, the standalone-port choice means the accessor must be added on `emRecFileModel<T>` directly (not delegated through a wrapped `emFileModel<T>`). This is the structural-divergence consequence flagged in this bucket's open question.

2. **`emFileLinkModel-accessor-model-change`** — audit notes "fix requires propagating `GetChangeSignal` up `emStructRec`-derived models (also affects emAutoplay, emVirtualCosmos)." Re-reading the source: the change signal does *not* live on `emStructRec` or `emRec` in C++ — it lives on `emFileModel` and `emRecFileModel` exposes it because it inherits. The `emRec`-side mutation tracking C++ uses is `emRecListener` (already ported in Rust at `emcore/src/emRecListener.rs`). The audit conflated two distinct mechanisms:
   - **emFileModel/emRecFileModel change signal** — fired on record load/save/explicit signal. This is what this bucket needs.
   - **emRec mutation listener** — `emRecListener` engine-callback when individual fields mutate. Already ported; orthogonal to this bucket.

   The correction: G2 is scoped to `emRecFileModel<T>`, not the broader emRec hierarchy. The downstream impact on emAutoplay and emVirtualCosmos is *that those models also extend `emRecFileModel`* (or its stand-in), and once `emRecFileModel<T>::GetChangeSignal` exists, those consumers can subscribe through the same delegating-accessor pattern. **No emRec-hierarchy port is needed for B-002.**

3. **`emFileLinkPanel-72`** — audit asks whether re-subscribe-on-`SetFileModel` needs a dedicated Rust setter or can be tied to model handle lifetime. Rust analog of `emFileLinkPanel::SetFileModel` is `set_link_model` (`emFileLinkPanel.rs:88`). Subscribing inside `set_link_model` is the natural port; the C++ remove-then-add pattern reduces in Rust to "the subscription is recorded in the engine when the panel is registered, and the engine handle is the same engine across model swaps, so the connect call inside `set_link_model` is sufficient." Removal is implicit: a dropped `SignalId` connection is harmless once the signal stops firing. (This is a per-panel call to `ectx.connect`, which is idempotent for the engine-id × signal-id pair — the engine ID is the panel's own ID, established at panel registration.)

These corrections do not move any rows out of B-002.

## Investigation: emRec hierarchy cross-bucket dependency

The bucket sketch flagged "emRec hierarchy cross-bucket dependency as the headline open question" — specifically whether the `emFileLinkModel-accessor-model-change` row blocks on out-of-bucket emRec infrastructure work, or is a within-bucket design concern.

**Verdict: within-bucket.**

Reasoning, from reading `emcore/src/emRecFileModel.rs` and `emcore/src/emFileModel.rs`:

- `emFileModel<T>` already owns `change_signal: SignalId` (line 117) and `GetChangeSignal()` returns it (line 64). It also fires it on file events (line 518).
- `emRecFileModel<T>` is a *separate* port that doesn't wrap `emFileModel<T>`. It owns load/save state (`FileState`) and rec parsing, but no signal field.
- The audit's framing — "emRec hierarchy lacks change-signal exposure" — is a misattribution. The signal lives on `emFileModel` in C++, and the Rust port simply forgot to add an analogous field to the standalone `emRecFileModel<T>`. There is no missing emRec-base-class infrastructure; the fix is a SignalId field plus accessor on `emRecFileModel<T>`, mirroring what `emFileModel<T>` already does.
- Other consumers of `emRecFileModel`-derived models (emAutoplay, emVirtualCosmos, emStocksFileModel) will *benefit* from the G2 add — once it lands, their delegating accessors become one-liners — but they are not blockers for B-002 and B-002 is not blocked by them. B-002 is independently completable.

**Operational consequence:** No cross-bucket prereq edge. B-001 and other emstocks/emfileman buckets can pick up the G2 accessor as a free win once B-002 lands; B-001 in particular can drop its "G1 delegating accessor" sketch in favor of inheriting from the now-signaled `emRecFileModel<T>`. Surface this to the working-memory session as an *opportunity*, not a prereq.

## Accessor groups

### G1 — `emTimer` wakeup signal on `emDirPanel` for idle key-walk clear — CLOSED (2026-05-01)

**Resolution.** Closed as behavioral equivalence. The C++ source for `ClearKeyWalkState` at `src/emFileMan/emDirPanel.cpp:350-355` is verified as purely `delete KeyWalkState; KeyWalkState=NULL;` — no paint, no invalidation, no observable side effect. Rust's lazy `Instant::now()` comparison in `emDirPanel::key_walk` produces the same observable trajectory on every input sequence: a >1 s idle followed by a keystroke starts a fresh key-walk search in both implementations. The internal "when" of the struct free is below the observable surface.

**Why the original "default: port" stance was wrong.** Adversarial-review finding 1 surfaced that the proposed `key_walk_timer: Option<emcore::emTimer::emTimer>` field cited a user struct (`emcore::emTimer::emTimer`) that does not exist — `crates/emcore/src/emTimer.rs` exposes only the `pub(crate) TimerCentral` and a `TimerId` handle, accessed via `Scheduler::create_timer(sig)`. The correct user-facing pattern is the `(SignalId, TimerId)` tuple seen at `emMainPanel.rs:1487-1490`. But once the C++ effect set is verified empty (finding 9), porting the timer adds zero observable behavior at the cost of new state and a new D-006 subscribe site — i.e. machinery for an effect that does not exist. Per Port Ideology, structure is not load-bearing here because the C++ effect is empty.

**Annotation.** No `DIVERGED:` is required; below-surface adaptations that preserve observable behavior are unannotated per CLAUDE.md "Annotation Vocabulary" (`IDIOM:` retired). A short prose comment at `emDirPanel::key_walk` already records the lazy-Instant equivalence and may be expanded to cite `emDirPanel.cpp:350-355` for traceability.

**Rows formerly depending on G1:**
- `emDirPanel-432` — re-classified as below-surface adaptation. Closed without code change. Working-memory session must update `inventory-enriched.json` to drop the row from B-002's actionable count (4 → 3 rows).

### G2 — `emRecFileModel<T>::GetChangeSignal()` (model-change broadcast)

**C++ source.** Inherited from `emFileModel`. `emRecFileModel.h:50` declares `const emSignal & GetChangeSignal() const;` "Signaled on every modification of the record." Fired by the `emRec` mutation hook that `emRecFileModel` installs at `PostConstruct`, plus on file load/save transitions inherited from `emFileModel::Signal()`.

**Rust state today.**
- `emcore::emFileModel<T>` already owns `change_signal` and exposes `GetChangeSignal()` returning `SignalId` (line 64). Fires it at `:518`.
- `emcore::emRecFileModel<T>` is structurally divergent from C++: standalone port that does not embed `emFileModel<T>` (per the explanatory comment on the struct itself, line 13–14). It owns its own `state: FileState`, `path`, `error_text`, etc. — but no `change_signal: SignalId`.
- `emfileman::emFileLinkModel` composes `emRecFileModel<emFileLinkData>` (line 8 import; data field not visible from the snippet but consistent with the FileModelState delegation at line 248–268). Exposes only `GetFileStateSignal` via `FileModelState` trait.

**Fix shape (amended 2026-05-01) — D-008 Cell-lazy allocation + D-007 mutator-fire threading.**

The original sketch widened `emRecFileModel::new(path)` to `new(path, change_signal: SignalId)`. Adversarial-review finding 2 demonstrated this is both policy-non-conforming (D-008 explicitly rejects "eager allocation by threading scheduler/SignalId through Acquire" in favour of A1, lazy `Cell<SignalId>`) **and** uncallable from the existing call sites: `emFileLinkModel::Acquire` (`crates/emfileman/src/emFileLinkModel.rs:202-206`) constructs through `ctx.acquire(name, || …)` whose factory closure has no `EngineCtx`; `emStocksFileModel::new(path)` (`crates/emstocks/src/emStocksFileModel.rs:28-34`) likewise has no scheduler access. The eager shape is therefore not viable.

**Adopted shape — D-008 canonical (mirrors B-009 `emFileManViewConfig.rs:365, 419-444`, B-014 `emVirtualCosmosModel`, and `vir_file_state_signal` in `emFilePanel.rs:67, 137, 145-149`):**

```rust
use std::cell::Cell;

pub struct emRecFileModel<T: Record + Default> {
    // ... existing
    /// Port of C++ inherited `emFileModel::ChangeSignal`. Lazy-allocated on
    /// first `GetChangeSignal(&self, ectx)` call per D-008 A1; null until then.
    /// `Cell<SignalId>` (not `SignalId`): allocated through `&self` in the
    /// combined-form accessor.
    change_signal: Cell<SignalId>,
}

impl<T: Record + Default> emRecFileModel<T> {
    /// Unchanged signature — `new(path)`. No ripple to call sites.
    pub fn new(path: PathBuf) -> Self {
        Self {
            // ... existing fields
            change_signal: Cell::new(SignalId::null()),
        }
    }

    /// Port of inherited C++ `emFileModel::GetChangeSignal`.
    /// Combined-form accessor: lazy-allocates on first call, returns the live id.
    pub fn GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId {
        let cur = self.change_signal.get();
        if cur.is_null() {
            let new_id = ectx.create_signal();
            self.change_signal.set(new_id);
            new_id
        } else {
            cur
        }
    }

    /// Port of C++ Signal-on-mutation. No-op when `change_signal` is null
    /// (matches C++ `emSignal::Signal()` with zero subscribers, per D-007 +
    /// D-008 composition note in decisions.md).
    pub fn signal_change(&self, ectx: &mut impl SignalCtx) {
        let sig = self.change_signal.get();
        if !sig.is_null() {
            ectx.fire(sig);
        }
    }
}
```

**`new(path)` keeps its current signature.** No call-site ripple. The two callers verified by `rg "emRecFileModel.*::new\("` (adversarial-review finding 11) — `emFileLinkModel.rs:204` and `emStocksFileModel.rs:30` — remain untouched.

**D-007 mutator-fire threading — explicit ripple enumeration (amended).** Adversarial-review finding 3 noted that the original sketch's `signal_change(&self, ectx)` proposal had no plan for *how `ectx` reaches each mutation site*. Today, `set_unsaved_state_internal` (`crates/emcore/src/emRecFileModel.rs:144-150`), `GetWritableMap` (`:52-62`), `TryLoad` (`:88-94`), `Save` (`:99-138`), `update` (`:139-149`), `hard_reset` (`:152-164`), and `clear_save_error` (`:167-172`) are all `&mut self` only — none accept `&mut impl SignalCtx`. Per D-007 ("Thread `&mut impl SignalCtx` through every mutator"), each must gain an `ectx: &mut impl SignalCtx` parameter so the trailing `self.signal_change(ectx)` is reachable.

Mutation-site enumeration (each must fire `change_signal` via `signal_change(ectx)`), expanded per finding 7 against `~/Projects/eaglemode-0.96.4/src/emCore/emFileModel.cpp` and `emRecFileModel.cpp:200` (the in-tree `Model.Signal(Model.ChangeSignal)` site for rec-mutation):

- `set_unsaved_state_internal` — Loaded/SaveError → Unsaved transition (called from `GetWritableMap`).
- `TryLoad` — Loading completion → Loaded transition.
- `Save` — Unsaved → Loaded transition (and Unsaved → SaveError transition; C++ fires both).
- `update` — Loaded → Waiting (out-of-date) and LoadError/TooCostly → Waiting transitions.
- `hard_reset` — any-state → Waiting transition (mirrors C++ `HardResetFileState`).
- `clear_save_error` — SaveError → Unsaved transition (mirrors C++ `ClearSaveError`).
- The `emRec` mutation hook installed in C++ `emRecFileModel::PostConstruct` (cpp:200). In Rust, the equivalent is the `emRecListener`-driven mutation callback already ported in `emcore/src/emRecListener.rs`; the `signal_change` call belongs at the listener-callback site once that path is wired.

Caller-side ripple (each gains an `ectx` parameter or threads through an existing one):
- `emFileLinkModel::ensure_loaded` (`crates/emfileman/src/emFileLinkModel.rs:240-245`) — currently `&mut self`; must become `(&mut self, ectx: &mut impl SignalCtx)`. Callers: `emFileLinkPanel::AutoExpand` (already operates inside `PanelCtx`, ectx-reachable).
- `emStocksFileModel::OnRecChanged` and the save-timer Cycle path (`crates/emstocks/src/emStocksFileModel.rs:42-49`) — already invoked from a Cycle context that owns ectx; thread through.
- Tests in `crates/emcore/tests/` and `crates/emfileman/tests/` that call any of the above mutators directly — update to pass a test ectx (existing pattern: `let mut sched = …; let mut ectx = ectx_for(...);`).

**Per D-007 single-callsite escape hatch.** Bootstrap-only callers — those reached only from inside `Acquire`'s factory closure where ectx is unavailable — keep their no-ectx signature with a `// CALLSITE-NOTE:` (B-014 precedent, decisions.md "Composition note (post-B-014)"): at bootstrap time `change_signal == SignalId::null()` so `signal_change` is a no-op anyway. Audit each ripple-affected call site to determine bootstrap-only vs post-Acquire; flag any post-Acquire site that genuinely lacks ectx for working-memory escalation.

**Delegating accessor on `emFileLinkModel` (combined form, D-008):**

```rust
impl emFileLinkModel {
    /// Port of inherited C++ `emFileModel::GetChangeSignal` via
    /// `emRecFileModel`. Combined-form delegation: lazy-allocates the
    /// underlying SignalId on first call.
    pub fn GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId {
        self.rec_model.GetChangeSignal(ectx)
    }
}
```

Note: `emRecFileModel`'s `FileModelState::GetFileStateSignal` (`crates/emcore/src/emRecFileModel.rs:290-294`) currently returns `SignalId::default()` with a comment that `emFilePanel::SetFileModel` does not use the signal. Per adversarial-review finding 10, once G2 lands, `emFileLinkModel::GetFileStateSignal` becomes a real subscriber via the panel wiring and the trait-level comment must be updated or removed; record this as a follow-up cleanup.

**Rows depending on G2:**
- `emFileLinkPanel-56` (initial subscribe in panel constructor / first-Cycle).
- `emFileLinkPanel-72` (re-subscribe on `set_link_model`).
- `emFileLinkModel-accessor-model-change` (accessor add itself).

## Per-panel consumer wiring

### emDirPanel (0 rows after amendment)

Closed per § Accessor groups → G1. `emDirPanel.cpp:350-355` confirms `ClearKeyWalkState` is purely internal-state cleanup with no observable side effect; the Rust lazy-Instant scheme is observably equivalent. No code change in this bucket. Working-memory session updates `inventory-enriched.json` to drop row `emDirPanel-432` from B-002's actionable count.

### emFileLinkPanel (1 shared callsite covering rows -56 and -72)

**Row collapse (amended 2026-05-01).** Adversarial-review finding 5: C++ `emFileLinkPanel.cpp:55` (ctor) and `:72` (in `SetFileModel`) are textually the same statement — `if (Model) AddWakeUpSignal(Model->GetChangeSignal());`. The Rust port constructs with `model: None` (`crates/emfileman/src/emFileLinkPanel.rs:81`) and the only path to a non-null model is `set_link_model`. The ctor branch is therefore structurally dead in Rust, and rows `-56` and `-72` resolve to a *single* Rust callsite: the deferred subscribe inside the existing first-Cycle init block. Update the row table accordingly.

**Out-of-scope subscriptions are already wired (finding 4).** B-005 has already installed both `Config::GetChangeSignal` (`crates/emfileman/src/emFileLinkPanel.rs:200-204`) and `UpdateSignalModel->Sig` (`:217-220`) inside the existing `subscribed_init` block at `:71, :88, :186-196`. `VirFileStateSignal` belongs to B-004. The original design's "audit gap" framing is dropped.

**Merge target.** The model-change branch is added inside the *existing* B-005 first-Cycle init block at `crates/emfileman/src/emFileLinkPanel.rs:186-196`. **Do not** introduce a second `subscribed_init` flag.

**Add to the panel struct (one new flag, not two):**
```rust
pub struct emFileLinkPanel {
    // ... existing (subscribed_init already present from B-005 at line 71)
    /// Tracks whether the current `model` has been subscribed. Reset on
    /// `set_link_model` to re-run the connect for the new model. Distinct
    /// from `subscribed_init`, which guards the panel-lifetime first-Cycle
    /// init (config + UpdateSignalModel subscriptions).
    model_subscribed: bool,
}
```

**Connect flow (merged into B-005's existing block):**
1. `set_link_model` (`:92-99`) sets `self.model = Some(...)` and *also* sets `self.model_subscribed = false`. (Covers rows -56 and -72; see below for the `DIVERGED:` annotation.)
2. Inside the existing first-Cycle init block (`:186-196`), append: if `self.model.is_some() && !self.model_subscribed`, call `model.borrow().GetChangeSignal(ectx)` (combined-form, lazy-allocates), then `ectx.connect(chg_sig, eid)`, then set `model_subscribed = true`.
3. Top-of-Cycle (after the existing config/UpdateSignal IsSignaled branches at `:200-220`): `if let Some(m) = &self.model { let s = m.borrow().GetChangeSignal(ectx); if !s.is_null() && ectx.IsSignaled(s) { self.needs_update = true; } }`. Mirrors C++ `DirEntryUpToDate=false; doUpdate=true;`.

The existing `needs_update: bool` and `update_data_and_child_panel` are reused. Combined-form re-call across init and IsSignaled is idempotent (B-014 precedent, cited at `:200`).

**`DIVERGED: language-forced` annotation at `set_link_model` (finding 6).** `set_link_model` (`emFileLinkPanel.rs:92-99`) has no `&mut EngineCtx` parameter. In C++, `SetFileModel` runs synchronously through the engine via `this`, so `AddWakeUpSignal` fires immediately; in Rust, the connect is deferred to the next Cycle (gated on `model_subscribed`). This imports a one-tick observable delay relative to C++ — the same shape B-004 / B-015 chose with `pending_vir_state_fire` (`crates/emcore/src/emFilePanel.rs:155, 508-509`). Add the following annotation at the top of `set_link_model`:

```rust
// DIVERGED: language-forced. C++ emFileLinkPanel::SetFileModel calls
// AddWakeUpSignal(Model->GetChangeSignal()) synchronously via `this`; the
// engine handle is reachable from any member function. The Rust signature
// `set_link_model(&mut self, ...)` lacks an EngineCtx parameter (Acquire
// closures and panel mutators do not own ectx), so the connect is deferred
// to the next Cycle through `model_subscribed: bool`. D-006 option-B
// (deferred connect) localized to model-set; B-004 / B-015
// `pending_vir_state_fire` precedent at emFilePanel.rs:155, 508-509.
```

`cargo xtask annotations` will require this tag to carry the `language-forced` category; the precedent citation satisfies the lint.

**Rows covered by this section:** `emFileLinkPanel-56`, `emFileLinkPanel-72` — single Rust callsite. `emFileLinkModel-accessor-model-change` lands as the G2 delegating accessor in `emFileLinkModel.rs`.

## Sequencing

**Within the bucket:**

1. **Land G2 accessor add first** (emRecFileModel `Cell<SignalId>` field + combined-form `GetChangeSignal(&self, ectx)` + `signal_change(&self, ectx)` + delegating accessor on `emFileLinkModel`). `new(path)` signature unchanged (D-008 A1) — no call-site arg ripple. The actual ripple is the D-007 mutator-fire ectx-threading (enumerated in § Accessor groups → G2). Ship behind a behavioral test that fires `signal_change` and asserts a subscriber wakes. **Pre-merge audit:** rg `emRecFileModel.*::new\(` returns exactly two callers — `crates/emfileman/src/emFileLinkModel.rs:204` and `crates/emstocks/src/emStocksFileModel.rs:30` (verified, finding 11); no signature change needed at either site.
2. ~~Land G1 emTimer port on emDirPanel~~ — closed as behavioral equivalence (G1 ships zero rows; see § Accessor groups → G1).
3. **Land emFileLinkPanel consumer wiring** — single shared callsite covering rows -56 and -72. Depends on G2. Merges into the *existing* B-005 first-Cycle init block at `emFileLinkPanel.rs:186-196`; introduces only one new field (`model_subscribed: bool`) and a `DIVERGED: language-forced` annotation at `set_link_model`.

**Cross-bucket prereqs.** None outbound (B-002 has no upstream prereq). Outbound *opportunity*: B-001 (emstocks) can simplify its G1 delegating accessor for `emStocksFileModel::GetChangeSignal` by inheriting through the now-signaled `emRecFileModel<T>` once B-002 lands. Not a blocker for B-001, just a simplification. Flag for working-memory session reconciliation.

**Cross-bucket impact (downstream).** Other models composing `emRecFileModel<T>` (emAutoplay, emVirtualCosmos, emStocksFileModel, plus any not yet enumerated) gain `GetChangeSignal` through delegation once G2 lands. None of those are bucket-coupled to B-002; they consume the G2 accessor independently in their own buckets.

## Verification strategy

**Behavioral tests.** Two new files (or one combined):

- `crates/emcore/tests/emrecfilemodel_change_signal.rs` — fire `signal_change(ectx)` on a fresh `emRecFileModel<TestRec>` after a subscribe; assert the subscriber wakes via the engine. Cover all six mutator entry points enumerated in G2 (`set_unsaved_state_internal` via `GetWritableMap`, `TryLoad` completion, `Save` Loaded/SaveError transitions, `update` out-of-date, `hard_reset`, `clear_save_error`). Pre-subscribe call (`signal_change` while `change_signal == SignalId::null()`) must be a no-op (D-007 + D-008 composition).
- `crates/emfileman/tests/typed_subscribe_b002.rs` — for `emFileLinkPanel`, fire the model's change signal via `model.borrow().rec_model.signal_change(ectx)`; run Cycle; assert `needs_update == true`. Also cover the row -72 path: construct panel with model A; subscribe via first-Cycle; fire A's signal; assert wake. Call `set_link_model(B)`; advance one Cycle to re-subscribe; fire B's signal; assert wake. Fire A's signal *after* swap; assert no wake (panel's `if let Some(m) = &self.model` guard handles this — only the current model's IsSignaled branch runs).
- `emDirPanel` test removed: G1 closed without code change; the existing lazy-Instant behavior is exercised by current emDirPanel tests.

For `emFileLinkPanel-72` (re-subscribe on model swap): construct panel with model A; fire A's signal; assert wake. Call `set_link_model(B)`; fire B's signal; assert wake. Fire A's signal *after* swap; assert no spurious wake (or accept idempotent waking — the C++ contract is: only the current model fires a meaningful event; the Cycle body's `if let Some(m) = &self.model` guard handles this regardless).

**No new pixel goldens.** The drift surface is signal flow; existing emfileman goldens (if any cover emFileLinkPanel) remain the regression backstop for paint output.

**Annotation checks.** The `emRecFileModel<T>` standalone-port comment becomes more nuanced (still standalone for state, but signal-aware). If the existing comment doesn't already carry an annotation, leave it as a prose comment (per the retired `IDIOM:` rule). If it does carry a `DIVERGED:` tag, update the body but the tag remains valid (the standalone-port choice was a language-forced divergence under the former emFileModel-wrapping scheme). Run `cargo xtask annotations` after edits.

## Open items deferred to working-memory session

1. **Reconcile audit-data corrections** into `inventory-enriched.json`:
   - `emFileLinkModel-accessor-model-change`: replace "emRec hierarchy lacks change-signal exposure" with "emRecFileModel<T> standalone port lacks change-signal field; fix is local to `emRecFileModel<T>` plus delegating accessor."
   - Cross-reference: drop the "(also affects emAutoplay, emVirtualCosmos)" note as a *prereq* — re-tag as a *downstream beneficiary* opportunity once G2 lands.
2. ~~**`emDirPanel-432` observable-equivalence question.**~~ Resolved 2026-05-01 amendment: `src/emFileMan/emDirPanel.cpp:350-355` confirms `ClearKeyWalkState` is purely internal cleanup. Row closed as below-surface adaptation; no code change. Working-memory session must update `inventory-enriched.json` to drop the row.
3. **`set_link_model`-driven subscribe (row -72) is a D-006 option-B style local override** (deferred-queue at model-set rather than panel-construct). This is a *new local pattern* — not yet a global override of D-006. If a second bucket rediscovers it, propose D-007. Not proposing now.
4. ~~**emFileLinkPanel out-of-scope subscriptions.**~~ Resolved 2026-05-01 amendment: B-005 has already wired both `Config::GetChangeSignal` (`emFileLinkPanel.rs:200-204`) and `UpdateSignalModel->Sig` (`:217-220`); `VirFileStateSignal` lives in B-004. Not an audit gap. The B-002 design merges into the existing B-005 `subscribed_init` block rather than introducing a second one.
5. **Other emRecFileModel composers (emAutoplay, emVirtualCosmos, emStocksFileModel) get `GetChangeSignal` for free once G2 lands.** B-001's G1 delegating accessor sketch can simplify post-B-002. Communicate this to the B-001 implementer.

## Proposed new D-### entries

**None.** After amendment, all 3 actionable rows fit existing decisions (D-003 fill-in-scope, D-006 subscribe-shape with option-B local override, D-007 mutator-fire ectx-threading, D-008 A1 Cell-lazy SignalId). The `set_link_model`-driven subscribe is a within-D-006 option-B local variant (precedent: B-004 / B-015 `pending_vir_state_fire`), not a new global decision.

## Success criteria

- The 3 actionable rows after G1 closure (`emFileLinkPanel-56`, `emFileLinkPanel-72`, `emFileLinkModel-accessor-model-change`) collapse to a single shared Rust callsite (rows -56 and -72) plus the G2 delegating accessor.
- The shared callsite has a `connect(...)` call inside the existing B-005 `subscribed_init` block at `crates/emfileman/src/emFileLinkPanel.rs:186-196`, gated by `model_subscribed` so it re-runs after `set_link_model`.
- `emFileLinkPanel::Cycle` has an `IsSignaled(model.GetChangeSignal(ectx))` branch alongside the existing config and UpdateSignalModel branches; the branch sets `self.needs_update = true`.
- `emRecFileModel<T>` carries a `change_signal: Cell<SignalId>` field (default `SignalId::null()`), a combined-form `GetChangeSignal(&self, ectx: &mut impl SignalCtx)` accessor (D-008 A1), and a `signal_change(&self, ectx: &mut impl SignalCtx)` mutator that no-ops on null. `new(path)` signature unchanged.
- D-007 ectx-threading applied at every mutator enumerated in G2 (`set_unsaved_state_internal`, `GetWritableMap`, `TryLoad`, `Save`, `update`, `hard_reset`, `clear_save_error`); each fires `signal_change(ectx)` after its state transition. Caller ripple at `emFileLinkModel.rs:240-245` and `emStocksFileModel.rs:42-49` updated; bootstrap-only sites carry `// CALLSITE-NOTE:` per B-014 precedent.
- `emFileLinkModel::GetChangeSignal(&self, ectx)` exists as a one-line combined-form delegating accessor.
- `set_link_model` carries a `DIVERGED: language-forced` annotation citing D-006 option-B and the B-004 `pending_vir_state_fire` precedent (`emFilePanel.rs:155, 508-509`).
- `emDirPanel-432` is closed without code change (behavioral equivalence per `emDirPanel.cpp:350-355`); inventory updated to reflect 3 actionable rows in B-002.
- `emRecFileModel`'s `FileModelState::GetFileStateSignal` stale comment at `emRecFileModel.rs:290-291` is updated/removed (finding 10 follow-up).
- `cargo clippy -D warnings`, `cargo-nextest ntr`, and `cargo xtask annotations` pass.
- New behavioral tests in `crates/emcore/tests/emrecfilemodel_change_signal.rs` and `crates/emfileman/tests/typed_subscribe_b002.rs` cover the 3 actionable rows including pre-subscribe no-op and post-`set_link_model` re-subscribe.
- B-002 status in `work-order.md` flips `pending → designed`.

---

## Adversarial Review — 2026-05-01

### Summary
- Critical (would break implementation): 3
- Important (would cause rework): 4
- Minor (nits / clarity): 2
- Notes (informational): 2

### Findings

1. **[Critical] G1 / emDirPanel-432 — `emcore::emTimer::emTimer` does not exist.** The fix shape (line 72) declares `key_walk_timer: Option<emcore::emTimer::emTimer>`, but `crates/emcore/src/emTimer.rs` exposes only `TimerCentral` (pub(crate), accessed via `Scheduler`) and a `TimerId` handle (`emTimer.rs:8-9, 22, 34`). There is no `emTimer` user struct with `GetSignal()`. This is the same audit-data error called out for B-017 in work-order.md:151 ("bucket sketch's 'emTimer::TimerCentral unported' framing is stale; TimerCentral is ported"). **Fix:** rewrite G1 against the established consumer pattern in `emMainPanel.rs:726, 1487, 1817, 1883` and `emMiniIpc.rs:350` — `let sig = ectx.create_signal(); let tid = ectx.scheduler.create_timer(sig);`, store `(TimerId, SignalId)`, restart via `scheduler.start_timer(tid, 1000, false)`, subscribe `ectx.connect(sig, eid)` in first-Cycle init. The lazy-creation framing is fine, but the type and call surface must match the actual API.

2. **[Critical] G2 — proposed `new(path, change_signal: SignalId)` violates D-008.** The design (lines 105–106) widens `emRecFileModel::new` to accept a SignalId at construction. This is option A2 in D-008 (eager allocation by threading scheduler/SignalId through Acquire), which D-008 explicitly rejects in favour of A1 (lazy `Cell<SignalId>` allocated on first `GetXxxSignal(&self, ectx)` call). Worse, `emFileLinkModel::Acquire` (`emFileLinkModel.rs:202-206`) and `emStocksFileModel::new` (`emStocksFileModel.rs:28-34`) construct `emRecFileModel` from contexts that have **no `EngineCtx`** — `ctx.acquire(name, || …)` is a `FnOnce()` factory closure, and `emStocksFileModel::new(path)` likewise has no scheduler access. The proposed signature is therefore not just policy-non-conforming but **uncallable from the existing call sites** without a much larger ripple. **Fix:** adopt the canonical D-008 shape — `change_signal: Cell<SignalId>` initialised null, combined-form `pub fn GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` that lazy-allocates, mutator helper `pub fn signal_change(&self, ectx: &mut impl SignalCtx)` that no-ops when null. `new(path)` keeps its current signature; the ripple disappears. Pattern precedent: `emFileManViewConfig.rs:365, 419-444` (B-009) and the `vir_file_state_signal` allocation in `emFilePanel.rs:67, 137, 145-149`.

3. **[Critical] G2 mutator-fire shape missing — D-007 not cited or applied.** The design enumerates four mutation sites (load completion, save completion, `set_unsaved_state_internal`, error transitions) but proposes a single `signal_change(&self, ectx)` helper with no plan for **how `ectx` reaches each site**. `emRecFileModel::set_unsaved_state_internal` is called from `GetWritableMap` (`emRecFileModel.rs:52-62`); both have `&mut self` only — no ectx. `TryLoad` / `Save` / `update` likewise are pure `&mut self`. Per D-007 ("Thread `&mut impl SignalCtx` through every mutator"), every mutator from `set_unsaved_state_internal` outward must take `&mut impl SignalCtx`, which forces `GetWritableMap`, `TryLoad`, `Save`, `update`, `clear_save_error`, `hard_reset` to all gain that parameter — a much larger ripple than the design admits. **Fix:** explicitly enumerate the mutator-API change and audit every caller (in `emFileLinkModel.rs:240-245`, `emStocksFileModel.rs:42-49`, plus tests). Confirm `ensure_loaded` and `OnRecChanged` callers can supply ectx; flag any that cannot per D-007's "single callsite that genuinely lacks ectx → per-callsite hybrid" escape hatch.

4. **[Important] Out-of-scope claim for G1's surrounding subscriptions is wrong.** Lines 171–172 say `UpdateSignalModel`, `VirFileStateSignal`, and `Config::ChangeSignal` are "out of scope for B-002 … the audit didn't catch them in this bucket". In fact B-005 already wired both `Config::GetChangeSignal` and `UpdateSignalModel->Sig` in `emFileLinkPanel::Cycle` (`emFileLinkPanel.rs:184-220`); `subscribed_init` already exists at `:71, :88, :186-196`. `VirFileStateSignal` belongs to B-004 (work-order.md:130, 149). **Fix:** delete the "audit gap" framing; the design must instead extend the existing B-005 first-Cycle init block (not introduce a new one) and add only the model-change branch. Otherwise the implementer will collide with the live `subscribed_init` field.

5. **[Important] Row -56 / Row -72 are the *same* C++ statement.** `emFileLinkPanel.cpp:55` (ctor) and `:72` (in `SetFileModel`) are both `if (Model) AddWakeUpSignal(Model->GetChangeSignal());`. The Rust port already constructs the panel with `model: None` (`emFileLinkPanel.rs:81`); `set_link_model` is the only place the model becomes non-null. Therefore there is no "ctor subscribe" and "SetFileModel subscribe" distinction in Rust — the ctor branch is structurally dead. **Fix:** collapse to a single `model_subscribed: bool` reset on `set_link_model` and re-asserted in first-`Cycle`-after-set (your own design says this on line 187, but the row table and § "Connect flow" still pretend they're two distinct subscribes). Document that row -56 and -72 share one Rust call site by construction.

6. **[Important] `set_link_model` mutates `subscribed_init`-adjacent state without firing ectx.** `set_link_model` (`emFileLinkPanel.rs:92-99`) has no `&mut EngineCtx`. The design's "set `model_subscribed = false` here, connect on next Cycle" pattern is correct, but it imports a one-tick observable delay relative to C++'s synchronous `AddWakeUpSignal` in `SetFileModel`. This is the B-004 / B-015 precedent: `language-forced` deferred subscribe with a `pending_*` flag. **Fix:** add an explicit `DIVERGED: language-forced` annotation at `set_link_model` citing D-006 option-B and the B-004 `pending_vir_state_fire` precedent (`emFilePanel.rs:155, 508-509`). Do not rely on prose alone — annotation lint will require the tag.

7. **[Important] Mutation-site enumeration is incomplete.** Line 122–126 lists four site classes but misses: `hard_reset` (`emRecFileModel.rs:152-164`) — clears data and forces Waiting; the C++ analogue fires change. `update` (`:139-149`) — when `is_out_of_date()` triggers `hard_reset_data` + transition to Waiting. `clear_save_error` (`:167-172`) — SaveError → Unsaved transition. C++'s `emFileModel::Update`, `HardResetFileState`, and `ClearSaveError` all fire the change signal. **Fix:** read `~/Projects/eaglemode-0.96.4/src/emCore/emFileModel.cpp` for every `Signal(ChangeSignal)` callsite and mirror exhaustively. Existing precedent: `emFileModel.rs:518` already fires on the corresponding transitions in the wrapped form.

8. **[Minor] D-009 relevance not analysed.** B-002 introduces no polling intermediary, but the design should explicitly state "no D-009 polling-intermediary topology in this bucket" so downstream reviewers don't have to re-derive it. Trivial add.

9. **[Minor] `ClearKeyWalkState` C++ effects already verifiable from cpp:350-355.** Design line 64 punts on whether ClearKeyWalkState has observable effects, but the source (already grep-visible at `emDirPanel.cpp:350-355`) shows it is purely `delete KeyWalkState; KeyWalkState=NULL;` — no paint or invalidation. Per the design's own success-criteria branch (a) vs (b), this is the (b) "behavioral parity, row closed without code change" case. **Recommendation:** flip the default from "port the timer" to "close as below-surface adaptation, document equivalence", reducing G1 to zero rows. This also removes the dead-row risk of porting timer infrastructure with no observable consumer.

10. **[Note]** emRecFileModel implements `FileModelState::GetFileStateSignal` returning `SignalId::default()` (`emRecFileModel.rs:290-294`) with a comment that "emFilePanel::SetFileModel does not use the signal." Once G2 lands, `emFileLinkModel::GetFileStateSignal` (which delegates to `rec_model`) becomes a real subscriber. The trait-level comment at `emRecFileModel.rs:290-291` becomes stale and must be updated or removed.

11. **[Note]** Design's "audit Task 6 / cargo bench" `rg` command on line 131 (`rg "emRecFileModel.*::new\("`) returns exactly two callers: `emFileLinkModel.rs:204` and `emStocksFileModel.rs:30`. Confirmed; no other callers in tree. Helpful to record the closed enumeration upfront.

### Recommended Pre-Implementation Actions

1. **Rewrite G1 against the actual `TimerCentral` API.** Before any code, settle the equivalence question (finding 9): if `ClearKeyWalkState` is purely state cleanup, close emDirPanel-432 with a behavioral-equivalence note and skip the timer port entirely. Otherwise, mirror `emMainPanel.rs:1487` for the timer-creation pattern.
2. **Replace G2's `new(path, signal_id)` shape with the D-008 canonical Cell-lazy form** before writing any production code. Audit `set_unsaved_state_internal` callers and decide D-007 ectx-threading scope (finding 3) — likely requires `GetWritableMap(&mut self, ectx)`, `TryLoad(&mut self, ectx)`, `Save(&mut self, ectx)`, etc.; enumerate mutator callsites in `emFileLinkModel.rs` and `emStocksFileModel.rs` and confirm ectx availability at each.
3. **Read the actual `emFileLinkPanel::Cycle` body** (`emFileLinkPanel.rs:179-224`) to merge into the *existing* B-005-installed first-Cycle init block. Do not introduce a second `subscribed_init` flag.
4. **Read `emFileModel.cpp`** for the full `Signal(ChangeSignal)` callsite set and re-do the mutation-site enumeration (finding 7).
5. **Add `DIVERGED: language-forced` annotation plan** for the deferred `set_link_model` subscribe, citing D-006 option-B and B-004 precedent (finding 6).
6. **Confirm row -56 / -72 collapse** — update the row table to note shared Rust callsite (finding 5).

---

## Amendment Log — 2026-05-01

Each entry resolves one adversarial-review finding by editing the design body. Original Adversarial Review section preserved verbatim above.

1. **[Critical, finding 1]** G1 — closed as behavioral equivalence. C++ `ClearKeyWalkState` at `~/Projects/eaglemode-0.96.4/src/emFileMan/emDirPanel.cpp:350-355` is purely internal cleanup. Edited Goal-and-scope, § Accessor groups → G1, § Per-panel → emDirPanel, § Sequencing, § Success criteria, § Open items #2. Removes the non-existent `emcore::emTimer::emTimer` user struct reference; `TimerCentral` API surface no longer needed. Row `emDirPanel-432` re-classified.
2. **[Critical, finding 2]** G2 — replaced `new(path, change_signal: SignalId)` with D-008 A1 Cell-lazy `Cell<SignalId>` field + combined-form `GetChangeSignal(&self, ectx: &mut impl SignalCtx)`. `new(path)` signature unchanged; ripple to `emFileLinkModel::Acquire` and `emStocksFileModel::new` eliminated. Edited § Accessor groups → G2 fix shape; § Cited decisions; § Sequencing.
3. **[Critical, finding 3]** D-007 mutator-fire shape now explicitly enumerates the ripple: `set_unsaved_state_internal`, `GetWritableMap`, `TryLoad`, `Save`, `update`, `hard_reset`, `clear_save_error` each gain `&mut impl SignalCtx`; caller ripple at `emFileLinkModel.rs:240-245` and `emStocksFileModel.rs:42-49` enumerated; B-014 bootstrap-only escape hatch cited. Edited § Accessor groups → G2; § Cited decisions; § Success criteria.
4. **[Important, finding 4]** Out-of-scope claim corrected: B-005 already wired Config and UpdateSignalModel at `emFileLinkPanel.rs:200-220` inside the existing `subscribed_init` block at `:71, :88, :186-196`. Edited § Per-panel → emFileLinkPanel; § Open items #4.
5. **[Important, finding 5]** Rows -56 and -72 collapsed to a single shared Rust callsite (ctor branch is structurally dead because `model: None` at construction). Edited § Per-panel → emFileLinkPanel header and connect flow; § Success criteria; § Sequencing.
6. **[Important, finding 6]** Added `DIVERGED: language-forced` annotation plan at `set_link_model` citing D-006 option-B and B-004 / B-015 `pending_vir_state_fire` precedent (`emFilePanel.rs:155, 508-509`). Edited § Per-panel → emFileLinkPanel; § Success criteria.
7. **[Important, finding 7]** Mutation-site enumeration expanded to include `update`, `hard_reset`, `clear_save_error` (verified against C++ `emFileModel.cpp` and `emRecFileModel.cpp:200`). Edited § Accessor groups → G2; § Verification.
8. **[Minor, finding 8]** D-009 explicitly noted as not applicable (the only `Cell` is the D-008 SignalId slot, not a polling intermediary). Edited § Cited decisions.
9. **[Minor, finding 9]** Folded into resolution 1: `ClearKeyWalkState` C++ effect set verified empty; G1 default flipped from "port" to "close". Edited § Accessor groups → G1.
10. **[Note, finding 10]** Stale `FileModelState::GetFileStateSignal` comment at `emRecFileModel.rs:290-291` flagged for update/removal as a follow-up cleanup. Recorded in § Accessor groups → G2 and § Success criteria.
11. **[Note, finding 11]** Closed enumeration of `emRecFileModel::new` callers (two: `emFileLinkModel.rs:204`, `emStocksFileModel.rs:30`) recorded in § Sequencing #1.

**Net delta.** Bucket actionable rows: 4 → 3 (G1 closed). New struct fields: 1 (`emFileLinkPanel.model_subscribed`); merges into existing `subscribed_init`. New annotations: 1 (`DIVERGED: language-forced` at `set_link_model`). API ripple: D-007 ectx-threading at six `emRecFileModel` mutators; no `new()` signature changes. Cited decisions extended from {D-003, D-006} to {D-003, D-006, D-007, D-008, D-009-not-applicable}.
