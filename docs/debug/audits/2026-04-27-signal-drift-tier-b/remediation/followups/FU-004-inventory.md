# FU-004 — D-009 polling-intermediary inventory (verified 2026-05-02)

**Bucket:** [FU-004-d009-polling-sweep.md](FU-004-d009-polling-sweep.md)
**Spec:** `docs/superpowers/specs/2026-05-02-FU-004-d009-inventory-design.md`
**Plan:** `docs/superpowers/plans/2026-05-02-FU-004-d009-inventory.md`

## Method

Enumeration via the spec's five greps over `crates/*/src/`:

1. `Cell<bool>` / `Cell<Option<...>>` / `Cell<usize>` / `Cell<u64>` field declarations.
2. `pending_/do_/to_/needs_/update_*: bool` field declarations.
3. `RefCell<Option<...>>` field declarations.
4. `Vec<Box<dyn Fn>>` and `Rc<RefCell<Vec<Box<dyn>>>` registries (zero hits).
5. `fn Cycle` / `fn cycle` body drain patterns (cross-referenced for fields that escape the naming heuristics).

Filtered out:
- Test fixtures (`emFlagsRec.rs:422 cb` — inside `#[cfg(test)]` `SchedCtxParts`).
- Memoization caches not Cycle-drained (`emFileManTheme.rs:718 cached` — already `DIVERGED: (language-forced)` for sync TGA-load).
- Singleton storage (`emGUIFramework.rs:224 clipboard` — global storage, not a flag).
- Frame counters (`emView.rs:453 current_frame: Cell<u64>` — `Paint`-end increment, not a Cycle-drained flag).
- Cached SignalId for late init (`emMainWindow.rs:84 file_update_signal: Cell<Option<SignalId>>` — set once at `create_main_window`, read to `fire`; no Cycle drain).

## Inventory

