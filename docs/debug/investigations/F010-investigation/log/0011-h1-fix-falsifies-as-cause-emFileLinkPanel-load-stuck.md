---
id: 0011
type: observe
timestamp: 2026-04-27T09:13:39Z
hypothesis_ids: [H1, B2]
supersedes: null
artifacts:
  - "git:1740cea4 (fix(F010): add DrawOp::Clear variant — close recording-mode dispatch hole)"
  - "/tmp/f010-trace2.txt (instrumented GUI trace, panel-type correlation)"
  - "/tmp/f010-trace3.txt (path-discrimination + viewport pixel sampling)"
  - "/tmp/f010-trace4.txt (Cycle/AutoExpand/AutoShrink/notice trace)"
---

# H1 fix is real but does not address the visible F010 symptom; root cause located in emFileLinkPanel load lifecycle

## Summary

The H1 fix at commit 1740cea4 (`DrawOp::Clear` variant + replay handler + `emPainter::Clear` wired through `try_record`) is mechanically correct and verified by the inverted unit test in `crates/emcore/tests/f010_h1_clear_recording.rs`. However, manual GUI verification per plan Task 4.2 confirmed that the visible F010 symptom (panel solid-black + invisible info pane after zooming into a Card-Blue directory) **persists unchanged** after the fix.

Resumed investigation per the protocol in `RESUME-INVESTIGATION.md` skipped directly to the diagnostic phase using staged GUI instrumentation. The visible black is **not** caused by the recording-mode `Clear()` dispatch hole. It is caused by an unrelated bug in `emFileLinkPanel`'s file-load lifecycle that closely matches the spirit of pre-registered hypothesis B2 (panel state machine reaches VFS_LOADED in production) but applies to `emFileLinkPanel`, not `emDirPanel`.

## What was done

1. Wrote `decide` entry 0010 committing to skip the methodology's defense-in-depth Tasks 3.2–3.8 in favor of fix-first validation.
2. Designed and implemented the H1 fix per `docs/superpowers/specs/2026-04-26-F010-h1-fix-design.md` and `docs/superpowers/plans/2026-04-26-F010-h1-fix.md`. Single commit at 1740cea4.
3. User performed manual GUI verification: launched `cargo run -p eaglemode`, navigated to the Card-Blue scene, attempted the symptom reproduction. Result: symptom persists, completely unchanged from the unfixed binary (verified across both debug and release builds).
4. Per RESUME-INVESTIGATION.md preconditions, resumed investigation. Staged four instrumentation passes against the same GUI binary, refining the diagnostic each cycle. All instrumentation written as `eprintln!` calls in production source, captured to /tmp/f010-trace{2,3,4}.txt, then reverted in the same wrap-up commit as this log entry.

## What was learned

### Visible symptom mechanism

- The visible "black panel interior" is the cosmos `emStarFieldPanel` background showing through a region that should be filled by the top-level `emFileLinkPanel`s but isn't.
- At the symptomatic zoom, three top-level `emFileLinkPanel`s (panel IDs 8v1, 9v1, 10v1) are dispatched on every frame and consistently observed in `vfs=Waiting` state for the entire trace (38/38 paint calls).
- `Waiting` is "not good" by `VirtualFileState::is_good()`. `emFileLinkPanel::Paint` takes path 1: `paint_status(painter, ...)` which paints status text only and does NOT fill the panel's background. The cosmos starfield's BLACK paint (drawn earlier in tree-walk order) shows through.
- Three nested `emFileLinkPanel`s (14v1, 15v1, 16v1, children of `emDirEntryPanel`s) reach `vfs=Loaded` reliably (10/10 paint calls). Their `Clear(0xBBBBBBFF)` (path 2) fills their tiny clip rects with light grey — these are the small light-grey rectangles visible at viewport center in the user's screenshots.
- The "stuck black" hysteresis is therefore: top-level emFileLinkPanel's vfs is stuck at Waiting, so its big region is never filled, so cosmos black persists. The recovery sequence (zoom out twice + zoom in) eventually nudges the load forward; once vfs reaches Loaded, path 2 fills the entire panel area with light grey.

### Root cause: missing model-change signal handling in Rust port

C++ `emFileLinkPanel::Cycle` (`src/emFileMan/emFileLinkPanel.cpp:77-107`) subscribes to four wake-up signals (constructor lines 53-56): `UpdateSignalModel->Sig`, `GetVirFileStateSignal()`, `Config->GetChangeSignal()`, and `Model->GetChangeSignal()`. When any of these fires, `Cycle` re-runs `UpdateDataAndChildPanel`, which re-checks `full_path` and re-drives the load.

