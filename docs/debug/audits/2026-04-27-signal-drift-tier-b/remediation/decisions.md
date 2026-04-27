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
- The emAutoplay flags-passing pattern (`AutoplayFlags { progress: Rc<Cell<f64>> }`) — does this fall under rule 1 or rule 2? emAutoplay has no C++ analogue (it's a Rust-only panel) so the rule needs adaptation. Bucket sketcher flags this for the working-memory session to resolve before bucket execution.

---

## D-003-gap-blocked-fill-vs-stub

**Question:** For the 16 gap-blocked rows where the upstream-gap is the blocker, do we fill the gap (port the missing infrastructure) or stub at the consumer (no-op signal subscription that becomes live when the gap fills)?

**Affects:** all gap-blocked rows: 10 P-001 + 2 P-002 + 3 P-003 = 15 rows. (Was 16 before B-006 reconciliation reclassified `emMainControlPanel-218` from gap-blocked to drifted.)

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
- For consumers that currently poll multiple sources (e.g., `emColorField::Cycle` polls four child ScalarFields), the subscribe call is N-fold. Bucket sketcher confirms whether the C++ original subscribes individually (likely) or to an aggregated signal. Default: mirror C++.

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
