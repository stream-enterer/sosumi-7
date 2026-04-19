# SP4.5-FIX-1 Audit Table

**Date:** 2026-04-19
**Scope:** `crates/emcore/src/{emPanelTree,emPanelCtx,emSubViewPanel,emView}.rs`
**Filter:** `borrow*()` calls on `RefCell<emView>` or `RefCell<EngineScheduler>` only.
**Verdict legend:** `safe` / `vulnerable` / `needs-deeper-analysis`.

| File:line | RefCell type | Borrow kind | Caller class | Verdict | Evidence |
|---|---|---|---|---|---|
| emPanelTree.rs:400 | view | borrow_mut | outermost | safe | `add_to_notice_list` uses `try_borrow_mut` (not bare `borrow_mut`); contention gracefully dropped. Line 400 is `view_rc.try_borrow_mut()`. |
| emPanelTree.rs:577 | view | borrow | outermost | safe | `register_engine_for` uses `try_borrow()` (line 577); called via `create_child` or `init_panel_view`; gracefully returns on contention. |
| emPanelTree.rs:588 | scheduler | borrow_mut | outermost | safe | `register_engine_for` uses `try_borrow_mut()` (line 588) on the scheduler; re-entrant DoTimeSlice context handled by early return. |
| emPanelTree.rs:623 | view | borrow_mut | outermost | safe | `deregister_engine_for` calls `view_rc.borrow_mut()` then immediately calls `queue_or_apply_sched_op` (which uses `try_borrow_mut` on scheduler). Reachable only from `remove()` via `PanelCtx::delete_child/delete_self` where `PanelCycleEngine::Cycle` has already dropped the view borrow before calling behavior. |
| emPanelTree.rs:3196 | view | borrow_mut | outermost | safe | `make_registered_tree` test helper; called only from test code with no enclosing borrow. |
| emPanelTree.rs:3209 | scheduler | borrow | outermost | safe | `sp4_5_panel_engine_registered_at_init_panel_view` test; direct scheduler borrow in test, no enclosing DoTimeSlice. |
| emPanelTree.rs:3214 | scheduler | borrow | outermost | safe | Same test function; direct scheduler borrow in test context only. |
| emPanelTree.rs:3226 | scheduler | borrow | outermost | safe | `sp4_5_child_panel_engine_registered_via_init_propagation` test; direct scheduler borrow in test. |
| emPanelTree.rs:3241 | scheduler | borrow | outermost | safe | `sp4_5_panel_engine_deregistered_on_panel_removal` test; direct scheduler borrow in test. |
| emPanelTree.rs:3246 | scheduler | borrow | outermost | safe | Same test; direct scheduler borrow after `tree.remove()` in test. |
| emPanelTree.rs:3279 | view | borrow_mut | outermost | safe | `sp4_5_register_pending_engines_catches_late_scheduler_attach` test; direct `view.borrow_mut()` in test setup, no engine Cycle running. |
| emPanelTree.rs:3295 | scheduler | borrow | outermost | safe | Same test; `sched.borrow()` after `register_pending_engines()` in test setup. |
| emPanelTree.rs:3305 | scheduler | borrow | outermost | safe | Same test; `sched.borrow()` after `tree.remove(root)` in test cleanup. |
| emPanelTree.rs:3308 | view | borrow_mut | outermost | safe | Same test; `view.borrow_mut()` in explicit cleanup block in test. |
| emPanelTree.rs:3310 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().remove_engine()` in cleanup block. |
| emPanelTree.rs:3313 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().remove_engine()` in cleanup block. |
| emPanelTree.rs:3316 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().remove_engine()` in cleanup block. |
| emPanelTree.rs:3319 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().remove_signal()` in cleanup block. |
| emPanelTree.rs:3350 | scheduler | borrow_mut | outermost | safe | `sp4_5_create_child_from_inside_engine_cycle_does_not_panic` test; `sched.borrow_mut().register_engine()` in test setup before DoTimeSlice. |
| emPanelTree.rs:3357 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().wake_up()` in test setup before DoTimeSlice. |
| emPanelTree.rs:3381 | scheduler | borrow | outermost | safe | Same test; `sched.borrow().get_engine_priority()` after DoTimeSlice in assertion. |
| emPanelTree.rs:3387 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().remove_engine()` in cleanup. |
| emPanelTree.rs:3402 | view | borrow_mut | outermost | safe | `sp4_5_create_child_with_view_already_borrow_mut_does_not_panic` test; explicit `view.borrow_mut()` in test to simulate production state—not a production code path. |
| emPanelTree.rs:3417 | scheduler | borrow | outermost | safe | Same test; `sched.borrow().get_engine_priority()` in assertion after catch-up. |
| emPanelTree.rs:3460 | view | borrow_mut | outermost | safe | `sp4_5_panel_cycle_uses_per_view_pixel_tallness` test; `view_a.borrow_mut()` in test setup before DoTimeSlice. |
| emPanelTree.rs:3461 | view | borrow_mut | outermost | safe | Same test; `view_a.borrow_mut().CurrentPixelTallness` assignment in setup. |
| emPanelTree.rs:3482 | view | borrow_mut | outermost | safe | Same test; `view_b.borrow_mut()` in test setup. |
| emPanelTree.rs:3483 | view | borrow_mut | outermost | safe | Same test; `view_b.borrow_mut().CurrentPixelTallness` assignment in setup. |
| emPanelTree.rs:3495 | scheduler | borrow_mut | outermost | safe | Same test; `sched_a.borrow_mut().wake_up()` in test before DoTimeSlice. |
| emPanelTree.rs:3496 | scheduler | borrow_mut | outermost | safe | Same test; `sched_b.borrow_mut().wake_up()` in test before DoTimeSlice. |
| emPanelTree.rs:3500 | scheduler | borrow_mut | outermost | safe | Same test; `sched_a.borrow_mut().DoTimeSlice()` — this is the outermost scheduler borrow, it is the DoTimeSlice call itself. |
| emPanelTree.rs:3501 | scheduler | borrow_mut | outermost | safe | Same test; `sched_b.borrow_mut().DoTimeSlice()` — outermost scheduler borrow. |
| emPanelTree.rs:3570 | view | borrow_mut | outermost | safe | `sp4_5_wake_up_panel_from_cycle_reaches_sibling` test; `view.borrow_mut()` in test setup before DoTimeSlice. |
| emPanelTree.rs:3598 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().wake_up(eid_a)` in test setup before DoTimeSlice. |
| emPanelTree.rs:3604 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().DoTimeSlice()` — outermost scheduler borrow. |
| emPanelTree.rs:3615 | view | borrow_mut | outermost | safe | Same test; `view.borrow_mut().pending_sched_ops.drain()` after DoTimeSlice — no enclosing borrow. |
| emPanelTree.rs:3621 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut()` to apply drained ops — outermost borrow. |
| emPanelTree.rs:3628 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().DoTimeSlice()` — outermost scheduler borrow. |
| emPanelTree.rs:3644 | view | borrow_mut | outermost | safe | Same test; `view.borrow_mut().pending_sched_ops.drain()` in cleanup — no enclosing borrow. |
| emPanelTree.rs:3645 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut()` to apply cleanup ops — outermost borrow. |
| emPanelCtx.rs:48 | view | borrow_mut | nested-from-Cycle | safe | `wake_up_panel` calls `view_rc.borrow_mut()` then `queue_or_apply_sched_op` (which uses `try_borrow_mut` on scheduler). Reachable from `PanelBehavior::Cycle` via `PanelCycleEngine::Cycle` (`emPanelCycleEngine.rs:42`), which drops the view borrow (taken only at line 33 for tallness) before calling the behavior. View is NOT held during the behavior call. |
| emSubViewPanel.rs:109 | view | borrow | nested-from-Cycle | safe | `GetSubView` borrows `self.sub_view` — a separate `RefCell<emView>` owned by `emSubViewPanel`, not the parent view's RefCell. No cross-borrow possible. |
| emSubViewPanel.rs:114 | view | borrow_mut | nested-from-Cycle | safe | `sub_view_mut` borrows `self.sub_view` — same separate RefCell as above; no parent-view re-entrancy. |
| emSubViewPanel.rs:146 | view | borrow_mut | nested-from-Cycle | safe | `set_sub_view_flags` borrows `self.sub_view` — separate RefCell; called from behavior Cycle/notices, never re-enters parent view. |
| emSubViewPanel.rs:159 | view | borrow_mut | nested-from-Cycle | safe | `sync_geometry` borrows `self.sub_view` — separate RefCell; called from `notice()` which runs inside `PanelCycleEngine::Cycle` after dropping any view borrow. |
| emSubViewPanel.rs:174 | view | borrow_mut | nested-from-Cycle | safe | Same `sync_geometry` function, second `sub_view.borrow_mut()` call; same rationale as line 159. |
| emSubViewPanel.rs:251 | view | borrow | nested-from-both | safe | `Input` borrows `self.sub_view` — separate RefCell; called from `dispatch_input` path or from `Update`. Sub-view RefCell is independent from parent. |
| emSubViewPanel.rs:258 | view | borrow_mut | nested-from-both | safe | Same `Input` function; `sub_view.borrow_mut()` on separate sub-view RefCell. |
| emSubViewPanel.rs:261 | view | borrow | nested-from-both | safe | Same `Input` function; `sub_view.borrow()` on separate RefCell. |
| emSubViewPanel.rs:262 | view | borrow | nested-from-both | safe | Same `Input` function; second `sub_view.borrow()` in the same function. |
| emSubViewPanel.rs:307 | view | borrow_mut | nested-from-Cycle | safe | `Cycle` borrows `self.sub_view` (not the parent view). `PanelCycleEngine::Cycle` drives `emSubViewPanel::Cycle`; sub_view RefCell is independent from parent view. |
| emSubViewPanel.rs:334 | scheduler | borrow | nested-from-Cycle | safe | `Cycle` borrows `self.sub_scheduler` — a separate `RefCell<EngineScheduler>` owned by `emSubViewPanel`, not the parent scheduler. |
| emSubViewPanel.rs:359 | view | borrow | nested-from-both | safe | `Paint` borrows `self.sub_view` for background color — separate sub-view RefCell; paint path holds no parent RefCells. |
| emSubViewPanel.rs:361 | view | borrow_mut | nested-from-both | safe | `Paint` borrows `self.sub_view` for `paint_sub_tree` — separate RefCell. |
| emSubViewPanel.rs:371 | view | borrow | nested-from-both | safe | `GetCursor` borrows `self.sub_view` — separate RefCell. |
| emSubViewPanel.rs:382 | view | borrow | nested-from-Cycle | safe | `drain_parent_invalidation` borrows `self.sub_view` for `is_title_invalid()` — separate RefCell. |
| emSubViewPanel.rs:383 | view | borrow | nested-from-Cycle | safe | Same function; `sub_view.borrow()` for `is_cursor_invalid()`. |
| emSubViewPanel.rs:384 | view | borrow | nested-from-Cycle | safe | Same function; `sub_view.borrow()` for `has_dirty_rects()`. |
| emSubViewPanel.rs:391 | view | borrow_mut | nested-from-Cycle | safe | Same function; `sub_view.borrow_mut()` for `clear_title_invalid()` — separate RefCell. |
| emSubViewPanel.rs:394 | view | borrow_mut | nested-from-Cycle | safe | Same function; `sub_view.borrow_mut()` for `clear_cursor_invalid()` — separate RefCell. |
| emSubViewPanel.rs:403 | view | borrow_mut | nested-from-Cycle | safe | Same function; `sub_view.borrow_mut()` for `take_dirty_rects()` — separate RefCell. |
| emSubViewPanel.rs:427 | view | borrow_mut | outermost | safe | `teardown` test helper; `panel.sub_view.borrow_mut()` in test cleanup — no enclosing borrow. |
| emSubViewPanel.rs:428 | scheduler | borrow_mut | outermost | safe | Same `teardown` test helper; `panel.sub_scheduler.borrow_mut()` in test cleanup. |
| emSubViewPanel.rs:465 | scheduler | borrow | outermost | safe | `sp8_sub_tree_root_panel_engine_registered` test; `panel.sub_scheduler.borrow()` in assertion — test code. |
| emSubViewPanel.rs:478 | scheduler | borrow | outermost | safe | `sp8_cycle_drives_sub_scheduler` test; `panel.sub_scheduler.borrow()` in assertion before Cycle. |
| emSubViewPanel.rs:498 | scheduler | borrow | outermost | safe | Same test; `panel.sub_scheduler.borrow()` in assertion after second Cycle call. |
| emView.rs:255 | view | borrow_mut | outermost | safe | `UpdateEngineClass::Cycle` at line 255: `view_rc.borrow_mut()` is the *initiating* borrow for the engine Cycle; it is not re-entrant — it is the outermost view borrow for this call chain. |
| emView.rs:312 | view | borrow_mut | outermost | safe | `VisitingVAEngineClass::Cycle` at line 312: same pattern — `view_rc.borrow_mut()` is the initiating borrow for this engine; no parent holds the view at this point. |
| emView.rs:660 | scheduler | borrow_mut | nested-from-both | safe | `queue_or_apply_sched_op` uses `try_borrow_mut` (line 660); contention gracefully handled by queuing the op into `pending_sched_ops`. Safe by construction. |
| emView.rs:1832 | scheduler | borrow_mut | nested-from-Update | needs-deeper-analysis | `RawVisitAbs` popup-creation branch calls `sched.borrow_mut()` directly (line 1832). Reachable from `Update` (`emView.rs:2497` SVPChoiceInvalid branch → `RawVisitAbs`) which is called from `UpdateEngineClass::Cycle` (`emView.rs:263`) while `DoTimeSlice` holds `scheduler.borrow_mut()`. Re-entrant scheduler borrow would panic if reached. **Initially classified `vulnerable` and fixed via the SP4.5-FIX-1 template (commits `a67bdc0`, `95d7266`); both reverted.** Template doesn't fit: deferred work needs scheduler access *to produce* four signal IDs that wire up the new `emWindow`, and the natural retry mechanism (`WakeUpUpdateEngine` + next-slice `Update`) does not preserve the `SVPChoiceInvalid` flag that triggered `RawVisitAbs`, so the catch-up tick cannot re-enter the popup branch — the fix would convert the panic into a silent missing-popup. Escalated as **SP4.5-FIX-2** (see closeout note `2026-04-18-emview-subsystem-closeout.md` §8.1 item 16). Path is uncommon (requires popup-zoom + outside-home + `SVPChoiceInvalid` in same slice); panic remains latent. |
| emView.rs:3141 | scheduler | borrow_mut | outermost | safe | `attach_to_scheduler` calls `scheduler.borrow_mut()` in setup; only called from `App::materialize_popup_surface` / window-init paths, never from inside DoTimeSlice or Update. |
| emView.rs:3414 | scheduler | borrow_mut | outermost | safe | `SignalEOIDelayed` calls `sched.borrow_mut()`. Currently called only from test code (`emView.rs:6327`); no production call sites yet. When wired, callers will need to ensure no enclosing scheduler borrow. |
| emView.rs:5017 | scheduler | borrow_mut | outermost | safe | `sp4_phase1_sched_op_routing` test; `scheduler.borrow_mut().create_signal()` in test setup. |
| emView.rs:5021 | scheduler | borrow | outermost | safe | Same test; `scheduler.borrow().is_pending(sig)` in assertion. |
| emView.rs:5025 | scheduler | borrow_mut | outermost | safe | Same test; `scheduler.borrow_mut()` to simulate borrowed state — test-only. |
| emView.rs:5062 | scheduler | borrow_mut | outermost | safe | Same test; `scheduler.borrow_mut().register_engine()` in test setup. |
| emView.rs:5068 | scheduler | borrow_mut | outermost | safe | Same test; `scheduler.borrow_mut().wake_up()` in test setup. |
| emView.rs:5071 | scheduler | borrow_mut | outermost | safe | Same test; `scheduler.borrow_mut().DoTimeSlice()` — outermost scheduler borrow. |
| emView.rs:5072 | scheduler | borrow_mut | outermost | safe | Same test; `scheduler.borrow_mut().remove_engine()` after DoTimeSlice. |
| emView.rs:5979 | scheduler | borrow_mut | outermost | safe | `test_signal_fields_and_visit_by_identity` test; `sched.borrow_mut().create_signal()` in setup. |
| emView.rs:5980 | scheduler | borrow_mut | outermost | safe | Same test; second `sched.borrow_mut().create_signal()` in setup. |
| emView.rs:5991 | scheduler | borrow | outermost | safe | Same test; `sched.borrow().is_pending(cp_sig)` in assertion. |
| emView.rs:6002 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().remove_signal(cp_sig)` in cleanup. |
| emView.rs:6003 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().remove_signal(title_sig)` in cleanup. |
| emView.rs:6318 | scheduler | borrow_mut | outermost | safe | `test_phase7_eoi_engine_fires_after_countdown` test setup; `sched.borrow_mut().register_engine()`. |
| emView.rs:6325 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().connect()` in setup. |
| emView.rs:6330 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().DoTimeSlice()` — outermost scheduler borrow. |
| emView.rs:6341 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().disconnect()` in cleanup. |
| emView.rs:6342 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().remove_engine()` in cleanup. |
| emView.rs:6346 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().remove_engine()` inside cleanup block (view `borrow_mut` open, but this is a SEPARATE `sched` borrow on the scheduler — not re-entrant since it's test-only sequential). |
| emView.rs:6349 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` — sequential test cleanup. |
| emView.rs:6352 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for visiting_va engine. |
| emView.rs:6355 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_signal()`. |
| emView.rs:6380 | scheduler | borrow | outermost | safe | `test_phase7_update_engine_wakeup_via_scheduler` test; `sched.borrow().has_awake_engines()` assertion. |
| emView.rs:6384 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()`. |
| emView.rs:6386 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for visiting_va. |
| emView.rs:6389 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_signal()`. |
| emView.rs:6498 | scheduler | borrow_mut | outermost | safe | `test_phase7_add_to_notice_list_wakes_update_engine` test; `sched.borrow_mut().sleep(eng_id)` in setup. |
| emView.rs:6499 | scheduler | borrow | outermost | safe | Same test; `sched.borrow().has_awake_engines()` assertion before notice. |
| emView.rs:6504 | scheduler | borrow | outermost | safe | Same test; `sched.borrow().has_awake_engines()` assertion after notice. |
| emView.rs:6511 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_signal()`. |
| emView.rs:6513 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine(eng_id)`. |
| emView.rs:6515 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine(visiting_va)`. |
| emView.rs:6565 | scheduler | borrow_mut | outermost | safe | `sp4_signal_fired_from_update_reaches_receiver_same_slice` test setup; `sched.borrow_mut().create_signal()`. |
| emView.rs:6566 | scheduler | borrow_mut | outermost | safe | Same test setup; `sched.borrow_mut().create_signal()`. |
| emView.rs:6567 | scheduler | borrow_mut | outermost | safe | Same test setup; `sched.borrow_mut().create_signal()`. |
| emView.rs:6568 | scheduler | borrow_mut | outermost | safe | Same test setup; `sched.borrow_mut().create_signal()`. |
| emView.rs:6604 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().register_engine()` for Receiver in setup. |
| emView.rs:6632 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().DoTimeSlice()` — outermost scheduler borrow. |
| emView.rs:6634 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().connect(geom_sig, recv_id)` in setup. |
| emView.rs:6648 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().fire(close_sig)` in setup. |
| emView.rs:6652 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().DoTimeSlice()` — outermost scheduler borrow. |
| emView.rs:6661 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().disconnect()`. |
| emView.rs:6662 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_signal()`. |
| emView.rs:6663 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()`. |
| emView.rs:6669 | scheduler | borrow_mut | outermost | safe | Same test cleanup inside view borrow block; `sched.borrow_mut().remove_engine()`. |
| emView.rs:6672 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for visiting_va. |
| emView.rs:6675 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for eoi. |
| emView.rs:6678 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_signal()` for EOISignal. |
| emView.rs:6703 | scheduler | borrow_mut | outermost | safe | `test_phase8_popup_close_signal_zooms_out` test setup; `sched.borrow_mut().create_signal()`. |
| emView.rs:6704 | scheduler | borrow_mut | outermost | safe | Same test setup; second `sched.borrow_mut().create_signal()`. |
| emView.rs:6705 | scheduler | borrow_mut | outermost | safe | Same test setup; third `sched.borrow_mut().create_signal()`. |
| emView.rs:6706 | scheduler | borrow_mut | outermost | safe | Same test setup; fourth `sched.borrow_mut().create_signal()`. |
| emView.rs:6749 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().fire(close_sig)` to trigger popup teardown. |
| emView.rs:6757 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().DoTimeSlice()` — outermost scheduler borrow. |
| emView.rs:6773 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for update. |
| emView.rs:6776 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for visiting_va. |
| emView.rs:6779 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for eoi. |
| emView.rs:6782 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_signal()` for EOISignal. |
| emView.rs:6819 | view | borrow | outermost | safe | `visiting_va_cycles_when_activated` test; `view_rc.borrow()` in assertion — outermost borrow in test. |
| emView.rs:6825 | view | borrow | outermost | safe | Same test; `view_rc.borrow().VisitingVA.borrow()` in assertion. |
| emView.rs:6832 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().wake_up()` before DoTimeSlice. |
| emView.rs:6834 | scheduler | borrow_mut | outermost | safe | Same test; `sched.borrow_mut().DoTimeSlice()` — outermost scheduler borrow. |
| emView.rs:6837 | view | borrow | outermost | safe | Same test; `view_rc.borrow().VisitingVA.borrow()` after DoTimeSlice. |
| emView.rs:6840 | view | borrow_mut | outermost | safe | Same test cleanup; `view_rc.borrow_mut()` in cleanup block. |
| emView.rs:6842 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for update. |
| emView.rs:6845 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for eoi. |
| emView.rs:6848 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_engine()` for visiting_va. |
| emView.rs:6310 | view | borrow_mut | outermost | safe | Test code; `attach_to_scheduler` called from test setup `test_phase7_eoi_engine_fires_after_countdown`, initiating borrow (no enclosing Cycle/Update/DoTimeSlice). |
| emView.rs:6371 | view | borrow_mut | outermost | safe | Test code; `attach_to_scheduler` called from test setup `test_phase7_update_engine_wakeup_via_scheduler`, initiating borrow in test. |
| emView.rs:6379 | view | borrow_mut | outermost | safe | Test code; `WakeUpUpdateEngine()` called from test `test_phase7_update_engine_wakeup_via_scheduler` after setup, no enclosing Cycle/Update. |
| emView.rs:6382 | view | borrow_mut | outermost | safe | Test code; `borrow_mut()` in test cleanup block of `test_phase7_update_engine_wakeup_via_scheduler` to drain pending ops; outermost borrow. |
| emView.rs:6491 | view | borrow_mut | outermost | safe | Test code; `Update(&mut tree)` called from test `test_phase7_add_to_notice_list_wakes_update_engine` directly after setup, not inside Cycle/DoTimeSlice. |
| emView.rs:6509 | view | borrow_mut | outermost | safe | Test code; `borrow_mut()` in test cleanup block of `test_phase7_add_to_notice_list_wakes_update_engine` to drain EOI signal; outermost borrow. |
| emView.rs:6851 | scheduler | borrow_mut | outermost | safe | Same test cleanup; `sched.borrow_mut().remove_signal()` for EOISignal. |
