# Resumption guide — F010 investigation methodology

If the H1 fix does NOT eliminate the F010 visible symptom (panel solid-black + invisible info pane after zooming into a Card-Blue directory), the investigation methodology must resume from where it was paused at log entry 0010.

This document is the operational contract for that resumption.

## Preconditions to verify before resuming

1. The H1 fix is deployed and behaves as intended at the unit-test layer:
   - `cargo nextest run -p emcore --test f010_h1_clear_recording` — should now FAIL (after the fix, Clear DOES record an op, falsifying H1's prediction). The test was authored to PASS when H1 holds; it FAILS after a correct fix.
   - This inversion is expected. Update `log/0007-h1-experiment.md`'s observation interpretation by writing a NEW `revise` entry referring to it (do not edit 0007 directly per the append-only rule).
2. The visible symptom persists in the live GUI when launched against the fixed binary (zoom into Card-Blue dir, observe solid-black + invisible info pane).
3. `git status` is clean. The investigation worktree at `/home/alex/Projects/eaglemode-rs.f010-investigation` exists and is on branch `f010-investigation`.

## Resumption procedure

### Step 1: Document the failure

Write a new log entry (next available 4-digit ID, currently 0011 if no other entries have been added):

```markdown
---
id: 00NN
type: observe
timestamp: <UTC ISO>
hypothesis_ids: [H1]
supersedes: null
artifacts: [<path to fix's diff or commit hash>]
---

# Manual GUI verification failed for H1 fix

Fix: <description of the H1 fix that was deployed, with commit SHA>

Manual verification: <date> in <scenario>. Symptom <persists / partially persists>: <description>.

H1 is necessary-but-not-sufficient OR unrelated to the visible F010 symptom. The methodology resumes at plan Task 3.2 to find the additional/alternative cause.
```

### Step 2: Resume the plan

Open `docs/superpowers/plans/2026-04-26-F010-investigation-methodology.md`. The remaining tasks in execution order (per cluster ordering at log entry 0003):

- Task 3.2 — H2 singleton (tile pre-fill / `view.background_color`). Cheap: one eprintln site, one GUI run, capture stderr.
- Task 3.3 — dispatch-cluster (P3 + B2 + B3). Three eprintlns at distinct dispatch sites; one GUI rebuild + reproduce; demultiplex stderr per hypothesis; cluster resolution.
- Task 3.4 — invalidation-cluster (P2 + B1 + B8). B1 is a unit test; P2 + B8 are eprintlns + GUI.
- Task 3.5 — Tier-2 standalones (H3 + H4 + H5 + H6 + P1). Mix of GUI runs (H3, P1) and unit tests (H4, H5, H6).
- Task 3.6 — Tier-3 standalones (H7 + H8 + H9 + H10 + P7). Mix of static-analysis reports (H7, H10), unit test (H8), and instrumented GUI (H9, P7).
- Task 3.7 — remaining blind spots (B4 + B6 + B7). B4 is feature-gated cache flush; B6 is USER-DRIVEN multi-machine (write a `decide` entry naming the matrix and suspend); B7 is instrumented GUI.
- Task 3.8 — order-config-cluster (P4 + B5 + H11 + P5). Heaviest: multi-build-config + GUI per config.

After each cluster, follow plan Task 3.1 steps 6-8 (cluster resolution + defense-in-depth advance).

If any cluster ALSO confirms (multi-cause situation), write an `escalate` entry and surface to user before fix-spec.

### Step 3: Manual verification gate (Task 4.2)

After the deferred clusters complete, re-run Task 4.2 with the cumulative fix(es) deployed.

## Special notes

- Plan-author created the harness for cluster `same-observable-with-H1` only (Task 2.1). Tasks 3.2 onwards each include their own harness construction inline. No additional Phase 2 work is required before resumption.
- Pre-registration entries are immutable. If a hypothesis description proves incorrect during resumption, write a `revise` entry (with `supersedes: <YAML ID>`) — do NOT edit the YAML.
- Cluster ordering at log/0003 is locked. Deviations during resumption require new `decide` entries.
- Forbidden fix-shapes at `forbidden-fix-shapes.md` apply to any fix proposed during resumption, not just H1's fix.
