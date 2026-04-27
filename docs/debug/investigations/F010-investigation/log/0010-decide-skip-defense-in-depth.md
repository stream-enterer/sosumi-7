---
id: 0010
type: decide
timestamp: 2026-04-26T18:11:36Z
hypothesis_ids: [H1]
supersedes: null
artifacts: [docs/debug/investigations/F010-investigation/RESUME-INVESTIGATION.md]
---

# Decision: skip defense-in-depth, proceed to fix-first validation

## Decision

Skip plan Tasks 3.2-3.8 (defense-in-depth across H2, dispatch-cluster, invalidation-cluster, Tier-2 standalones, Tier-3 standalones, remaining blind spots, order-config-cluster). Proceed directly to fix-spec design for H1, then manual GUI verification (Task 4.2) as the validation gate.

## Rationale

H1 is mechanically confirmed at entry 0007 via a unit test demonstrating that `emPainter::Clear()` records zero ops in recording mode (the `require_direct()` dispatch returns None for `DrawList` targets and the call early-returns silently). P8 (the cluster mate) is falsified at entry 0008. The cluster resolved at entry 0009 with H1 as the surviving hypothesis.

The plan's defense-in-depth rule (Task 3.1 step 8) exists to detect multi-cause situations: if a second cluster also confirms a different mechanism, F010 may have multiple independent causes and a fix targeting only H1 would leave the visible symptom intact. This is a real risk worth managing.

The user has chosen a faster validation strategy: design a fix for H1, deploy it, and have the user verify in the live GUI whether the visible F010 symptom (panel solid-black + invisible info pane after zooming into a Card-Blue directory) is eliminated. If yes → H1 was the dominant cause and the methodology terminates with success. If no/partial → the deferred clusters become essential and must be resumed per the protocol in `RESUME-INVESTIGATION.md`.

This is a deviation from the methodology spec's mandatory defense-in-depth. The deviation is justified by:
- Tasks 3.2-3.8 are GUI-driven and require user-in-the-loop reproduction over multiple instrumentation rounds (estimated multi-hour effort).
- Tasks 3.2 onward modify production source for instrumentation — each round requires GUI rebuild + symptom reproduction + revert.
- The fix-first approach VALIDATES the cause-and-effect chain end-to-end (H1's mechanism → F010 visible symptom) in a single user-driven GUI session, replacing 7 cluster-by-cluster GUI sessions.
- A failed validation is not a methodology violation; it triggers resumption per the RESUME-INVESTIGATION protocol with the full hypothesis space still locked and ready.

## Risk acknowledgment

If the H1 fix does not eliminate the visible symptom:
- Either H1 is necessary but not sufficient (a second cause coexists), OR
- H1 is unrelated to the visible symptom (a "real bug but not THE bug" scenario).

In either case, the locked pre-registration table (entries in `hypotheses/`) and the cluster ordering (entry 0003) remain valid — investigation resumes at Task 3.2 in the original plan.

## Next action

1. Halt investigation methodology execution at this entry.
2. Hand off to fix-spec phase: brainstorm a fix for H1 (preferred shape from `forbidden-fix-shapes.md`: add `DrawOp::Clear { color }` variant to `emPainterDrawList`, plus record path and replay handler — fixes the broken path itself rather than dodging it).
3. After fix-spec lands and the fix is implemented, run Task 4.2 (manual GUI verification) as the terminating gate.
4. If Task 4.2 passes → methodology terminates with confirm entry per plan Task 4.2 step 3.
5. If Task 4.2 fails/partial → execute resumption per `RESUME-INVESTIGATION.md`.
