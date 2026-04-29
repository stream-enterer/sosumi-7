# Signal-Drift Remediation — Global Decisions

ADR-style entries resolving the cross-cutting decisions enumerated in `decision-catalog.md`. Each entry is at sketch resolution — enough to be a stable citation target for bucket sketches, not a full design. Per-bucket brainstorms refine and may amend.

Stable IDs (`D-###`) are referenced from `inventory-enriched.json` and from `buckets/B-###-<slug>.md`. When a bucket-design session needs to revisit a decision, the working-memory session updates this file and back-propagates citations.

---

## D-001-typemismatch-accessor-policy

**Question:** For accessors returning `u64` where `SignalId` is expected (all in emfileman: `emFileManModel::GetSelectionSignal`, `emFileManModel::GetCommandsSignal`, `emFileManViewConfig::GetChangeSignal`), do we flip the accessor type or adapt consumers?

**Affects:** 14 P-003 rows + 1 P-009 cleanup item (`emFileModel.rs:490`). Cited by every bucket touching emfileman type-mismatch consumers and by the emFileManModel accessor bucket.

**Options considered:**
- **A. Flip accessor to `SignalId`.** Matches C++ contract. Touches all 11 consumers but mechanically (replace `u64` polling with `subscribe(signal_id)`). Aligns with audit Task 6 finding that the `u64` convention has no forced-category justification.
- **B. Keep `u64`, adapt consumers.** Preserves existing Rust port choice. Requires bespoke per-consumer adaptation; doesn't scale and silently ratifies the unannotated divergence.
- **C. Hybrid.** Flip accessor in fileman but keep a `u64` adapter for already-adapted consumers. Maximum churn, no clear benefit.

**Chosen direction:** **A. Flip accessor to `SignalId`.**

**Why:** Per Port Ideology, divergence without forced category is fidelity-bug. The `u64` convention is unannotated and the audit re-validation found no language-forced, dependency-forced, upstream-gap-forced, or performance-forced justification. Flipping is the canonical fix; the consumer migration is mechanical because all 11 consumers currently poll the same `u64` accessor pattern.

**Open questions deferred to bucket design:**
- Whether to flip accessor + migrate consumers in a single PR or stage (accessor-flip + per-consumer follow-ups). Likely single PR given mechanical-ness; bucket sketcher decides.
- Whether `emFileManViewConfig::GetChangeSignal` and `emFileManModel::GetSelectionSignal/GetCommandsSignal` flip in one bucket or one bucket per accessor. Depends on row-count split (Phase 3 clustering).

---

## D-002-rc-shim-policy

**Question:** For consumers using `Rc<RefCell<>>` / `Rc<Cell<>>` shared state in click-handler closures instead of subscribing to a signal, is conversion to signal-subscribe always correct, or are there load-bearing cases that justify keeping the shim?

**Affects:** P-004 (33 rows: emcore=15, emmain=14, emstocks=4) + P-005 (1 row: emAutoplay-1172 only). Cited by every bucket containing rc-shim rows. (Was P-004=29 + P-005=6 originally; B-013 moved 4 emstocks dialog-result rows P-005 → P-004; B-014 reclassified emVirtualCosmos-575 P-005 → P-001 after verifying its `Rc<RefCell<Model>>` is just a routine model handle, not a click-handler shim.)

**Options considered:**
- **A. Always convert.** Treat every rc-shim consumer as drift; replace with signal-subscribe.
- **B. Triage per row.** Keep shim where it's load-bearing for cross-panel coordination or dialog-result handoff; convert otherwise.
- **C. Always keep.** Treat the shim as a Rust idiom adaptation. Rejected on Port Ideology grounds: shim observably changes timing (closures fire vs signals fire), so it's not below-surface adaptation.

**Chosen direction:** **B. Triage per row, defaulting to convert.**

