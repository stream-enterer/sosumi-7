# B-018-fileDialog-singleton — P-008 — emFileDialog connect-with-poll-fallback singleton

**Pattern:** P-008-connect-with-poll-fallback
**Scope:** emcore
**Row count:** 1
**Mechanical-vs-judgement:** judgement-heavy
**Cited decisions:** none (packet ships no decisions; bucket-design brainstorm must originate or cite-forward the relevant ADRs)
**Prereq buckets:** none

## Pattern description

P-008 covers sites where a `scheduler.connect(...)` call coexists with a nearby `IsSignaled(...)` poll on the same signal — a hybrid wiring shape that is neither pure event-driven nor pure polling. Either the connect is redundant (poll will catch it) or the poll is redundant (connect will wake the engine), and the audit cannot tell which without reading C++ intent. In this bucket the singleton instance is `emFileDialog`'s `od_finish_sig` / `od_sig` wiring, where a poll at line 169 sits alongside a post-show `connect` at line 733 and the audited `connect_call` at line 516.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emFileDialog-196 | src/emCore/emFileDialog.cpp:196 | crates/emcore/src/emFileDialog.rs:516 | present | IsSignaled at rs:169; fallback `scheduler.connect(od_finish_sig, outer_engine_id)` at rs:733 |

## C++ reference sites

- src/emCore/emFileDialog.cpp:196

## Open questions for the bucket-design brainstorm

- Is the IsSignaled poll at `emFileDialog.rs:169` the actual drift, or is the connect_call at `:516` misplaced relative to the C++ shape at `emFileDialog.cpp:196`?
- Does the post-show `scheduler.connect(od_finish_sig, outer_engine_id)` at `:733` make either earlier wiring site redundant, or are all three required by C++ semantics?
- What does C++ `emFileDialog.cpp:196` actually do — connect, poll, or both — and which Rust site is the faithful mirror?
- Should remediation remove the poll, remove one of the connects, or restructure so the three sites collapse into a single wiring point?
- Does this singleton justify a P-008-wide ADR, or is `emFileDialog` idiosyncratic enough that the decision stays local to this bucket?
- Are there latent test signals (golden, behavioral) that would distinguish "poll fires first" vs "connect fires first" ordering, and do we need to author one before remediating?
