# SP4.5-FIX-1 Timing Measurements

**Date:** 2026-04-19
**Context:** Spec §3, Task 8 of `docs/superpowers/plans/2026-04-19-sp4-5-fix-1-followups.md`.
**Eagle Mode version measured:** 0.96.4 at `~/git/eaglemode-0.96.4/`.

## Measurement method

Probes added to the C++ tree (source-only; no binary committed):

- `emEngine` private fields: `SP4_5_FIX_1_create_slice` (emUInt64), `SP4_5_FIX_1_panel_name` (const char*), `SP4_5_FIX_1_first_cycle_logged` (bool).
- `emEngine.cpp` constructor: initialises `create_slice = scheduler.GetTimeSliceCounter()`, `panel_name = NULL`, `first_cycle_logged = false`.
- `emPanel.cpp` constructor end: sets `SP4_5_FIX_1_panel_name = Name.Get()`.
- `emScheduler.cpp` before `e->Cycle()`: if `!first_cycle_logged && panel_name != NULL`, emits `SP4_5_FIX_1_PANEL_FIRST_CYCLE name=... delta=...` to stderr and sets `first_cycle_logged = true`.

The delta is `GetTimeSliceCounter() at first Cycle - GetTimeSliceCounter() at emEngine construction`.

Run command:

```bash
cd ~/git/eaglemode-0.96.4
EM_DIR=$(pwd) LD_LIBRARY_PATH=lib timeout 12 ./bin/eaglemode 2>&1 | grep SP4_5_FIX_1 > /tmp/sp4_5_fix_1_cpp_capture.txt
```

169 lines captured (all panels created during the 12-second run).

## Results

| Path | Rust analogue | Representative C++ panel name | Rust delta (slices) | C++ delta (slices) | Difference |
|---|---|---|---|---|---|
| 1 — top-level startup (StartupEngine shape) | `sp4_5_fix_1_timing_top_level_startup_baseline_slices` | `bookmarks` (first panel emitted, created during top-level startup) | 1 | 0 | **+1 (Rust is one slice late)** |
| 2 — top-level mid-Update | `sp4_5_fix_1_timing_top_level_mid_update_baseline_slices` | `AboutEagleMode`, `FS`, `Mandelbrot`, etc. (panels created mid-Update during cosmos rendering) | 1 | 0 | **+1 (Rust is one slice late)** |
| 3 — sub-scheduler | `sp4_5_fix_1_timing_sub_scheduler_baseline_slices` | `ctrl` (panel created under controlView / sub-scheduler) | 1 | 0 | **+1 (Rust is one slice late)** |

All 169 captured C++ panel entries show `delta=0`: in Eagle Mode 0.96.4, every panel's `emEngine::Cycle` is dispatched in the **same** time slice as its construction, via the `WakeUp` call that fires inside the `emPanel` constructor (for the root panel via `View.UpdateEngine->WakeUp()`) or via notice dispatch that wakes the panel's own engine within the ongoing `DoTimeSlice`.

The Rust implementation's deferred-registration pattern (SP4.5-FIX-1 fix: `register_pending_engines()` catch-up sweep after `DoTimeSlice`) always adds one slice of delay.

## C++ instrumentation diff (one-shot, reverted after capture)

```diff
diff --git a/include/emCore/emEngine.h b/include/emCore/emEngine.h
index 413189c..8fbe23e 100644
--- a/include/emCore/emEngine.h
+++ b/include/emCore/emEngine.h
@@ -235,6 +235,7 @@ private:
 
 	friend class emScheduler;
 	friend class emSignal;
+	friend class emPanel; // SP4_5_FIX_1 probe
 
 	void WakeUpImp();
 
@@ -260,6 +261,11 @@ private:
 
 	emUInt64 Clock;
 		// State of emScheduler::Clock after last call to Cycle().
+
+	// SP4_5_FIX_1 probe fields (temporary, one-shot measurement)
+	emUInt64 SP4_5_FIX_1_create_slice;
+	const char * SP4_5_FIX_1_panel_name;
+	bool SP4_5_FIX_1_first_cycle_logged;
 };
 
 inline emScheduler & emEngine::GetScheduler() const
diff --git a/src/emCore/emEngine.cpp b/src/emCore/emEngine.cpp
index 0379f19..724c0ea 100644
--- a/src/emCore/emEngine.cpp
+++ b/src/emCore/emEngine.cpp
@@ -31,6 +31,10 @@ emEngine::emEngine(emScheduler & scheduler)
 	AwakeState=-1;
 	Priority=DEFAULT_PRIORITY;
 	Clock=Scheduler.Clock;
+	// SP4_5_FIX_1 probe init
+	SP4_5_FIX_1_create_slice=Scheduler.GetTimeSliceCounter();
+	SP4_5_FIX_1_panel_name=NULL;
+	SP4_5_FIX_1_first_cycle_logged=false;
 }
 
 
diff --git a/src/emCore/emPanel.cpp b/src/emCore/emPanel.cpp
index 102e04e..f4d0ac9 100644
--- a/src/emCore/emPanel.cpp
+++ b/src/emCore/emPanel.cpp
@@ -154,6 +154,8 @@ emPanel::emPanel(ParentArg parent, const emString & name)
 		View.CursorInvalid=true;
 		View.UpdateEngine->WakeUp();
 	}
+	// SP4_5_FIX_1 probe: record panel name (pointer valid for panel lifetime)
+	SP4_5_FIX_1_panel_name=Name.Get();
 }
 
 
diff --git a/src/emCore/emScheduler.cpp b/src/emCore/emScheduler.cpp
index 4d24a47..0c06f49 100644
--- a/src/emCore/emScheduler.cpp
+++ b/src/emCore/emScheduler.cpp
@@ -19,6 +19,7 @@
 //------------------------------------------------------------------------------
 
 #include <emCore/emEngine.h>
+#include <stdio.h>
 
 
 //==============================================================================
@@ -116,6 +117,14 @@ void emScheduler::DoTimeSlice()
 		e->RNode.Next->Prev=e->RNode.Prev;
 		e->RNode.Prev->Next=e->RNode.Next;
 		CurrentEngine=e;
+		// SP4_5_FIX_1 probe: log first Cycle call per panel
+		if (!e->SP4_5_FIX_1_first_cycle_logged && e->SP4_5_FIX_1_panel_name!=NULL) {
+			e->SP4_5_FIX_1_first_cycle_logged=true;
+			fprintf(stderr,
+				"SP4_5_FIX_1_PANEL_FIRST_CYCLE name=%s delta=%llu\n",
+				e->SP4_5_FIX_1_panel_name,
+				(unsigned long long)(GetTimeSliceCounter()-e->SP4_5_FIX_1_create_slice));
+		}
 		if (!e->Cycle()) {
 			if ((e=CurrentEngine)==NULL) continue;
 			e->Clock=Clock;
```

