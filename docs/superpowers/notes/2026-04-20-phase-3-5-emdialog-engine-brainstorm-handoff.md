# Phase 3.5 (proposed) — `emDialog → emEngine` — Brainstorm Handoff

**Created:** 2026-04-20, after Phase 3 completed with E024 explicitly deferred.
**Intended consumer:** a fresh session tasked with *brainstorming* (not implementing) the scope, shape, and entry criteria of an intermediate phase between Phase 3 (complete) and Phase 4 (unstarted).

---

## Handoff prompt

Paste the section between the dashed rules into the next session after `/clear`.

---

You are starting a **brainstorm session** for a proposed intermediate phase of the eaglemode-rs port-ownership-rewrite at `/home/a0/git/eaglemode-rs`. Working title: **Phase 3.5 — `emDialog → emEngine`**. Its sole purpose is to close raw-material divergence entry **E024** genuinely — a goal Phase 3 Task 6 attempted, fell short of, and honestly deferred.

**This is a brainstorm, not an implementation.** Use the `superpowers:brainstorming` skill. Produce a scope memo + follow-on plan outline. Do NOT touch code.

### Required reading (in order)

1. `CLAUDE.md` — Port Ideology, `#[allow]` whitelist, Do-NOT list. Binding.
2. `docs/superpowers/notes/2026-04-19-phase-3-closeout.md` — Phase 3 exit state.
3. `docs/superpowers/notes/2026-04-19-phase-3-ledger.md` — especially the Task 6 entry (as corrected) and the Task 7 I3b exclusion inventory. Ledger explicitly states E024 closure is deferred and names the prerequisite ("emDialog → emEngine port with proper wake-up-signal subscription plumbing").
4. `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json` — E024 entry (status `open`, `phase_3_progress` populated with commits `44e4aa9b` + `8a9154f4`). Read the entry's full text; it names the observable-surface property precisely.
5. `crates/emcore/src/emFileDialog.rs` — read the `DIVERGED:` block immediately above `pub fn Cycle` (added at `8a9154f4`). That comment is the exact description of the divergence Phase 3.5 exists to close.
6. `crates/emcore/src/emDialog.rs` — note: `emDialog` is currently NOT an `emEngine`. It has a `finish_signal: SignalId` fired synchronously from `Finish()` called by the caller.
7. `crates/emcore/src/emEngine.rs` — current `emEngine` trait contract (`Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool`).
8. `crates/emcore/src/emScheduler.rs` and `crates/emcore/src/emEngineCtx.rs` — look for `connect`, `is_signaled_for_engine`, `wake_up`. These are the pieces that stand in for C++ `emEngine::AddWakeUpSignal`. Confirm whether a true "subscribe engine to wake on signal" surface exists; the Task 6 implementer used `ctx.is_signaled(sig)` inside a caller-pulled method and did *not* subscribe for scheduler-driven wake-up.
9. C++ reference: `~/git/eaglemode-0.96.4/include/emCore/emEngine.h` (especially `AddWakeUpSignal`), `~/git/eaglemode-0.96.4/src/emCore/emDialog.cpp` (emDialog's Cycle), `~/git/eaglemode-0.96.4/src/emCore/emFileDialog.cpp:80-110` (emFileDialog's Cycle). These are ground truth.
10. `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §4 D4.9, §6 D6.1–D6.5 — design context for signal-driven dispatch.

### State at entry (verify)

- `main` @ `d0f1cc7b` (merge of `port-rewrite/phase-3-continue`). Tag `port-rewrite-phase-3-complete` exists and matches. Working tree clean except `.claude/`.
- Gate: 2476/0/9 nextest, 237/6 goldens baseline preserved, clippy clean, pre-commit hook active.
- Confirm with: `git status`, `git log --oneline -5`, `git tag -l 'port-rewrite-phase-3*'`.

### The problem you are brainstorming

**E024's observable property:** C++ `emFileDialog::Cycle` is dispatched by the scheduler automatically when subscribed signals fire (via `AddWakeUpSignal(Fsb->GetFileTriggerSignal())` and `AddWakeUpSignal(OverwriteDialog->GetFinishSignal())`). In the Rust port, `emFileDialog::Cycle` is a method the caller invokes manually. Same state-transition logic, but the "who decides when Cycle runs" semantic differs — which is exactly the timing observability E024 names.

**The hard constraint:** Closing E024 requires `emFileDialog` to be a registered `emEngine`. `emFileDialog` inherits from `emDialog` in C++; Rust-side, `emDialog` is not an engine yet. So `emDialog → emEngine` is a prerequisite — and possibly `emPanel → emEngine` before that, since `emDialog` inherits from `emPanel` in C++. Trace the inheritance chain and determine how far down the rabbit hole you actually need to go.

### Brainstorm goals (not exhaustive — add your own)

1. **Scope floor:** what is the *minimum* code change that closes E024 honestly? Does it require porting all of `emDialog`, or only wiring the `Cycle`/`AddWakeUpSignal`-equivalent surface?
2. **Scope ceiling:** does porting `emDialog → emEngine` transitively pull in `emPanel → emEngine`? If yes, is that already scoped for Phase 4? If Phase 4 already owns that work, Phase 3.5 may not need to exist — Phase 4 would absorb E024.
3. **The `AddWakeUpSignal` shape:** the current Rust scheduler has `connect(signal, engine)` (engine-side subscription) and `is_signaled_for_engine(signal, engine)` (check inside Cycle). Is that already semantically equivalent to `AddWakeUpSignal`, or is there a missing piece (e.g., the subscribed engine not auto-waking when the signal fires)? Read `emScheduler.rs` and find out. This determines whether Phase 3.5 is a small mechanical change or a scheduler-behavior change.
4. **Blast radius:** `rg -n 'emDialog::new|emDialog {' crates/` — how many sites construct dialogs today? Which of them currently own dialog state and would need to be adapted to the engine-registration ownership model (the dialog is Box'd into the scheduler; callers hold an `EngineId` and reach through ctx)?
5. **Ownership flip:** the Task 6 implementer's rationale for deferring was that making `emFileDialog` an engine would "require giving up ownership at construction (Box into scheduler), breaking the setter API that all current tests rely on." Is that an inherent problem or a fixable one (e.g., split inner-engine-state from outer-dialog-wrapper)? Look at how `InputDispatchEngine` (`crates/emcore/src/emInputDispatchEngine.rs`) handles the pattern — it's the extant template for a framework-owned engine.
6. **Test strategy:** Task 6 added `test_force_overwrite_result` as a helper to bypass real signal flow. In a scheduler-driven world, those tests rewrite themselves as "fire signal X, tick scheduler, assert state Y". Is this test migration trivial or does it expose gaps in the test scheduler harness?
7. **Is this a phase or a plan-less one-off?** If the scope floor from (1) is small (<~300 LOC, no new architectural surfaces), it may not warrant the full Bootstrap/Closeout ritual. It could be a single-commit "E024 closure" PR folded into Phase 4's Bootstrap. If the ceiling from (2) is large, it deserves its own phase.
8. **Relationship to deferred setter-API work elsewhere:** Phase 3's B3.4d keystone threaded `SchedCtx` through non-Input setters. Any pattern there that informs this phase?

### Anti-patterns to avoid (recurring traps)

- **Do not propose `Rc<RefCell<>>`, `Any`/downcast, `Weak`, `Arc`/`Mutex`, or `unsafe` to escape the ownership flip.** These are on the Do-NOT list in CLAUDE.md and several have dedicated feedback memories (`feedback_destructure_before_unsafe.md`, etc.).
- Do not recommend adding a second shadow scheduler or a parallel dispatch mechanism.
- Do not recommend renaming the divergence; E024 is an observable-surface property, not a naming one.
- If you find yourself proposing to "keep the caller-pulled method but make the scheduler *also* call it," stop — that's the same rebranded divergence. Either it's a registered engine with scheduler-driven Cycle, or E024 stays open.
- Do not rewrite `emDialog`'s inheritance structure beyond what C++ requires. Preserve name correspondence per `CLAUDE.md` File and Name Correspondence.

### Output deliverable

Write `docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-engine-plan.md` (or `-folded-into-phase-4.md` if the brainstorm concludes it belongs to Phase 4). Contents:

1. **Decision:** phase or fold-in? With rationale.
2. **Scope:** minimum code paths touched, expected blast radius (in files / LOC / test count delta).
3. **Prerequisites:** anything that must land before this phase can begin (e.g., if `emPanel → emEngine` is a prereq, name it).
4. **Tasks:** numbered task list per the project's plan template (phased, gated, gate commands, commit structure).
5. **Invariants:** what grep-enforceable assertions must hold at exit. At minimum: `emFileDialog: impl emEngine` and `dialog-registered-at-construction` evidence.
6. **E024 closure criteria:** exactly what evidence in the code + tests demonstrates E024 is truly closed (not rebranded). Suggest a test shape: "fire file_trigger_signal into the scheduler; tick once; assert dialog's finish_signal is now pending" — no caller invocation of any Cycle method.
7. **Open questions for user:** anything the brainstorm couldn't settle alone.

Do NOT write a Phase 4 plan in this session — Phase 4's scope is separate and will have its own brainstorm.

### Discipline

- This is brainstorm + planning only. No code edits.
- Use `superpowers:brainstorming` skill. It enforces the right shape for this work.
- If during brainstorm you realize the real answer is "absorb into Phase 4 and add one task there," that is a legitimate output — say so in the decision section and explain.
- If during brainstorm you discover a second E-entry that depends on this work (e.g., another dialog/cycle divergence), flag it in open questions.

### Begin with

1. Read the required docs in order.
2. `git status`, `git log --oneline -5`, `git tag -l 'port-rewrite-phase-3*'` — confirm entry state.
3. `rg -n 'impl emEngine for|fn Cycle\(' crates/emcore/src/` — map the existing engine population.
4. `rg -n 'AddWakeUpSignal\|wake_up_on\|connect(' crates/emcore/src/emScheduler.rs crates/emcore/src/emEngineCtx.rs` — confirm the wake-up surface.
5. Start the brainstorm skill.

---

End of handoff prompt.
