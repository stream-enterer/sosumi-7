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

**Affects:** P-004 (29 rows: emcore=15, emmain=10, emstocks=4) + P-005 (6 rows). Cited by every bucket containing rc-shim rows.

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

**Open questions deferred to per-bucket design:**
- Whether to introduce a typed wrapper (e.g., `Bumper<T>`) to enforce the ectx-threading discipline at the type level. Currently no — individual `&mut self` + `&mut EngineCtx<'_>` signatures are explicit enough.

---

## D-008-signal-allocation-shape

**Question:** Where/when does a model's `SignalId` get allocated, given that `Acquire(ctx)` doesn't carry scheduler/`EngineCtx`? (Same friction D-006 chose first-Cycle init to avoid on the consumer side.)

**Affects:** every accessor flip that introduces a new `SignalId` field on a model. Forced companion to D-007 — every accessor flip needs *some* allocation shape, so the question recurs by construction.

**Origin:** Surfaced and resolved during the B-009-typemismatch-emfileman bucket-design brainstorm (commit `0a7d7fd3`).

**Options considered:**
- **A1. Lazy allocation by first subscriber.** Model field is `Cell<SignalId>` initialized `null`. Each consumer's first-Cycle init calls a new `&self` method `EnsureXxxSignal(ectx) -> SignalId` which allocates on first call and caches. Mutators check `if sig != null { ectx.fire(sig) }`. Matches C++ `emSignal::Signal()` with zero subscribers (silent no-op).
- **A2. Eager allocation by threading scheduler through Acquire.** `Acquire(ctx) -> Acquire(ctx, scheduler)`. Crisp single allocation point. Hits the same friction D-006 cited: ConstructCtx doesn't expose scheduler/`create_signal`; threading through every Acquire callsite is a ripple bigger than the bucket.
- **A3. Frame `emContext` to expose a scheduler handle.** Models call `ctx.scheduler().create_signal()` at Acquire. Substantial framework change; out of B-009 scope. Future architectural candidate when enough models accumulate `SignalId::default()`/`null()` placeholders.

**Chosen direction:** **A1. Lazy allocation by first subscriber.**

**Why:** A1 is self-contained (no framework or Acquire-signature changes), mirrors D-006's first-Cycle init shape symmetrically (subscribers initialize, allocation occurs as a side-effect of init), and the "no-op fire when no subscriber" semantics match C++ exactly. A2 is rejected for the same reason D-006 rejected its analogous option C. A3 is the most C++-faithful long-term answer (in C++, models do have scheduler access via engine ownership) but is out of scope for any individual bucket — it lifts to a separate framework change.

**Composes with D-007:** D-007's `ectx.fire(sig)` is a no-op when `sig == SignalId::null()`, matching C++ pre-subscriber semantics.

**Watch-list (not a decision):** A3 — expose scheduler through `emContext` — may become worthwhile once enough models carry `SignalId::default()`/`null()` placeholders. Current placeholder occupants noted by B-009 brainstorm: `emFileLinkModel`, `emFileManTheme`, `emFileManConfig`, `emFileModel`. If this list grows, the working-memory session promotes A3 to a separate framework-lift bucket.

**Open questions deferred to per-bucket design:**
- Whether `Ensure*Signal` should sit on `&self` (with interior `Cell` mutation) or `&mut self`. Currently `&self` per A1 description; revisit if borrow-checker friction surfaces.
