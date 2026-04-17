# Phase 0 audit — Sonnet 4.6 commits bab81ec + 3675687

Date: 2026-04-17
Auditor: Opus 4.7 (this session)
Refs: spec 2026-04-17-emview-viewing-subsystem-design.md

## bab81ec — RawVisitAbs change-block side effects

C++ (emView.cpp:1803-1806):
- RestartInputRecursion = true
- CursorInvalid = true
- UpdateEngine->WakeUp()
- InvalidatePainting()  // whole-view: CurrentX/Y/Width/Height

Rust (bab81ec at emView.rs in Update change block):
- NOT PORTED comment for RestartInputRecursion (field does not exist)
- self.cursor_invalid = true — matches C++
- NOT PORTED comment for UpdateEngine->WakeUp (no UpdateEngine field)
- self.dirty_rects.push(Rect::new(0, 0, viewport_width, viewport_height))
  — WRONG in principle: C++ invalidates Current* rect, not viewport-sized
  rect. During popup Current* differs from Home*. Without the Home/Current
  split there is no Current rect to invalidate, so this substitute is the
  best the port could do in January's tree shape.

Verdict:
- cursor_invalid = true: KEEP.
- viewport-sized dirty rect: REPLACE in Phase 3 once Home/Current split
  exists — push invalidate_painting(Current rect) instead.
- RestartInputRecursion + UpdateEngine->WakeUp NOT PORTED comments:
  CLOSE in Phase 5 (adds those fields).

## 3675687 — test_update_change_block_side_effects

Current test asserts: dirty_rects contains exactly one rect covering
(0, 0, viewport_width, viewport_height).

Verdict: REWRITE in Phase 3. New assertion: dirty_rects contains one
rect equal to the current_rect (which in non-popup cases will equal
the home rect which equals 0,0,viewport_width,viewport_height — so
the baseline still passes — but the *test body* must read
self.current_x/y/width/height rather than self.viewport_width/height,
so it keeps passing when popup is introduced in Phase 4).