| # | Site | Shape | Setters | Drain | C++ counterpart | C++ shape | Classification | Evidence | Action |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `crates/emmain/src/emBookmarks.rs:549` `emBookmarkButton::pending_click_fire` | `Cell<bool>` | `set_pending_click_fire_for_test` (588), no production setter | Cycle (598, 707) | none — input handler unwritten (B-013 stub) | n/a (placeholder for future Input integration) | vestigial | `emBookmarks.rs:705-707` doc-cmt: "input handler — to be wired by B-013 — sets `pending_click_fire`" | scratch-dump entry; cleanup trigger = B-013 input handler implementation |
| 2 | `crates/emmain/src/emMainWindow.rs:77` `emMainWindow::to_close` | `bool` | 171 (`Close`), 383 (autoplay close) | `Cycle` 416 (self-delete) | `emMainWindow.h:118 bool ToClose;` set at `emMainWindow.cpp:163`, drained `cpp:184` | same set-and-Cycle-drain shape | C++-mirrored | `emMainWindow.rs:415` cmt cites C++ cpp:184-187 | add 3-line C++-mirrored doc-comment at field declaration |
| 3 | `crates/emfileman/src/emFileLinkPanel.rs:78` `emFileLinkPanel::do_update` | `bool` | 110, 141, 372, 379, 395, 450 (VFS/UpdateSignal/Model branches) | `LayoutChildren` (per existing cmt) | `emFileLinkPanel.cpp:84-101` `DoUpdate` flag, drained in `LayoutChildren` (cpp:236-296) | same set-and-Cycle-drain shape | C++-mirrored | existing field doc-comment cites cpp:84-101 | none — doc-comment already present (B-016/M-001 cite) |
| 4 | `crates/emfileman/src/emFileLinkPanel.rs:83` `emFileLinkPanel::dir_entry_up_to_date` | `bool` | 111, 142, 176, 378 | 188, 218 (re-resolution gate in LayoutChildren) | `emFileLinkPanel.cpp:90,100` paired with `DoUpdate` | same set-and-Cycle-drain shape | C++-mirrored | existing field doc-comment cites cpp:90, cpp:100 | none — doc-comment already present (B-016/M-001 cite) |
| 5 | `crates/emmain/src/emVirtualCosmos.rs:440` `emVirtualCosmosItemPanel::update_needed` | `bool` | 481, 486, 697 (`SetItemRec`, `notice`) | **none in production code** — no `Cycle` reads it; only test reads at line 1003 | `emVirtualCosmos.cpp:480-485` `UpdateFromRecNeeded=true; WakeUp();` drained `cpp:304-307 Cycle` via `UpdateFromRec` | same set-and-Cycle-drain shape *intended*; Rust port incomplete | needs deeper audit | C++ has full Cycle drain; Rust sets but never drains — fidelity-bug, not D-009 polling — split as own bucket | split into own follow-up (port-completion bucket distinct from FU-004) |
| 6 | `crates/emmain/src/emVirtualCosmos.rs:722` `emVirtualCosmosPanel::needs_update` | `bool` | 934 (`notice` on `VIEWING_CHANGED`) | `Cycle` 923-924 | `emVirtualCosmos.cpp:613` `Notice(NF_VIEWING_CHANGED)` calls `UpdateChildren` synchronously (no flag) | synchronous fire | D-007 candidate | rs:917-919 cmt: "We defer to Cycle since notice() has no PanelCtx." — divergence acknowledged | follow-up spec required (recorded in scratch-dump) |
| 7 | `crates/emstocks/src/emStocksControlPanel.rs:35` `FileFieldPanel::update_controls_needed` | `bool` | 47 (init=true), 53 (drain reset) | drained inside same `update_controls`/Cycle pass | mirrors `emStocksControlPanel.cpp:35,98,102,106,110,225` `UpdateControlsNeeded` | same set-and-Cycle-drain shape | C++-mirrored | C++ cpp:225 `if (UpdateControlsNeeded) UpdateControls();` matched | add 3-line C++-mirrored doc-comment at field declaration |
| 8 | `crates/emstocks/src/emStocksControlPanel.rs:471` `emStocksControlPanel::update_controls_needed` | `bool` | 509, 612, 868, 875, 882, 889 | Cycle 1175 (`if self.update_controls_needed && self.widgets.is_some()`) | `emStocksControlPanel.cpp:35,98,…,225` (same field, parent struct) | same set-and-Cycle-drain shape | C++-mirrored | C++ cpp:225 drain matched | add 3-line C++-mirrored doc-comment at field declaration |
| 9 | `crates/emstocks/src/emStocksItemPanel.rs:39` `CategoryPanel::update_controls_needed` | `bool` | 311, 325, 369 (per-category signal handlers) | drained in same Cycle pass at 348/352 | mirrors C++ `emStocksItemPanel::CategoryPanel::UpdateControlsNeeded` | same set-and-Cycle-drain shape | C++-mirrored | inline drain matches C++ same-Cycle pattern | add 3-line C++-mirrored doc-comment at field declaration |
| 10 | `crates/emstocks/src/emStocksItemPanel.rs:209` `emStocksItemPanel::update_controls_needed` | `bool` | 284, 413, 779, 1043, 1047, 1052, 1080 | 425 (`= false` after UpdateControls) | mirrors C++ `emStocksItemPanel::UpdateControlsNeeded` (same shape as ControlPanel) | same set-and-Cycle-drain shape | C++-mirrored | drain pattern identical to C++ ControlPanel cpp:225 | add 3-line C++-mirrored doc-comment at field declaration |
| 11 | `crates/emcore/src/emPanelTree.rs:224` `emPanel::pending_input` | `bool` | `set_pending_input` (1649) — input dispatcher | `RecurseInput` (clears after dispatch) | `emPanel.h` `PendingInput` field (cited in existing rs cmt) | same set-and-RecurseInput-drain shape | C++-mirrored | existing field doc-comment cites C++ `emPanel::PendingInput` directly | none — doc-comment already present |
| 12 | `crates/emcore/src/emListBox.rs:358` `emListBox::pending_item_trigger_fire` | `bool` | 981 (`trigger_item_internal`, called from `Input`) | `drain_pending_fires` 403 (called from `Input`, same call) | `emListBox.cpp` `Signal(ItemTriggerSignal)` inline in input methods | same-call deferral (drain-at-end-of-Input, not next Cycle) | C++-mirrored | "Phase-3 B3.4c" comment; drain happens in same Input call, not next Cycle — no polling drift | none — doc-comment already present (B3.4c migration cite) |
| 13 | `crates/emcore/src/emTextField.rs:144` `emTextField::pending_text_fire` | `bool` | 2316 (`fire_change`) | 224 (`drain_pending_fires` end-of-Input) | `emTextField.cpp` `Signal(TextSignal)` inline | synchronous fire (in-progress conversion to D-007) | in-progress-migration | doc-cmt: "B3.4c: fire latches … B3.4d setter-path migration drains them" | cross-reference owning track (B3.4c/B3.4d emTextField conversion) — not FU-004 concern |
| 14 | `crates/emcore/src/emTextField.rs:148` `emTextField::pending_selection_fire` | `bool` | 522 (`fire_selection_change`) | 226 (`drain_pending_fires`) | `emTextField.cpp` `Signal(SelectionSignal)` inline | synchronous fire (in-progress conversion to D-007) | in-progress-migration | same B3.4c/d doc-cmt at lines 137-143 | cross-reference owning track (B3.4c/B3.4d emTextField conversion) |
| 15 | `crates/emcore/src/emTextField.rs:149` `emTextField::pending_can_undo_redo_fire` | `bool` | 2324 (`fire_can_undo_redo`) | 227 (`drain_pending_fires`) | `emTextField.cpp` `Signal(CanUndoRedoSignal)` inline | synchronous fire (in-progress conversion to D-007) | in-progress-migration | same B3.4c/d doc-cmt at lines 137-143 | cross-reference owning track (B3.4c/B3.4d emTextField conversion) |
| 16 | `crates/emcore/src/emView.rs:481` `emView::needs_animator_abort` | `bool` | 1375, 1420 (scroll/zoom paths) | window-loop check via `needs_animator_abort()` getter (3538) + clear (3544) | C++ `emView` aborts via `emViewAnimator::Deactivate()` direct call from scroll/zoom mutators | C++ fires synchronously (no flag) | needs deeper audit | "VIEW-003" tracking ID; consumer is winit window loop; whether this is dependency-forced (winit) or D-007-fixable requires reading the abort dispatch chain | split into own follow-up (VIEW-003 audit bucket) |
| 17 | `crates/emcore/src/emFilePanel.rs:73` `emFilePanel::pending_vir_state_fire` | `bool` | 104, 116, 125 (`SetFileModel`, `set_custom_error`, `clear_custom_error`) | `Cycle` start at 169-171 | `emFilePanel.cpp:51,78,87` `Signal(VirFileStateSignal)` inline | synchronous fire | forced retention (language-forced) | existing doc-cmt: "Drained at the start of Cycle. Mirrors C++ synchronous … with a 1-cycle delay (language-forced, same category as B-015 Option B)" | add `DIVERGED: (language-forced)` annotation block at field declaration |