**Why:** The audit captured snippets show genuine variation. Most emCoreConfigPanel rows are pure drift (a button press should fire a signal that other panels observe; the shim hides the signal from observers entirely). But the emstocks dialog-result Cells (`cut_stocks_result`, `paste_stocks_result`, `delete_stocks_result`) are accepting a value out of a one-shot dialog callback — the underlying contract is "post-finish dialog handoff," and the C++ equivalent uses a member field set in `Cycle()` after `IsFinished()`. That's not a signal-shim; the shim *is* the contract. Per-row triage is the only honest call.

**Decision rule for the per-row triage** (so bucket designers don't re-litigate it):
1. **Convert** if the C++ original uses a signal accessor and a subscribe at the consumer site.
2. **Keep** if the C++ original uses a member field assigned post-finish (dialog-result pattern) or post-cycle.
3. **Escalate to working-memory session** if neither rule cleanly applies.

**Open questions deferred to bucket design:**
- ~~The emAutoplay flags-passing pattern (`AutoplayFlags { progress: Rc<Cell<f64>> }`) — does this fall under rule 1 or rule 2? emAutoplay has no C++ analogue (it's a Rust-only panel) so the rule needs adaptation. Bucket sketcher flags this for the working-memory session to resolve before bucket execution.~~ **Resolved by B-003 brainstorm 703fa462 + working-memory ratification (2026-04-27):** B-003 designer found that `AutoplayFlags`'s seven inbound `Cell`s are produced but never consumed (existing DIVERGED annotation at `emAutoplayControlPanel.rs:84` claiming "polled by parent panel" is factually wrong; only test sites read them). **Resolution R-A: drop AutoplayFlags entirely.** Matches C++ (which has no AutoplayFlags pattern); removes a no-forced-category DIVERGED block; consistent with this entry's default-convert rule. The single load-bearing piece — outbound `progress: Rc<Cell<f64>>` driving `AutoplayCheckButtonPanel`'s paint — replaces with `Rc<RefCell<emAutoplayViewModel>>` on the check-button panel reading `GetItemProgress()` in `Paint`. emAutoplay is consequently *not* an exception to rule 1 — it had no C++ analogue because the original divergence was unjustified, not because of a structural constraint.

---

## D-003-gap-blocked-fill-vs-stub

**Question:** For the 16 gap-blocked rows where the upstream-gap is the blocker, do we fill the gap (port the missing infrastructure) or stub at the consumer (no-op signal subscription that becomes live when the gap fills)?

**Affects:** all gap-blocked rows: 10 P-001 + 0 P-002 + 3 P-003 = 13 rows. (Was 16 originally; reduced by B-006 reclassifying `emMainControlPanel-218`, B-007 reclassifying `emFileSelectionBox-64`, and B-008 reclassifying `emMainPanel-69` — all audit-time stale gap-blocked tags where the accessor actually exists. Pattern: every reclassification was a P-002 row whose accessor existed at audit time but was tagged missing. The remaining 13 gap-blocked rows are all P-001/P-003.)

**Options considered:**
- **A. Fill the gap.** Port the missing upstream model/accessor as part of each gap-blocked bucket.
- **B. Stub at consumer.** Wire the consumer to subscribe to a placeholder `SignalId` that the gap-fill PR later activates. Visible drift remains until gap fills.
- **C. Defer entirely.** Mark gap-blocked rows as not-actionable until the gap closes; remove from this remediation effort.

**Chosen direction:** **A. Fill the gap, scoped per bucket.**

**Why:** "Gap-blocked" is the audit's term for "the C++ accessor exists but the Rust accessor doesn't and that's why the consumer can't subscribe." The fix shape is: port the accessor, then wire the consumer. Both halves live in the same scope (the model file), so the bucket containing the gap-blocked consumer is the natural home for the gap-fill. C is rejected because it kicks the can; B is rejected because it adds a placeholder API surface that has to be torn down later.

**Open questions deferred to bucket design:**
- Some gap-blocked rows reference accessors whose underlying *model* infrastructure isn't ported (e.g., emVirtualCosmosModel). Per-bucket sketcher checks: is the gap a missing accessor on a ported model (fill in scope) or a missing model entirely (escalate — bucket cannot complete without out-of-scope porting)?

---

## D-004-stocks-application-strategy

**Question:** ~78 emstocks rows likely repeat the same drift across panels (emStocksControlPanel: 37, emStocksItemPanel: 25, emStocksListBox: 7, others: 9). Do we design the fix once and apply mechanically across all stocks panels (single design, batch PR), or design per-panel?

**Affects:** all 78 emstocks rows.

**Options considered:**
- **A. Design-once, apply-mechanically.** One bucket per pattern-id, scoped to emstocks; the pattern-fix is canonical and applied across all stocks files.
- **B. Per-panel buckets.** Each emstocks file is its own bucket. Risks redundant design work.
- **C. Two-tier:** design once on the largest panel (emStocksControlPanel), then apply per-panel with the design as reference.

**Chosen direction:** **A. Design-once, apply-mechanically — but only within a single pattern-id.**

**Why:** Phase 3 clustering already groups by `(pattern_id, scope_key)`, so emstocks rows naturally split across pattern-id buckets (P-001 emstocks, P-002 emstocks, P-004 emstocks-dialog-cells, etc.). Within each such bucket, the rows are pattern-coherent by construction; design-once / apply-mechanically falls out of the clustering. No special stocks-handling logic needed at the bucket level — the cross-cutting nature is already absorbed by Phase 3.

**Operational consequence:** D-004 is effectively a non-decision *given* the Phase 3 clustering rule. It remains in the catalog as a citation for the per-bucket sketcher to confirm "yes, mechanical application across all in-bucket rows is the intent." If clustering proposes a sub-split that violates this (e.g., breaking emstocks rows of one pattern-id across multiple buckets by file), revisit.

**Open questions deferred to bucket design:**
- Whether emStocksControlPanel-37 (the largest single-file slice) should pilot the pattern fix and merge before the rest of the stocks rows in the same bucket land. PR-staging concern, not design concern.

---

## D-005-poll-replacement-shape

**Question:** When replacing a polling consumer with a subscribe, do we go direct (`subscribe → react in callback`) or use `subscribe → mark dirty → re-poll on next tick`?

**Affects:** P-006 (10) + P-007 (6) + the 4 polling rows in P-003 = 20 rows.

**Options considered:**
- **A. Direct subscribe.** Callback fires, consumer reacts immediately. Matches C++ `Cycle()`-driven model where signal arrival schedules a `Cycle()` invocation that re-reads state.
- **B. Subscribe + mark-dirty + re-poll.** Callback only sets a dirty flag; next tick re-runs the polling code. Preserves the polling code unchanged, only adds the trigger.
- **C. Per-row choice.** Pick A or B based on consumer complexity.

**Chosen direction:** **A. Direct subscribe (with the consumer collapsed into the subscribe callback).**

**Why:** C++ `Cycle()` semantics are: "scheduler invokes `Cycle()` when any subscribed signal fires; `Cycle()` re-reads relevant state." The Rust port already has `Cycle()` shaped this way. So "direct subscribe" in Rust = "subscribe to the signal so the engine schedules `Cycle()`," which then re-reads and reacts. Option B is what the polling consumers currently do *minus the subscribe* — adopting B would just preserve the drift with a trigger. Option C invites bikeshedding.

**Open questions deferred to bucket design:**
- ~~For consumers that currently poll multiple sources (e.g., `emColorField::Cycle` polls four child ScalarFields), the subscribe call is N-fold. Bucket sketcher confirms whether the C++ original subscribes individually (likely) or to an aggregated signal. Default: mirror C++.~~ **Resolved by B-015 brainstorm (b521b3f6):** C++ `emColorField::AutoExpand` subscribes individually to each child (8 separate `AddWakeUpSignal` calls at cpp:245/255/265/277/288/298/308/320, not aggregated). Default holds: mirror C++.

**See also D-006-subscribe-shape** — D-005 picks the *reaction model* (direct subscribe, react in Cycle); D-006 picks the *wiring shape* (first-Cycle init block + IsSignaled checks at top of Cycle). Complementary, not competing. Any P-006/P-007 bucket implementing the D-005 direct-subscribe choice does so via the D-006 shape.

---

## D-006-subscribe-shape

**Question:** What is the canonical Rust shape for the C++ `AddWakeUpSignal(sig)` + `IsSignaled(sig) in Cycle()` pattern, given that the Rust port registers panel-engines *after* panel `new()` returns and `ConstructCtx` does not expose `connect`?

**Affects:** every bucket that adds a subscribe — at minimum P-001, P-002, P-006, P-007, and the consumer side of P-003. Cited by B-005 onward.

**Origin:** Surfaced and resolved during the B-005-typed-subscribe-emfileman bucket-design brainstorm (design doc `docs/superpowers/specs/2026-04-27-B-005-typed-subscribe-emfileman-design.md`, commit `d95d55a7`).

**Options considered:**
- **A. First-Cycle init.** Panel adds `subscribed_init: bool`; first `Cycle()` invocation calls `ectx.connect(sig, ectx.id())` for every signal of interest, sets the flag, then runs `IsSignaled` checks. Reactions live inline in Cycle, mirroring C++ Cycle body. Matches C++ structure (C++ also calls `AddWakeUpSignal` from Cycle context, not constructor).
- **B. Deferred-queue at construction.** Mirrors `emDialog::add_pre_show_wake_up_signal` — queue subscriptions at construction, drain on first wake. Viable fallback for panels that need subscriptions live before first wake; none of B-005's 21 rows require this.
- **C. Invert panel-tree lifetime.** Register engines before behavior `new()` so `ConstructCtx` can `connect` directly. Rejected: large blast radius, no payoff over A.

**Chosen direction:** **A. First-Cycle init.**

**Why:** A is the smallest viable shape that mirrors C++ structure exactly without requiring upstream ownership-model changes. The `subscribed_init: bool` overhead is one byte per panel and one branch per Cycle call; negligible. The C++ `AddWakeUpSignal` call site is in Cycle context, not constructor, so first-Cycle init *is* the closest port. Option B is a known-good fallback for any future bucket whose panels can't honor first-Cycle init (e.g., a panel that must react to signals before its first natural Cycle invocation); D-006 admits B as a per-bucket override but defaults to A.

**Operational rule:**
1. Default shape: first-Cycle init block calling `ectx.connect(...)` for every reactive signal, gated on `!subscribed_init`, then `IsSignaled(...)` checks at top of Cycle, then reactions inline.
2. If a bucket's row data shows a panel that needs subscriptions live before first natural Cycle, switch that panel to deferred-queue (option B) and document at the bucket level. The working-memory session updates this entry to record any such override.

**Ratified by implementation (post-B-005 merge `91433733`):** three implemented sightings — B-014 (`emVirtualCosmosPanel`), B-009 (`emFileManControlPanel`), B-005 (`emFileManControlPanel` 20 widget signals + `emFileLinkPanel-53` broadcast). The first-Cycle init shape is the canonical remediation pattern for P-002 (no-subscribe-accessor-present); subsequent P-002 buckets should adopt without re-litigation.

**Option B override sightings (deferred signal allocation + pending-fire drain):**
- **B-015 row -50 (`emFilePanel::SetFileModel`)** — subscribe deferred to Cycle because `SetFileModel` callers include constructors with no `EngineCtx`. `DIVERGED: language-forced` annotation at callsite.
- **B-004 emcore-slice (`emFilePanel::vir_file_state_signal` allocation + mutator fire)** — `new()` unchanged; `ensure_vir_file_state_signal` allocates in Cycle; `pending_vir_state_fire: bool` set by `SetFileModel` / `set_custom_error` / `clear_custom_error` and drained in Cycle. Same constraint: 14 construction callsites have no `EngineCtx`. Observable timing: 1-cycle delay from C++'s synchronous `Signal(VirFileStateSignal)` at `emFilePanel.cpp:51,78,87`. Language-forced.

**Open questions deferred to per-bucket design:**
- Whether buckets with consumer rows that subscribe to type-mismatched accessors (P-003 family) need a sub-shape that handles the `u64`-vs-`SignalId` type at the connect call. Currently no — those connects must wait for the accessor flip (D-001) per the cross-bucket prereq.

---

## D-007-mutator-fire-shape

**Question:** When a model accessor flips from a polled `u64` counter to a real `SignalId` (D-001 family), how do mutators fire the signal? Models in the Rust port don't carry a `Scheduler` reference (no class-based engine ownership); the bumper has only `&mut self` today.

**Affects:** every accessor flip in D-001's family, and any future flip that converts a polled counter to a signal. Cited by B-009 onward; back-propagates to B-008 (prior sighting on `Acquire`) and B-004 (where it was flagged as candidate-if-rediscovered).

**Origin:** Surfaced and resolved during the B-009-typemismatch-emfileman bucket-design brainstorm (design doc `docs/superpowers/specs/2026-04-27-B-009-typemismatch-emfileman-design.md`, commit `0a7d7fd3`). Third sighting of the pattern (B-008 hit it on `Acquire`; B-004 flagged it as candidate-if-rediscovered).

**Options considered:**
- **A. Thread `&mut EngineCtx<'_>` through every mutator.** Mutator signature gains `ectx`; mutator calls `ectx.fire(sig)` synchronously. Matches C++ `emSignal::Signal()` synchronous semantics exactly. Blast radius: all mutator callsites (in B-009 these are panel `Input`/`Cycle` bodies that already have ectx — no exceptions found in callsite enumeration).
- **B. Model owns a tiny "publisher" engine.** Mutators set a dirty flag; the publisher engine fires on next tick. Adds one engine per model. **Observable cost:** one-tick defer relative to mutator (C++ fires synchronously). Rejected: observable drift with no forced-category justification.
- **C. "Fire at next observation" hybrid.** Lazy fire from a place that has ectx. Awkward; defers fire to observer access, not mutator. Rejected.

**Chosen direction:** **A. Thread `&mut EngineCtx<'_>` through mutators.**

**Why:** Per Port Ideology, observable timing matches C++ synchronously — B and C both introduce timing drift without forced-category justification. The only blocker for A is "we haven't threaded ectx through mutators yet," which is project-internal ownership, not language-forced. B-009's mutator-callsite enumeration confirmed all production callsites already have ectx (panel `Input`/`Cycle` bodies). Future buckets that flip accessors should enumerate their callsites and confirm ectx availability — if a single callsite genuinely lacks ectx (filesystem watch callback, IPC handler, deferred timer), that exception forces a per-callsite hybrid; surface in reconciliation.

**Composes with D-008:** the mutator-fire helper is a no-op when the signal is `SignalId::null()`, matching C++ `emSignal::Signal()` with zero subscribers.

**Mutator signature shape (amended post-B-009 merge `50994e26`):** the bound on the ectx parameter is **`&mut impl SignalCtx`**, not the literal `&mut EngineCtx<'_>` shown in earlier design tables. Both `EngineCtx` and `SchedCtx` implement `SignalCtx`; the trait bound is the canonical form. **Why the broadening was forced:** `PanelBehavior::Input` only receives `PanelCtx` (no `EngineCtx`), so any mutator reachable from an `Input` callsite must accept the broader bound. B-009 confirmed this on multiple emfileman accessor flips. **B-014 precedent applies here too** — the trait-bound shape was used informally before B-009; B-009 makes it the explicit rule. Per-bucket designers: write `&mut impl SignalCtx` in mutator signatures; do not write `&mut EngineCtx<'_>` even if your only known callsite is an engine `Cycle`.

**Open questions deferred to per-bucket design:**
- Whether to introduce a typed wrapper (e.g., `Bumper<T>`) to enforce the ectx-threading discipline at the type level. Currently no — individual `&mut self` + `&mut EngineCtx<'_>` signatures are explicit enough.

**Composition note (post-B-014):** D-007 + D-008 compose to handle bootstrap-only callsites benignly. Example from B-014: `emVirtualCosmosModel::Reload` is called only from inside `Acquire`'s bootstrap closure where ectx is unavailable; at that point no panel has subscribed, so `change_signal == SignalId::null()` and `ectx.fire(...)` would be a no-op. The mutator can keep its no-ectx signature with a `// CALLSITE-NOTE:` indicating future post-Acquire callers must thread ectx. First benign hybrid recorded.

Watch-list: promoted to D-009-polling-intermediary-replacement; see § D-009 below (or above, depending on doc ordering).

---

## D-008-signal-allocation-shape

**Question:** Where/when does a model's `SignalId` get allocated, given that `Acquire(ctx)` doesn't carry scheduler/`EngineCtx`? (Same friction D-006 chose first-Cycle init to avoid on the consumer side.)

**Affects:** every accessor flip that introduces a new `SignalId` field on a model. Forced companion to D-007 — every accessor flip needs *some* allocation shape, so the question recurs by construction.

**Origin:** Surfaced and resolved during the B-009-typemismatch-emfileman bucket-design brainstorm (commit `0a7d7fd3`).

**Options considered:**
- **A1. Lazy allocation by first subscriber.** Model field is `Cell<SignalId>` initialized `null`. The accessor allocates lazily on first call and caches. Mutators check `if sig != null { ectx.fire(sig) }`. Matches C++ `emSignal::Signal()` with zero subscribers (silent no-op). **API shape:** a single accessor matching the C++ name (`fn GetXxxSignal(&self, ectx) -> SignalId`) is the canonical form — it mirrors C++'s `GetXxxSignal()` per File and Name Correspondence and folds allocation into the same call. (An earlier draft of this entry split the API into `EnsureXxxSignal(ectx)` + `GetXxxSignal(&self)`; that split was speculative and produced no real callsite that wanted allocation-check without allocation. Sanctioned post-B-003 merge `eb9427db`; combined form re-applied at B-014 merge `c2871547` and B-009 merge `50994e26`, both of whose design docs were written against the pre-amendment split form. **Bucket designers writing future "no-acc" buckets should adopt the combined form directly — do not re-derive the split form.** Three sightings of the split→combined supersession is the established pattern.)
- **A2. Eager allocation by threading scheduler through Acquire.** `Acquire(ctx) -> Acquire(ctx, scheduler)`. Crisp single allocation point. Hits the same friction D-006 cited: ConstructCtx doesn't expose scheduler/`create_signal`; threading through every Acquire callsite is a ripple bigger than the bucket.
- **A3. Frame `emContext` to expose a scheduler handle.** Models call `ctx.scheduler().create_signal()` at Acquire. Substantial framework change; out of B-009 scope. Future architectural candidate when enough models accumulate `SignalId::default()`/`null()` placeholders.

**Chosen direction:** **A1. Lazy allocation by first subscriber.**

**Why:** A1 is self-contained (no framework or Acquire-signature changes), mirrors D-006's first-Cycle init shape symmetrically (subscribers initialize, allocation occurs as a side-effect of init), and the "no-op fire when no subscriber" semantics match C++ exactly. A2 is rejected for the same reason D-006 rejected its analogous option C. A3 is the most C++-faithful long-term answer (in C++, models do have scheduler access via engine ownership) but is out of scope for any individual bucket — it lifts to a separate framework change.

**Composes with D-007:** D-007's `ectx.fire(sig)` is a no-op when `sig == SignalId::null()`, matching C++ pre-subscriber semantics.

**Watch-list (not a decision):** A3 — expose scheduler through `emContext` — may become worthwhile once enough models carry `SignalId::default()`/`null()` placeholders. Current placeholder occupants noted by B-009 brainstorm: `emFileLinkModel`, `emFileManTheme`, `emFileManConfig`, `emFileModel`. If this list grows, the working-memory session promotes A3 to a separate framework-lift bucket.

**Open questions deferred to per-bucket design:**
- ~~Whether `Ensure*Signal` should sit on `&self` (with interior `Cell` mutation) or `&mut self`. Currently `&self` per A1 description; revisit if borrow-checker friction surfaces.~~ **Resolved post-B-003:** combined `GetXxxSignal(&self, ectx)` form using interior `Cell` mutation works cleanly; `&mut self` not needed. The Ensure/Get split is retired.

---

## D-009-polling-intermediary-replacement

**Question:** When the Rust port has a polling intermediary — a `Cell` / `RefCell` / similar field set in one site, polled by another engine's `Cycle` to fire a signal or trigger a reaction — where C++ fires/calls directly, what is the canonical fix?

**Affects:** any drift instance with the polling-intermediary topology. Sightings as of promotion (2026-04-27):
1. `AutoplayFlags.progress` (B-003 brainstorm `703fa462`) — addressed by D-002 §1 R-A: drop the entire shim.
2. `mw.to_reload` chain through `MainWindowEngine` (B-012 brainstorm `bf6e9bd5`) — addressed by routing reload through `mw.ReloadFiles(&self, ectx)` per D-007.
3. `FsbEvents` closure-buffer drained by `emFileSelectionBox::Cycle` (B-010 brainstorm `09f08710`) — addressed by direct widget-state read in `IsSignaled` branches per D-006.
4. `generation: Rc<Cell<u64>>` counter on `emCoreConfigPanel` (B-010 sighting; D4 resolution at `81b19c75`) — **resolved**. `FactorFieldPanel` subscribes to per-field value signals in `Cycle` via `set_value_silent`; `MouseMiscGroup`/`CpuGroup` subscribe to aggregate `emRecNodeConfigModel::GetChangeSignal()` and call `update_output()` via `set_checked_silent`. `DIVERGED: language-forced` 1-cycle delay at `FactorFieldPanel::Cycle`.

**Origin:** Promoted by B-010 brainstorm after the 3-sighting threshold was reached (4 actual sightings at promotion time). Replaces the earlier watch-list paragraph in D-007.

**Options considered:**
- **A. Remove the intermediary; thread ectx into the original mutation site (or a typed method on the owning type) and fire synchronously per D-007.** Matches C++ structure; no observable timing drift.
- **B. Keep the intermediary; document as DIVERGED with a `preserved-design-intent` claim.** Rejected: the timing drift (one-tick defer) is observable; no forced category survives the four-question test for any of the 4 sightings.
- **C. Per-instance triage between A and B.** Rejected: every sighting so far has converged on A; no defensible B-instance has surfaced. Per-instance framing would invite bikeshedding without payoff.

**Chosen direction:** **A. Remove the intermediary.**

**Why:** Every sighting so far is observable drift (one-tick defer relative to C++'s synchronous fire/call). Per Port Ideology, observable drift without forced-category justification is fidelity bug. The fix recipe is deterministic: thread ectx into the original mutation site (or expose a typed method on the owning type that takes ectx), call `ectx.fire(...)` or invoke the typed method synchronously per D-007, delete the intermediate field and any polling block that drains it. Composes with D-006 (subscribe shape) and D-007 (mutator-fire shape).

**Operational rule:**
1. Identify the polling intermediary: a Cell/RefCell field set in site A, polled in site B's `Cycle` to fire/react.
2. Find the C++ analogue. Confirm C++ fires/calls synchronously without an analogous intermediary.
3. Remove the intermediary; fire/invoke synchronously from site A per D-007.
4. Delete the intermediate field and the polling block.

**Open questions deferred to per-bucket design:**
- Whether to introduce a typed wrapper (e.g., `PollingIntermediary<T>`) to flag this drift shape at the type level. Currently no — pattern is recognizable enough by code review.
