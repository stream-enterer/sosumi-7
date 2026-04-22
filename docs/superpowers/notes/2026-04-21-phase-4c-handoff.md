# Phase 4c Handoff Prompt

Paste the block below into a fresh session.

---

You are resuming the eaglemode-rs port-ownership rewrite at **Phase 4c — emRec listener tree retrofit + structural compounds (`emStructRec`, `emUnionRec`, `emArrayRec`, `emTArrayRec<T>`, `emRecListener`)**.

## State at handoff

- Repo: `/home/a0/git/eaglemode-rs`. Branch: `main` (clean tree expected; verify with `git status --porcelain`).
- Phase 4b.1 merged at `d9e1bc85`, tagged `port-rewrite-phase-4b-1-complete`. Chain of merges on main: 4a → 4b → 4b.1 → (4c next).
- **Not pushed yet** — `origin/main` is behind local `main` by the 4b.1 commits. Decide with user whether to push before starting 4c.
- Gate at main HEAD: fmt clean, clippy clean, **2562 nextest pass**, goldens **237 pass / 6 fail** (the six longstanding pre-existing failures unchanged since Phase 4a baseline).

## Phase 4b.1 inheritance (what's live, what's deferred)

**Live on main:**
- All Phase 4a primitives unchanged (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`).
- Phase 4b's `emFlagsRec`.
- Phase 4b.1: `emAlignment` (u8 type alias + `EM_ALIGN_*` constants + string conversions), `emAlignmentRec` (u8-valued, C++-faithful), `emColorRec` (with `have_alpha` alpha-forcing).
- Three production consumers (`emVirtualCosmos`, `emBookmarks`, `emFileManTheme`) migrated off legacy parser-era types.
- Legacy `emColorRec`/`emAlignmentRec` + `RecListenerList` deleted from `emRecRecTypes.rs`.
- Stopgap free functions in `emRecRecTypes.rs`: `em_color_{to,from}_rec_struct`, `em_alignment_{to,from}_rec_value` — retire when Phase 4d persistence lands on the new rec types.

**Deferred to Phase 4c (THIS PHASE):**
- Listener tree rep: `aggregate_signals: Vec<SignalId>` field on every primitive (including 4b.1's `emAlignmentRec` and `emColorRec`).
- `register_aggregate(&mut self, sig: SignalId)` method on every emRec concrete type.
- `emRecListener`.
- `emStructRec`, `emUnionRec`, `emArrayRec`, `emTArrayRec<T>`.
- **ADR governs the rep choice (do not relitigate):** `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md` chose **R5 reified signal chain**.

**Deferred to Phase 4d+:**
- Persistence (`emRecReader`/`Writer` stack, `TryRead`/`TryWrite` on every concrete type).
- Retirement of the stopgap free functions in `emRecRecTypes.rs`.
- `emConfigModel::LoadAndSave` wiring.

**Deferred to Phase 4e (with explicit tracking):**
- Kani harness regeneration for `emColorRec`. 7 legacy harnesses deleted; requires `ConstructCtx` mock infrastructure that doesn't yet exist. `TODO(phase-4e)` marker lives at `crates/eaglemode/tests/kani/proofs_generated.rs:748`.
- **Alignment single-axis drift audit.** The new `emAlignmentRec` ships C++-faithful (u8 bitmask) but no consumer migrated — `emfileman::emFileManTheme` still uses the pre-existing Rust `emTiling::Alignment` single-axis enum (Start|Center|End|Stretch). The legacy bitmask→enum lossy mapping survives as free-function stopgaps. Phase 4e plan now carries invariant **I4e-6** and a dedicated "Pre-existing drift to audit" section gating this.
- Stale `.kani/provable_functions.json` entries (10 `emAlignmentRec` + 1 `emColorRec::FromRecStruct`). Will wash at next regeneration; do NOT hand-edit.
- `emCoreConfig` migration closes E026 + E027.

## How to proceed

1. Run the **shared Bootstrap ritual** at `docs/superpowers/plans/2026-04-19-port-rewrite-bootstrap-ritual.md` with `<N>` = `4c`. Scan the Phase 4c plan for stage-only tasks at B11a — **Phase 4c has stage-only tasks**: the primitives retrofit is staged across multiple commits before the compounds consume the `register_aggregate` method. READ THE PLAN CAREFULLY at B11a — disable the pre-commit hook if the plan confirms stage-only semantics.
2. Phase 4c plan: `docs/superpowers/plans/2026-04-21-port-rewrite-phase-4c-emrec-compound-types.md`.
3. Companion ADR (authoritative for the rep): `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md`.
4. Use the `superpowers:subagent-driven-development` skill for task execution. TDD per task; pre-commit hook runs fmt+clippy+nextest automatically unless disabled per B11a.

## Known operational details to carry forward

- **Seven primitives** now carry the retrofit, not six as the Phase 4c plan states (plan was written before 4b.1 landed):
  - Phase 4a: `emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec` (5)
  - Phase 4b: `emFlagsRec` (1)
  - Phase 4b.1: `emAlignmentRec`, `emColorRec` (2)
  - **Total: 8 primitives**. Invariant I4c-1 must cover all eight; scan every file under `crates/emcore/src/em*Rec.rs`.
- **`emColorRec`'s alpha-forcing branch** in `SetValue` runs BEFORE the equality check (matches C++ `emRec.cpp:1162-1169`). The `aggregate_signals` fire loop retrofits here must run AFTER the own-signal fire, only when a change actually occurred — do not fire aggregates on the alpha-normalized no-op path.
- **`SignalId` lives in `crate::emSignal`**, not `crate::emScheduler`. Any plan text that says otherwise is stale.
- **Scheduler `EngineScheduler` panics on drop with pending signals** — tests that fire signals must `sc.remove_signal(sig)` after asserting the fire.
- **`emDoubleRec` uses explicit `<`/`>` guards (not `f64::clamp`)** to preserve C++ IEEE 754 NaN behavior — do NOT "simplify" if you happen to read it.
- **Test scaffolding**: every primitive duplicates `make_sched_ctx` + Rc<RefCell<...>> setup. Phase 4a/4b/4b.1 closeouts flagged hoisting these to shared `crates/emcore/src/test_support` — don't do that hoist in Phase 4c's task list unless the plan explicitly says to. It's scope creep.
- **`.claude/` is in `.gitignore`**; subagents may not notice. Sanity-check `git status` before every commit to catch stray harness leaks.
- **Alignment drift is DOCUMENTED, not a bug to fix here.** If Phase 4c writing triggers the thought "should I also migrate emFileManTheme to emAlignment u8?", the answer is NO — that audit is explicitly Phase 4e (I4e-6). Stay in scope.
- **Legacy `RecListenerList` is GONE** (Phase 4b.1 removed it; no consumers remained). Do not resurrect. The new listener tree is the `Vec<SignalId>` reified signal chain per ADR — completely independent of the deleted callback-list machinery.

## References

- `docs/superpowers/notes/2026-04-19-phase-4b-1-closeout.md` — Phase 4b.1 summary with invariants verified and tracking items for downstream phases.
- `docs/superpowers/notes/2026-04-19-phase-4b-1-ledger.md` — task-by-task SHA log for 4b.1.
- `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md` — authoritative rep for the listener tree.
- `docs/superpowers/plans/2026-04-21-port-rewrite-phase-4-overview.md` — canonical execution chain (4a → 4b → 4b.1 → 4c → 4d → 4e).
- `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` — authoritative spec; §7 D7.1 for the emRec layer.
- `CLAUDE.md` — Port Ideology, File and Name Correspondence, `DIVERGED:` / `SPLIT:` rules.

Begin Phase 4c.
