# B-004-no-wire-misc — P-001 — wire missing accessor + subscribe (misc small scopes)

**Pattern:** P-001-no-subscribe-no-accessor
**Scope:** misc (emcore, emBookmarks, emVirtualCosmos)
**Row count:** 4
**Mechanical-vs-judgement:** balanced — wiring is mechanical once accessor shape is decided; accessor shape is per-scope judgement.
**Cited decisions:** D-003-gap-blocked-fill-vs-stub (gap-fill in scope), D-006-subscribe-shape (canonical wiring).
**Prereq buckets:** none.

**Reconciliation amendments (2026-04-27, post-design 3497069d):**
- **3 accessor groups (G1, G2, G3):** G1 `emFilePanel::GetVirFileStateSignal` (gap-fill on emFilePanel base; in-scope consumer is `emImageFilePanel`), G2 `emBookmarkButton::GetClickSignal` (bespoke per-button accessor; in-scope consumer is the button's own Cycle), G3 `emVirtualCosmosModel::GetChangeSignal` (gap-fill, no in-scope consumer; D-003 option A justifies for future emVirtualCosmosFpPlugin port).
- **emBookmarks-1479 vs the 21 unported rows clarified:** the 21 unported rows reference C++ editing panels (`emBookmarkEntryAuxPanel`, `emBookmarksAuxPanel`) that the Rust port has BLOCKED at `emBookmarks.rs:518` (read-only port). Row 1479 is on `emBookmarkButton` which IS ported (struct exists, paints, has stub Cycle). Actionable drift, not pre-port state.
- **emVirtualCosmos B-004/B-008 distinction:** B-008 row 104 = input edge (`App::file_update_signal` → `Reload()`); B-004 G3 = output edge (`Reload()` → change broadcast). Independent.
- **Soft forward-edges (informational, non-blocking):**
  - B-004 G1 → B-015: once `GetVirFileStateSignal` lands, derived-panel polling rows in B-015 (and any future P-006) gain a subscribe target. B-015 already designed; can stub against the planned accessor.
  - B-004 G3 ↔ B-008: as above; either may land first.
- **Mutator-fire ectx-threading flagged as candidate D-### if rediscovered.** B-008 hit the same shape on `Acquire`; if a third bucket sees it, promote.

## Pattern description

Rust path neither subscribes nor exposes the C++-side signal accessor; both ends of the wire are missing. Fix shape is to port the accessor on the upstream model, then wire the consumer subscribe. This bucket is the small-scope leftover after larger P-001 scopes are bucketed: four heterogeneous rows spanning emCore image-file panels, emMain bookmarks navigation, emVirtualCosmos model-change signalling, and the emFilePanel base-class vir-file-state subscription that several derived panels currently poll.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emImageFile-117 | src/emCore/emImageFile.cpp:117 | crates/emcore/src/emImageFile.rs:85 | missing | Rust port lives in SPLIT file emImageFileImageFilePanel.rs per File and Name Correspondence. |
| emBookmarks-1479 | src/emMain/emBookmarks.cpp:1479 | crates/emmain/src/emBookmarks.rs:528 | missing | Bookmark click navigation unimplemented; emBookmarkButton standalone, does not extend emButton. |
| emVirtualCosmosModel-accessor-model-change | n/a | crates/emmain/src/emVirtualCosmos.rs:213 | missing | model-change accessor missing on emVirtualCosmosModel (no C++ line in packet). |
| emFilePanel-accessor-vir-file-state | n/a | crates/emcore/src/emFilePanel.rs:100 | missing | emFilePanel-derived panels (image, dir, dirstat, filelink, stocksfile) poll vir_file_state in Cycle instead of subscribing. |

## C++ reference sites

- src/emCore/emImageFile.cpp:117
- src/emMain/emBookmarks.cpp:1479

## Open questions for the bucket-design brainstorm

- Per D-003: for emVirtualCosmosModel-accessor-model-change, is the gap a missing accessor on a ported model (fill in scope) or a missing model entirely (escalate — bucket cannot complete without out-of-scope porting)?
- Per D-003: same check for emFilePanel-accessor-vir-file-state — is emFilePanel ported sufficiently that the accessor can be added in-bucket, or does the vir_file_state state itself need porting first?
- emBookmarks-1479: emBookmarkButton is a standalone struct rather than an emButton derivative; does the fix require restructuring emBookmarkButton to extend emButton (so the click_signal accessor pattern applies), or is a bespoke accessor on emBookmarkButton in scope?
- emImageFile-117: the C++ site is in emImageFilePanel constructor and the Rust port is in the SPLIT file emImageFileImageFilePanel.rs — confirm the wire belongs in the SPLIT file (not the primary emImageFile.rs).
- Once emFilePanel-accessor-vir-file-state is filled, the five derived panels (image, dir, dirstat, filelink, stocksfile) currently polling vir_file_state become downstream consumers — are they remediated in this bucket as part of the wire, or are they separate P-006/P-007 rows in their own buckets?