## Captured stderr (representative — first 20 lines)

```
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=bookmarks delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name= delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=0 delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=1 delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=2 delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=3 delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=4 delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=5 delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=6 delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=7 delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=aux delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=aux delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=aux delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=aux delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=aux delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=aux delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=aux delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=aux delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=autoplay delta=0
SP4_5_FIX_1_PANEL_FIRST_CYCLE name=performance delta=0
```

All 169 entries show `delta=0` (no exceptions). Including `ctrl` (sub-scheduler panel), `bookmarks` (top-level startup), and panels created mid-Update during cosmos rendering.

## Revert confirmation

`git diff -- include/ src/` shows clean (no source diffs) after `git checkout -- include/ src/`. Only built artifacts (bin/, lib/, obj/) remain modified; source tree is restored.

## Decision

Delta on all three paths is 0 in C++ vs 1 in Rust — a consistent **+1 slice drift** from SP4.5-FIX-1's deferred `register_pending_engines()` catch-up sweep. Per spec §3.5 ("If `Rust delta > C++ delta` on any path → file the affected path(s) as a new follow-up item"), this is filed as **SP4.5-FIX-3 — same-slice panel-engine registration**.

### Why this is not a forced divergence

SP4.5-FIX-1 chose the simplest correctness-preserving fix: defer registration to a post-`DoTimeSlice` sweep. That introduces a 1-slice delay because the sweep runs *between* `DoTimeSlice` calls. A *same-slice* fix is achievable without re-entrancy panics:

- Add a `SchedOp::RegisterPanelEngine(panel_id)` variant that, on `apply_via_ctx(&mut EngineCtx<'_>)`, builds a `PanelCycleEngine` adapter from the panel's `View` weak (already on the panel) and inserts it into `EngineCtxInner::engines`, then writes the produced `EngineId` back into `tree.panels[panel_id].engine_id`. `EngineCtx` already exposes `tree: &mut PanelTree` for the write-back.
- `register_engine_for`, on `try_borrow_mut` failure, enqueues the variant on the view's `pending_sched_ops` instead of returning silently.
- The variant is drained at the end of `UpdateEngineClass::Cycle` (after `view.Update` returns) — *still inside* the same outer `DoTimeSlice`. With C++ priority re-ascent semantics already implemented in `EngineCtxInner::wake_up_engine` (`emEngine.rs:236-241`), the newly-registered engine can be picked up in the same slice if it's woken at the same or higher priority.

Net effect: 0-slice drift, matching C++. The current 1-slice drift is a fix-shape choice, not a structural Rust limitation.

### Why not roll same-slice into this spec

Per the parent spec §3.5: "Do not design the fix in this spec." The decision is intentional — this spec measures and decides; the fix is a separate sub-project.

### SP4.5-FIX-3 charter

- Scope: design + implement same-slice panel-engine registration via `SchedOp::RegisterPanelEngine`.
- Deliverables: new `SchedOp` variant + `apply_via_ctx` impl; modified `register_engine_for` to enqueue on contention; the three SP4.5-FIX-1 timing fixtures (`sp4_5_fix_1_timing_*_baseline_slices`) updated to assert `delta == 0` instead of `1`; documentation of the priority-rescent guarantee and any remaining gap.
- Observable goal: all three baseline tests pass with `delta == 0`, matching C++.
- Risk: priority re-ascent only fires if the new engine is woken at the same or higher priority as the currently-scanning queue. If panels register at `Priority::Medium` and `UpdateEngineClass` is also `Medium`, the re-ascent works only if registration happens before the scan steps below `Medium`. Drain-at-end-of-`Cycle` satisfies this for the common case.
- Filed at: `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` §8.1 item 16, alongside the SP4.5-FIX-2 entry.

### Observable impact while SP4.5-FIX-3 is unstarted

Panels in Rust receive their first `Cycle` call one scheduler time slice (~10 ms) later than in C++. Below the user-perceptible threshold for layout / paint / input response, but a real structural divergence from C++ timing. Locked by the Rust baseline tests (commits `b4681d3`, `66decfc`, `d4238d8`) so any improvement (e.g., SP4.5-FIX-3 landing) will surface as a baseline-update need.

**Status: DONE — measurement complete; +1 slice drift filed as SP4.5-FIX-3; no further action in this spec.**
