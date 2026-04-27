# B-015-polling-emcore-plus — P-006 — replace polling with subscribe (emcore + 1 emMainPanel)

**Pattern:** P-006-polling-accessor-present
**Scope:** emcore + emMainPanel singleton
**Row count:** 10
**Mechanical-vs-judgement:** mechanical-heavy
**Cited decisions:** D-005-poll-replacement-shape — direct subscribe collapses each polled comparison into a callback that schedules `Cycle()`, mirroring C++ scheduler semantics.
**Prereq buckets:** none

## Pattern description

Consumer polls cached state per-frame instead of subscribing to an existing accessor. The accessor is ready in Rust; only the subscribe call is missing, so each `Cycle()` re-reads upstream values and compares against locally cached fields. In this bucket the polling sites are concentrated in `emColorField::Cycle` (eight child-widget polls across RGBA + HSV ScalarFields and the Name TextField), plus one `emFilePanel` model-state poll and one `emMainPanel` wall-clock timer poll.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emColorField-245 | src/emCore/emColorField.cpp:245 | crates/emcore/src/emColorField.rs:277 | present | Cycle compares cached `sf_*`/`tf_name` vs `*_out` instead of subscribing per ScalarField |
| emColorField-255 | src/emCore/emColorField.cpp:255 | crates/emcore/src/emColorField.rs:277 | present | Green ScalarField; same polling Cycle |
| emColorField-265 | src/emCore/emColorField.cpp:265 | crates/emcore/src/emColorField.rs:277 | present | Blue ScalarField; same polling Cycle |
| emColorField-277 | src/emCore/emColorField.cpp:277 | crates/emcore/src/emColorField.rs:277 | present | Alpha ScalarField; same polling Cycle |
| emColorField-288 | src/emCore/emColorField.cpp:288 | crates/emcore/src/emColorField.rs:282 | present | Hue ScalarField; same polling Cycle |
| emColorField-298 | src/emCore/emColorField.cpp:298 | crates/emcore/src/emColorField.rs:282 | present | Saturation ScalarField; same polling Cycle |
| emColorField-308 | src/emCore/emColorField.cpp:308 | crates/emcore/src/emColorField.rs:282 | present | Value (brightness) ScalarField; same polling Cycle |
| emColorField-320 | src/emCore/emColorField.cpp:320 | crates/emcore/src/emColorField.rs:285 | present | Name TextField polled via sync_from_children comparing GetText() to cached tf_name |
| emFilePanel-50 | src/emCore/emFilePanel.cpp:50 | crates/emcore/src/emFilePanel.rs:138 | present | C++ AddWakeUpSignal/RemoveWakeUpSignal pair on (un)set FileModel; Rust SetFileModel only stores model_weak |
| emMainPanel-68 | src/emMain/emMainPanel.cpp:68 | crates/emmain/src/emMainPanel.rs:663 | present | emTimer exists; emMainPanel uses wall-clock polling, Cycle returns false so panel is not re-woken |

## C++ reference sites

- src/emCore/emColorField.cpp:245
- src/emCore/emColorField.cpp:255
- src/emCore/emColorField.cpp:265
- src/emCore/emColorField.cpp:277
- src/emCore/emColorField.cpp:288
- src/emCore/emColorField.cpp:298
- src/emCore/emColorField.cpp:308
- src/emCore/emColorField.cpp:320
- src/emCore/emFilePanel.cpp:50
- src/emMain/emMainPanel.cpp:68

## Open questions for the bucket-design brainstorm

- For `emColorField::Cycle` polling four (RGBA) plus three (HSV) child ScalarFields plus one TextField, confirm whether the C++ original subscribes to each child's value signal individually or to an aggregated signal — default is mirror C++ (per D-005).
- For `emFilePanel-50`, confirm the subscribe is to the FileModel's wake-up signal added/removed in `SetFileModel`, matching the C++ AddWakeUpSignal/RemoveWakeUpSignal pair.
- For `emMainPanel-68`, confirm the wall-clock poll should be replaced with an emTimer subscribe (the timer infrastructure exists) and that `Cycle` must return true while the timer is pending so the panel is re-woken.
- Confirm direct-subscribe (not subscribe + mark-dirty) is appropriate even for the N-fold ScalarField subscribes in `emColorField`, given that the consumer collapses cleanly into the callback.
