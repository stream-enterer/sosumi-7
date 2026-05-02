# FU-003 — emView multi-view content/control split port

**Pattern:** upstream port of a structural emView feature.
**Scope:** `emcore` (emView), with downstream effects on `emmain` (emBookmarks) and any other consumer of multi-view navigation.
**Row count:** 1 direct Tier-B tie-in (B-004 row `emBookmarks-1479`); broader downstream surface unenumerated.
**Prereq buckets:** none.

## Pattern description

C++ `emView` supports a content-view / control-view split: one navigation event can target the active content view while leaving the control view positioned independently. The Rust port has only single-view navigation. B-004 (`emBookmarks-1479`) annotated this as `DIVERGED: (upstream-gap-forced)` because faithful porting of the navigation reaction requires the multi-view split to exist first.

## Items

| ID | Site | Description |
|---|---|---|
| FU-003-1 | `crates/emcore/src/emView.rs` (+ supporting files) | Port emView's content/control view split: separate active-view tracking, navigation routing, and any emCore consumers that branch on view kind. |
| FU-003-2 | `crates/emmain/src/emBookmarks.rs:724` | Replace `DIVERGED: (upstream-gap-forced)` with the real multi-view-aware navigation reaction matching C++ `emBookmarks.cpp` behavior. |
| FU-003-3 | (sweep) | Audit other DIVERGED/UPSTREAM-GAP annotations citing emView single-view limits. None known at Tier-B close, but a scan is the first phase. |

## Acceptance

- `emView` exposes the same content/control-view API surface as C++ (or a documented forced-divergence subset).
- B-004 row 1479 reaction body matches C++; the `upstream-gap-forced` annotation is removed.
- Golden tests for any view-navigation flows pass.

## Notes

- This is a large standalone port effort, not a Tier-B-style row sweep. Treat it as its own audit/plan, not a follow-up bucket execution.
- Gate any other Tier-A/B/C work that names emView multi-view as a blocker on this bucket's completion.
- Recommend a dedicated brainstorm pass before plan-writing — the C++ shape involves coordinated emView/emPanel/emWindow changes that will not survive a quick scope.
