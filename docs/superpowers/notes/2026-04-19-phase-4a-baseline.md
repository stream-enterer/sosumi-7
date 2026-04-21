# Phase 4a — Baseline

Captured 2026-04-21 at Bootstrap B7. Branch: `port-rewrite/phase-4a` off `main` at `c94eb3ae` (post phase-3.6.2 merge).

## nextest

```
Summary [  15.126s] 2514 tests run: 2514 passed, 9 skipped
```

## goldens

```
test result: FAILED. 237 passed; 6 failed; 0 ignored
```

Six pre-existing failures (matches B8 expected baseline):
- composition::composition_tktest_1x
- composition::composition_tktest_2x
- notice::notice_window_resize
- test_panel::testpanel_expanded
- test_panel::testpanel_root
- widget::widget_file_selection_box

## clippy

Clean exit 0 (`cargo clippy --all-targets --all-features`).

## rc_refcell_total

304

## diverged_total

180

## rust_only_total

18

## idiom_total

0

## try_borrow_total

0

## Notes

- Phase 3.6.2 closeout (`2026-04-21-phase-3-6-2-closeout.md`) does not include the literal `Status: COMPLETE — all C1–C11 checks passed` line required by ritual B4; the 3.6.x track ran as a sub-phase outside strict B1–B12/C1–C11. Functionally green: merged to main (`c94eb3ae`), clippy clean, 2514 nextest pass, goldens at baseline, E040 + E041 resolved per closeout invariant sweep. Proceeding.
- `.claude/` is untracked in the working tree (local harness settings: `scheduled_tasks.lock`, `settings.local.json`). Not project work; B5 "clean" spirit satisfied.
- Baselines inherit phase-3.6.2 exit state, not the pre-Phase-1 reference numbers (per ritual B7 note).
