# Phase 3.6.1 Closeout — 2026-04-21

## Summary

- Widened `DialogCheckFinishCb` to match `DialogCycleExt` signature:
  `FnMut(&DialogResult, &mut DlgPanel, &mut EngineCtx<'_>) -> bool`.
- `emFileDialog` installs a validation closure (`on_check_finish`) that
  reads fsb state, spawns the overwrite-confirm dialog (OD), and returns
  `false` on any error/dir/overwrite-needed path — matching the C++
  `emFileDialog::CheckFinish` funnel exactly.
- P3 DIVERGED marker on `emFileDialog::on_cycle_ext` removed.
- Both file-trigger and button-click paths now run through the widened
  `DialogPrivateEngine::Cycle` step 3 `on_check_finish`.

## Commit range

```
fadeb5f9 phase-3.6.1 task 2 ledger: emFileDialog validation funnel landed
bf5b8c84 phase-3.6.1 task 2: emFileDialog installs on_check_finish; P3 divergence closed
2d6a3608 phase-3.6.1 task 1 ledger: callback widening landed
cdc00395 phase-3.6.1 task 1: widen DialogCheckFinishCb to (result, &mut DlgPanel, &mut EngineCtx)
83a17d28 phase-3.6.1 plan: two-task execution + E040 deferred
9827c8bf phase-3.6.1 spec: widen DialogCheckFinishCb to match DialogCycleExt; close P3 divergence
```

## Invariant results

| Check | Expected | Actual | Status |
|---|---|---|---|
| `impl emEngine for DialogPrivateEngine` count | 1 | 1 | PASS |
| `pub fn Cycle(.*PanelCtx` in emDialog/emFileDialog | 0 | 0 | PASS |
| `DIVERGED:.*P3` in emFileDialog | 0 | 0 | PASS |
| `.Cycle(` in emFileDialog | 0 | 0 | PASS |
| nextest | 2513/0/9 | 2513/0/9 | PASS |
| clippy | clean | clean | PASS |
| goldens | 237 passed / 6 failed | 237 passed / 6 failed | PASS |

## E040 status

Still open. Blocked on winit `WindowId` infra: `install_pending_top_level_headless`
takes no `WindowId` parameter and collides when two dialogs (outer FD + OD) both
try to install headlessly. Unchanged from Phase 3.6 closeout.

## Known follow-up

**E041** — `emFileDialog::CheckFinish` / `run_file_dialog_check_finish` duplication.
Task 2 extracted `run_file_dialog_check_finish` as the shared validation body for
the closure path, but left `emFileDialog::CheckFinish` (public method, pre-show path)
with the validation body duplicated. Two call sites encode the C++ algorithm; drift
risk if C++ updates. Track in raw-material as E041.
