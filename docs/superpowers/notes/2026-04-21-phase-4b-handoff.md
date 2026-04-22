# Phase 4b Handoff Prompt

Paste the block below into a fresh session.

---

You are resuming the eaglemode-rs port-ownership rewrite at **Phase 4b — emRec listener tree + emFlagsRec (revised 2026-04-21; see Phase 4 overview note)**.

## State at handoff

- Repo: `/home/a0/git/eaglemode-rs`. Branch: `main` (clean tree expected; verify with `git status --porcelain`).
- Phase 4a merged at `47709e0f` (tag `port-rewrite-phase-4a-complete`). Follow-up commits `1269802d` + `6b9e8a4f` + `51834c80` on main cleaned a `.claude/` harness-file leak and fixed a closeout-note error.
- **Not pushed yet** — `origin/main` is behind local `main` by the Phase 4a merge + closeout + three fix-ups. Decide with user whether to push before starting 4b; Bootstrap B5/B6 checks work locally either way.
- Gate at main HEAD: fmt clean, clippy clean, **2535 nextest pass**, goldens **237 pass / 6 fail** (the six longstanding pre-existing failures — composition_tktest_1x/2x, notice_window_resize, testpanel_expanded/root, widget_file_selection_box).

## Phase 4a inheritance (what's live, what's deferred)

**Live on main:**
- `emRecNode` trait (parent accessor only).
- `emRec<T: Clone + PartialEq>: emRecNode` trait.
- Five primitive concretes: `emBoolRec`, `emIntRec` (i64), `emDoubleRec` (f64, NaN-preserving guard form), `emEnumRec` (u32 index + `Vec<String>` table; stored `max_index`), `emStringRec`.
- `emRecParser.rs` (SPLIT from the old `emRec.rs` — textual parser/serializer for record format, maps to C++ `emRecReader`/`emRecWriter`).

**Deferred to 4b+ (grep `TODO(phase-4b+)` in `crates/emcore/src/em{Bool,Int,Double,Enum,String}Rec.rs`):**
- `Invert()` on emBoolRec (emRec.cpp:315-319).
- `SetToDefault`, `IsSetToDefault`, `TryStartReading`, serialization hooks on all 5 primitives.
- Parent wiring on all 5 concretes (`parent() -> None` currently). Landing parent pointers will retroactively change observable behavior at every currently-isolated `SchedCtx` fire site — capture as a Phase 4b invariant.
- `emEnumRec._identifiers` field has a leading underscore to suppress dead-code lint; **drop the underscore the moment `GetIdentifier(index)` lands** or the accessor will read a misnamed field silently.
- JSON entries E026 (persistence) + E027 (compound types) remain open; close at Phase 4e gate (renumbered 2026-04-21; see docs/superpowers/plans/2026-04-21-port-rewrite-phase-4-overview.md).

## How to proceed

1. Run the **shared Bootstrap ritual** from `docs/superpowers/plans/2026-04-19-port-rewrite-bootstrap-ritual.md` with `<N>` = `4b`. Scan the Phase 4b plan for stage-only tasks at B11a before disabling the pre-commit hook.
2. Phase 4b plan: `docs/superpowers/plans/2026-04-19-port-rewrite-phase-4b-emrec-compound.md`.
3. Use the `superpowers:subagent-driven-development` skill for task execution. TDD per task; pre-commit hook runs fmt+clippy+nextest automatically.

## Known operational details to carry forward

- `SignalId` lives in `crate::emSignal`, **not** `crate::emScheduler` — the Phase 4a plan had this wrong; any Phase 4b plan code using `crate::emScheduler::SignalId` is similarly bogus and must be fixed at the import site.
- Test scaffolding pattern for `SchedCtx` is duplicated across all 5 primitive test modules; see `emBoolRec.rs` or `emIntRec.rs` for the canonical `make_sched_ctx` form. **Consider hoisting to a `test_support` helper** before Phase 4b adds more tests in the same shape.
- Scheduler `EngineScheduler` panics on drop with pending signals — tests must `sc.remove_signal(sig)` after asserting a fire.
- `emDoubleRec` uses explicit `<`/`>` guards (not `f64::clamp`) to preserve C++ IEEE 754 NaN behavior — do NOT "simplify" to `.clamp()`.
- `.claude/` is now in `.gitignore` as of `6b9e8a4f`; subagents may not notice. Sanity-check `git status` before every commit to catch stray harness leaks.

## References

- `docs/superpowers/notes/2026-04-19-phase-4a-closeout.md` — full Phase 4a summary with invariants.
- `docs/superpowers/notes/2026-04-19-phase-4a-ledger.md` — per-task commit SHAs + C++ line citations.
- `docs/superpowers/notes/2026-04-19-phase-4a-exit.md` — baseline vs exit metric deltas.
- `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` — authoritative spec; §7 D7.x for the emRec layer.
- `CLAUDE.md` — Port Ideology, File and Name Correspondence, `DIVERGED:` / `SPLIT:` rules.

Begin Phase 4b.