The Rust port's `Cycle` (`crates/emfileman/src/emFileLinkPanel.rs:175-182`) does not subscribe to any of these signals and does not call `update_data_and_child_panel`. It only calls `refresh_vir_file_state()` — an observation, not a driver. Per-frame Cycle output confirms: `vfs_before=Waiting vfs_after=Waiting` (no transition occurs).

`AutoExpand` is the one site that calls `model.ensure_loaded()`. AutoExpand fires once when the panel first becomes auto-expandable. Trace shows AutoExpand fires 3 times (once per top-level panel) with `full_path=""` because the panel's `full_path` field has not yet been updated from the model's resolved path. `ensure_loaded()` is a no-op when there is no path to load against. Result: `vfs_after=Waiting`.

Subsequent path resolution (model determines its target path through its own asynchronous mechanism) is observable via `notice CHILD_LIST_CHANGED` events firing later — children are eventually created — but `ensure_loaded()` is never called again, and `vfs` remains stuck at `Waiting` for the panel's entire lifetime.

### Relationship to pre-registered hypotheses

This finding closely matches **B2** (`Panel state machine reaches VFS_LOADED in production`) in spirit, but B2's hypothesis YAML targets `emDirPanel`, not `emFileLinkPanel`. The same mechanism (state machine fails to reach Loaded for the symptomatic panel) is the actual cause; the panel type is different.

Per methodology, this is not a strict B2 confirmation — the YAML's `experimental_design` and `falsification_criterion` reference `emDirPanel.rs:454`, which is a different file and different panel type. Treating it as B2 confirmed would conflate two distinct panels with the same bug pattern.

The investigation methodology expected hypothesis space to be locked at end of Phase 1 (entry 0004). This finding is outside the locked set. The handoff to fix-spec proceeds with the understanding that:
- H1 was confirmed mechanically (entry 0007) and the fix landed (commit 1740cea4) — but H1 is not the cause of the visible F010 symptom.
- B2's MECHANISM (state machine doesn't reach Loaded) is what's at play, but for `emFileLinkPanel` not `emDirPanel`.
- The other 21 deferred hypotheses (H2–H10, P1–P5, P7, B1, B3–B8) were not run; per the resume protocol they remain pre-registered and locked. None of them target `emFileLinkPanel`'s Cycle/wake-up signal subscription either.

## What was completed

1. H1 fix shipped (commit 1740cea4): `DrawOp::Clear` variant added; recording-mode dispatch hole closed; all 7 panel callers' Clear paths now record correctly.
2. H1 inverted unit test passes (`f010_h1_clear_records_one_op`).
3. New round-trip test (`replay_clear_matches_direct`) in `emPainterDrawList.rs::tests` proves Clear records and replays equivalently to direct paint.
4. Manual GUI verification per plan Task 4.2: SYMPTOM PERSISTS. H1 dispatch hole was a real defect but is not load-bearing for the visible F010 symptom.
5. Root cause located: `emFileLinkPanel::Cycle` is missing C++'s model-change signal handling. Top-level emFileLinkPanels never call `ensure_loaded()` after their `full_path` resolves. vfs stays at Waiting; path 1 paints status text without bg fill; cosmos starfield BLACK persists in the panel area.

## Next steps

1. **Brainstorm fix-spec** for the emFileLinkPanel load-lifecycle bug. Two candidate fix shapes were discussed in-session:
   - **Faithful port:** subscribe to the model's change signal in `set_link_model`, handle the signal in `Cycle` by calling `update_data_and_child_panel` and (transitively) `ensure_loaded` whenever the model changes. Mirrors C++ behavior. CLAUDE.md port-ideology preferred.
   - **Defensive fix:** in `Cycle`, if `full_path` is non-empty AND `vfs == Waiting` AND `model.is_some()`, call `model.ensure_loaded()` again. Quick, catches the late-path-resolution case.
2. The fix-spec brainstorming is being done in a NEW session/context (per user's explicit handoff at end of this investigation cycle). This log entry is the input.
3. After the fix-spec lands and the fix is implemented, manual GUI verification should be re-attempted. If the symptom resolves, F010 closes. If not, return to RESUME-INVESTIGATION.md with this entry as the new starting context.

## Methodology compliance

- Append-only log: this entry is new (id 0011); no prior entries (0001–0010) modified.
- Hypothesis YAMLs in `hypotheses/` remain immutable since the Phase 1 lock at entry 0004.
- Production code is clean: all instrumentation eprintlns added during this resumption phase have been reverted in the same commit that lands this log entry. The only retained production-code change from this cycle is the H1 fix at 1740cea4.
- Forbidden-fix-shapes (`forbidden-fix-shapes.md`) was honored throughout: the H1 fix added a record-path variant rather than dodging the broken path; no feature flags or dispatch reroutes were used.