**Total enumerated:** 17 rows.

## Classification counts

- `C++-mirrored`: 9 (rows 2, 3, 4, 7, 8, 9, 10, 11, 12)
- `forced retention`: 1 (row 17)
- `vestigial`: 1 (row 1)
- `in-progress-migration`: 3 (rows 13, 14, 15)
- `D-007 candidate`: 1 (row 6)
- `needs deeper audit`: 2 (rows 5, 16)

## D-007 candidates

### Row 6 — `emVirtualCosmosPanel::needs_update`

**Site:** `crates/emmain/src/emVirtualCosmos.rs:722`.
**Synopsis:** C++ `emVirtualCosmosPanel::Notice(NF_VIEWING_CHANGED)` calls `UpdateChildren()` synchronously (cpp:613). Rust's `notice` lacks a `PanelCtx`, so it sets `needs_update = true` and the next `Cycle` invokes `update_children`. The Rust source explicitly acknowledges the divergence ("We defer to Cycle since notice() has no PanelCtx."). Resolution path: thread `PanelCtx` (or a narrower ctx with the methods needed to drive `update_children`) into the `notice` signature, fire synchronously, drop the flag — matching the canonical CLAUDE.md D-009 fix.
**Follow-up spec:** required. Recorded in `docs/scratch/2026-05-02-future-work-dump.md` under `## FU-004 D-007 candidates`.

## Needs-deeper-audit candidates

### Row 5 — `emVirtualCosmosItemPanel::update_needed`

**Site:** `crates/emmain/src/emVirtualCosmos.rs:440`.
**Synopsis:** C++ `emVirtualCosmosItemPanel::Cycle` checks `UpdateFromRecNeeded` and calls `UpdateFromRec()` (cpp:304-307). The Rust port has the flag and the setters but **does not implement the Cycle drain or the `UpdateFromRec` method** — only `LayoutChildren` and the field-touching `notice`/`SetItemRec` exist. This is an incomplete port (fidelity-bug pending `UpdateFromRec` port), not a D-009 polling-drift question. Outside FU-004 scope.

### Row 16 — `emView::needs_animator_abort`

**Site:** `crates/emcore/src/emView.rs:481`.
**Synopsis:** Tracking ID `VIEW-003`. Set by scroll/zoom paths; consumed by the window loop (winit-driven). Whether the abort can fire synchronously at the mutation site (D-007 candidate) or is forced by the winit event-loop boundary (dependency-forced retention) requires reading the full animator-abort dispatch chain plus the winit redraw integration. Out of FU-004's verification budget (spec §Risk note "verification depth varies").

## Closure note

Honest verification produced **one** D-007 candidate (row 6) and two needs-deeper-audit candidates (rows 5, 16). FU-004 cannot be closed: row 6 needs a follow-up spec, and rows 5/16 each need their own follow-up bucket. The remaining 14 rows are verified non-issues (`C++-mirrored`, `forced retention`, `in-progress-migration`, `vestigial`) and constitute the bulk of the inventory's value as a record preventing re-flagging.

Status: **open** — 1 D-007 candidate + 2 deeper-audit candidates pending follow-ups.
