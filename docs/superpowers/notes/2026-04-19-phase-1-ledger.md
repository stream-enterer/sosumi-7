# Phase 1 — Scheduler Event-Loop Threading — Ledger

**Started:** 2026-04-19 (resume session)
**Branch:** port-rewrite/phase-1
**Baseline:** see 2026-04-19-phase-1-baseline.md
**Spec sections:** §3.1, §3.1.1, §3.3, §3.7 (framework_actions only), §4 D4.1–D4.11
**JSON entries to close:** E001, E002, E003, E004, E005, E007, E008, E009, E010, E011, E036

## Task log

- Task 1 done @ 0bb61f0. register_engine argument-order decision: adapters expose target order `(behavior, priority)`; bodies call legacy `scheduler.register_engine(pri, b)`. Task 3 will flip the scheduler side.
- Task 1 note: name collision — `emEngine::EngineCtx` (existing, cycle-context) coexists with new `emEngineCtx::EngineCtx`. Both `pub`; different module paths. No rename performed. May warrant reconciliation in later phase.
- Task 1 note: `SignalId`/`EngineId`/`Priority` are re-exports only internally in `emScheduler`; imports sourced from `emSignal` and `emEngine` modules directly.
