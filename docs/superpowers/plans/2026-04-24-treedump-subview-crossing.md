# Tree Dump + Control Channel: Sub-View Crossing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the tree dump and control channel to cross `emSubViewPanel` boundaries so F010-relevant panels (inner-view `emDirPanel` recs under cosmos) become dumpable and navigable. Replace the `/`-separated `panel_path` wire field with emCore-native `{ view?, identity }` addressing.

**Architecture:** Six phases behind the existing `EMCORE_DEBUG_CONTROL=1` gate. Phase 0 fixes a latent SP7 port bug (sub-view context parenting) that blocks C++-faithful dump topology — the fix is safe (verified via three parallel subagent investigations), matches the Rust port's own SP7 spec §3.1, and is prerequisite to everything else. Phases 1–3 build the new dump walker shape (identity resolution, view discovery, context-cascade). Phase 4 cuts over the wire format. Phase 5 reshapes `get_state`. Phase 6 updates docs. Phase 7 runs the F010 canonical capture with the new tooling.

**Tech Stack:** Rust 2021, existing deps only (`serde` / `serde_json` / `std::os::unix::net`). No new crates. Tests use `#[cfg(test)]` in-module style already established.

**Spec:** `docs/superpowers/specs/2026-04-24-treedump-subview-crossing-design.md`.

---

## Preamble — Port bug prerequisite

The spec's dump §(A)4 assumes C++-nested context topology: sub-view emContexts are children of their parent view's emContext. Investigation 2026-04-24 showed the Rust port currently passes `app.context` (root) to `emSubViewPanel::new` at `emMainWindow.rs:976/996`, flattening all four views as siblings under root.

This contradicts the Rust port's own SP7 spec (`docs/superpowers/specs/2026-04-19-emview-sp7-emcontext-threading-design.md` §3.1) which shows the nested pattern as the port target. No commit or annotation justifies the flat choice; it's an unintentional shortcut. Safety analysis (parallel subagent, 2026-04-24) confirmed the fix is purely additive — no code today depends on the flat topology. Phase 0 closes this gap before the dump work.

---

## File structure

| File | Responsibility | Phase(s) |
|------|---|---|
| `crates/emmain/src/emMainWindow.rs` | Pass home view's context to SVP constructors | 0 |
| `crates/emcore/src/emCtrlSocket.rs` | `resolve_identity`, `resolve_target`, command handlers, `get_state`, CtrlCmd/CtrlReply types | 1, 4, 5 |
| `crates/emcore/src/emTreeDump.rs` | `collect_views`, `dump_context` children, per-view `current_frame` | 2, 3 |
| `crates/emcore/src/emView.rs` | `dump_tree` shim simplified (walker now self-contained) | 3 |
| `docs/debug/agent-control-channel.md` | Wire format, command table, recipes | 6 |
| `docs/debug/investigations/F010.md` | Blocked-state preamble, next-steps | 6, 7 |
| `docs/debug/ISSUES.json` | F010 blocked_question rewrite | 6 |

---

## Phase 0 — Port bug fix: sub-view context parenting

Phase goal: sub-view emContexts become children of the home view's emContext. This matches C++ and SP7's design.

### Task 0.1 — Failing test: sub-view context is a child of home view's context

**File:** `crates/emmain/src/emMainWindow.rs` (append to existing `#[cfg(test)]` block — if none exists in this file, add it at the bottom)

- [ ] **Step 1: Find the test module.** Run `grep -n "#\[cfg(test)\]\|mod tests\|mod test" crates/emmain/src/emMainWindow.rs`. If no test module exists, create one at the end of the file:

```rust
#[cfg(test)]
mod port_topology_tests {
    use super::*;
    // (tests go here)
}
```

- [ ] **Step 2: Write the failing test.** In the test module, add:

```rust
/// After `create_main_window`, every sub-view's emContext must be a
/// child of the home view's emContext — not a sibling under the root
/// context. Matches C++ `emSubViewPanel.cpp:114` (`emView((emContext&)
/// superPanel.GetView())`) and SP7 spec §3.1.
#[test]
fn sub_view_contexts_nest_under_home_view_context() {
    // Skip if the test can't run headless; the real end-to-end test
    // lives in an integration test. Here we verify the constructor
    // contract directly.
    //
    // Minimal reconstruction: make a root ctx, a "home view" ctx that
    // is a child of root, and assert that emSubViewPanel::new called
    // with home_view_ctx produces a sub-view whose context's parent is
    // home_view_ctx.
    use emcore::emContext::emContext;
    use emcore::emSubViewPanel::emSubViewPanel;
    use emcore::emPanelTree::PanelTree;
    use emcore::emScheduler::EngineScheduler;
    use std::rc::Rc;

    let root = emContext::NewRoot();
    let home_view_ctx = emContext::NewChild(&root);

    // Minimal SchedCtx + outer panel id (follow the pattern used by
    // the existing emSubViewPanel tests in
    // `crates/emcore/src/emSubViewPanel.rs` around line 655).
    let mut outer_tree = PanelTree::new();
    let outer_root = outer_tree.create_root("root", false);
    let outer_id = outer_tree.create_child(outer_root, "slot", None);
    let mut sched = EngineScheduler::new();
    let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let cb = emcore::emClipboard::FrameworkClipboard::default();
    let pa = std::cell::RefCell::new(Vec::new());
    let wid = winit::window::WindowId::dummy();

    let svp = {
        let mut sc = emcore::emEngineCtx::SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw,
            root_context: &root,
            framework_clipboard: &cb,
            current_engine: None,
            pending_actions: &pa,
        };
        emSubViewPanel::new(Rc::clone(&home_view_ctx), outer_id, wid, &mut sc)
    };

    // The sub-view's context's parent should be home_view_ctx —
    // not root. Use Rc::ptr_eq for identity comparison.
    let sub_parent = svp
        .sub_view
        .GetContext()
        .GetParentContext()
        .expect("sub-view context must have a parent");
    assert!(
        Rc::ptr_eq(&sub_parent, &home_view_ctx),
        "sub-view's context parent should be home_view_ctx, not root or anything else"
    );
    // Also assert it's NOT the root (catches the latent SP7 bug).
    assert!(
        !Rc::ptr_eq(&sub_parent, &root),
        "sub-view's context parent must not be the root context"
    );
}
```

> If the precise `SchedCtx` / `EngineScheduler` / `FrameworkClipboard` constructors differ in this workspace, adapt to the pattern used at `crates/emcore/src/emSubViewPanel.rs` around line 640–665 (the existing `emSubViewPanel::new` test harness). The essential invariant the test verifies does not depend on the SchedCtx surface.

- [ ] **Step 3: Run the test, confirm it would fail under the current code.** This test is purely a contract assertion on `emSubViewPanel::new` — it should pass today *if you pass `home_view_ctx` to the constructor*, because the constructor already uses its argument as the parent. The bug is in the *caller* (`emMainWindow.rs`), not the callee. Proceed to Task 0.2 which validates end-to-end behavior.

```bash
cargo test -p emmain --lib port_topology_tests::sub_view_contexts_nest_under_home_view_context 2>&1 | tail -20
```

Expected: PASS (the constructor does the right thing when given the right parent). This test locks in the *contract*; the bug is that `emMainWindow.rs` violates the contract by passing the wrong parent.

### Task 0.2 — Failing end-to-end test: `create_main_window` produces nested topology

**File:** `crates/emmain/src/emMainWindow.rs` (same test module)

- [ ] **Step 1: Add the end-to-end test.** This one exercises `create_main_window` itself to catch the caller bug:

```rust
/// After `create_main_window`, inspecting the resulting window's view
/// context must show that all three sub-view contexts (control / content
/// / slider... wait, slider is not an emView — just control + content)
/// are nested as children of the home view's context, not siblings of
/// the home view under the root context.
#[test]
#[ignore = "requires winit event loop; run via integration harness only"]
fn create_main_window_produces_nested_subview_contexts() {
    // Intentionally `#[ignore]`d: `create_main_window` needs a real
    // `ActiveEventLoop`, which is a winit construct that cannot be
    // constructed in unit-test context. The assertion instead lives
    // conceptually here and is validated by the integration test in
    // Phase 7 (`F010_subview_dump_nests_under_home_view_context`).
    //
    // This stub documents the contract; the real check runs against
    // a live binary.
    unreachable!("see F010_subview_dump_nests_under_home_view_context in Phase 7");
}
```

- [ ] **Step 2: Run the test to confirm it's ignored (not running).**

```bash
cargo test -p emmain --lib create_main_window_produces_nested_subview_contexts 2>&1 | tail -5
```

Expected: `test ... ignored`.

- [ ] **Step 3: Commit the two-test harness (Task 0.1 + Task 0.2).**

```bash
git add crates/emmain/src/emMainWindow.rs
git commit -m "$(cat <<'EOF'
test(emMainWindow): lock in nested sub-view context-parenting contract

Task 0.1 asserts emSubViewPanel::new's contract directly (passes today).
Task 0.2 stubs the end-to-end assertion, pointing at the Phase 7
integration test that will actually exercise it against a running
binary.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 0.3 — Fix `create_main_window` to pass home view's context

**File:** `crates/emmain/src/emMainWindow.rs:965-999`

- [ ] **Step 1: Read the current call sites to anchor the edits.** Run:

```bash
sed -n '964,1000p' crates/emmain/src/emMainWindow.rs
```

Expected: the two `emSubViewPanel::new(Rc::clone(&app.context), ...)` calls at lines 976 and 996.

- [ ] **Step 2: Replace both call sites.** Use `Edit` tool to change, at `crates/emmain/src/emMainWindow.rs:976`:

```rust
        let mut svp = emSubViewPanel::new(Rc::clone(&app.context), ctrl_id, window_id, &mut sc);
```

to:

```rust
        // SP7 §3.1: sub-view's emContext parents to the outer view's
        // emContext (matching C++ emSubViewPanel.cpp:114). Previously
        // this passed app.context (root), flattening the topology —
        // corrected 2026-04-24 when instrumenting cross-view dump.
        let home_view_ctx = window.view.GetContext().clone();
        let mut svp = emSubViewPanel::new(home_view_ctx, ctrl_id, window_id, &mut sc);
```

And similarly at line 996:

```rust
        let mut svp =
            emSubViewPanel::new(Rc::clone(&app.context), content_id, window_id, &mut sc);
```

→

```rust
        let home_view_ctx = window.view.GetContext().clone();
        let mut svp =
            emSubViewPanel::new(home_view_ctx, content_id, window_id, &mut sc);
```

(The `home_view_ctx` binding is scoped to the inner block each time — no collision.)

- [ ] **Step 3: Run full test suite to confirm nothing regressed.**

```bash
cargo-nextest ntr 2>&1 | tail -30
```

Expected: all tests pass. Agent 3 (safety) confirmed this change is purely additive — if any test fails, read the failure carefully; most likely a test was fixed accidentally, not broken.

- [ ] **Step 4: Commit.**

```bash
git add crates/emmain/src/emMainWindow.rs
git commit -m "$(cat <<'EOF'
fix(emMainWindow): nest sub-view emContext under home view's ctx

Closes SP7 §3.1 latent gap. emSubViewPanel::new was receiving the root
context as parent_context, making all four views siblings under root
instead of nesting control/content sub-views under the home view's
context. C++ emSubViewPanel.cpp:114 nests; the Rust port's own SP7 spec
§3.1 documents the nested pattern as the target; this implementation
took an unannotated flat shortcut.

Safety verified by analysis: context walking is depth-agnostic
(LookupInherited / GetRootContext walk the parent chain regardless of
depth); scheduler dispatch is scope-based, not context-based; no tests
assert the flat topology. The change is purely additive.

Needed for the cross-subview dump work: the C++-faithful topology
requires nested context cascading.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 0.4 — Audit other `emSubViewPanel::new` call sites

**Files:** `crates/emcore/src/emSubViewPanel.rs:663`, `crates/emcore/src/emSubViewPanel.rs:813` (test fixtures)

- [ ] **Step 1: Inspect the test call sites.**

```bash
sed -n '655,670p' crates/emcore/src/emSubViewPanel.rs
sed -n '805,820p' crates/emcore/src/emSubViewPanel.rs
```

- [ ] **Step 2: Decide per site.** These are unit tests constructing a synthetic SVP. If they don't assert on context-tree topology, the context parenting doesn't affect them — the flat vs. nested distinction is invisible inside the SVP itself. **Do not change** these sites; they're intentionally using a minimal single-context harness. The production fix at Task 0.3 is what matters.

- [ ] **Step 3: Add a short note in the test comments so a future reader understands these are different.** In both locations, add above the `emSubViewPanel::new(...)` line:

```rust
// Test harness: uses `emctx` as parent_context for minimal setup. Production
// code (emMainWindow.rs) passes the home view's context per SP7 §3.1 — this
// test doesn't exercise context-tree topology so the simpler harness is fine.
```

- [ ] **Step 4: Commit (docs-only; no behavioral change).**

```bash
git add crates/emcore/src/emSubViewPanel.rs
git commit -m "$(cat <<'EOF'
docs(emSubViewPanel): clarify test-harness parent_context choice

Call out that these test-only sites deliberately use a minimal single-
context harness and don't mirror the production nesting (which is
exercised by emMainWindow.rs). Prevents a future reader from "fixing"
the test to match production and breaking the minimal setup.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 1 — `resolve_identity` pure function

Phase goal: new `resolve_identity(tree, root, identity) → Result<PanelId, String>` with full emCore-native semantics. Replaces `resolve_panel_path` (still present in this phase; its deletion is Task 4.5).

### Task 1.1 — Failing tests for `resolve_identity`

**File:** `crates/emcore/src/emCtrlSocket.rs` — add a new `#[cfg(test)] mod resolve_identity_tests` in the same file (alongside the existing `resolve_panel_path` tests which you'll delete in Task 4.5).

- [ ] **Step 1: Add the test module.** Insert after the last existing test module:

```rust
#[cfg(test)]
mod resolve_identity_tests {
    use super::*;
    use crate::emPanelTree::{EncodeIdentity, PanelTree};

    /// Build an outer-view-shaped tree: root named "root" with three
    /// children named "control view", "content view", "slider".
    fn outer_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", false);
        tree.create_child(root, "control view", None);
        tree.create_child(root, "content view", None);
        tree.create_child(root, "slider", None);
        (tree, root)
    }

    /// Build a cosmos-shaped inner tree: root named "" with one empty-
    /// named child (cosmos), which has a child named "home".
    fn cosmos_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let sub_root = tree.create_root("", true);
        let cosmos = tree.create_child(sub_root, "", None);
        tree.create_child(cosmos, "home", None);
        (tree, sub_root)
    }

    #[test]
    fn empty_identity_addresses_root() {
        let (tree, root) = outer_tree();
        assert_eq!(resolve_identity(&tree, root, "").unwrap(), root);
    }

    #[test]
    fn root_name_addresses_root() {
        let (tree, root) = outer_tree();
        assert_eq!(resolve_identity(&tree, root, "root").unwrap(), root);
    }

    #[test]
    fn multi_segment_outer_tree() {
        let (tree, root) = outer_tree();
        let target = resolve_identity(&tree, root, "root:content view").unwrap();
        assert_eq!(tree.name(target), Some("content view"));
    }

    #[test]
    fn empty_name_inner_root() {
        let (tree, sub_root) = cosmos_tree();
        assert_eq!(resolve_identity(&tree, sub_root, "").unwrap(), sub_root);
    }

    #[test]
    fn single_empty_segment_inner_tree_finds_cosmos() {
        // Sub-root name is ""; cosmos name is "".
        // DecodeIdentity(":") == ["", ""] — first "" matches sub_root,
        // then descend by "" to cosmos.
        let (tree, sub_root) = cosmos_tree();
        let cosmos = resolve_identity(&tree, sub_root, ":").unwrap();
        assert_ne!(cosmos, sub_root);
        assert_eq!(tree.name(cosmos), Some(""));
    }

    #[test]
    fn double_empty_segment_plus_home_finds_home() {
        let (tree, sub_root) = cosmos_tree();
        let home = resolve_identity(&tree, sub_root, "::home").unwrap();
        assert_eq!(tree.name(home), Some("home"));
    }

    #[test]
    fn root_name_mismatch_errors() {
        let (tree, root) = outer_tree();
        let err = resolve_identity(&tree, root, "wrong:anything").unwrap_err();
        assert!(err.contains("identity root mismatch"), "got: {}", err);
    }

    #[test]
    fn missing_segment_errors_with_depth_and_name() {
        let (tree, root) = outer_tree();
        let err = resolve_identity(&tree, root, "root:nonexistent").unwrap_err();
        assert!(err.contains("no such panel"), "got: {}", err);
        assert!(err.contains("nonexistent"), "got: {}", err);
    }

    #[test]
    fn ambiguous_siblings_error() {
        // Manually build a tree with two identically-named siblings.
        let mut tree = PanelTree::new();
        let root = tree.create_root("r", false);
        tree.create_child(root, "dup", None);
        tree.create_child(root, "dup", None);
        let err = resolve_identity(&tree, root, "r:dup").unwrap_err();
        assert!(err.contains("ambiguous identity"), "got: {}", err);
    }

    /// Parametric round-trip: for every panel in a tree,
    /// resolve_identity(tree, root, GetIdentity(tree, p)) must yield p.
    #[test]
    fn round_trip_over_all_panels() {
        for (tree, root) in [outer_tree(), cosmos_tree()] {
            for pid in tree.panel_ids() {
                let id_str = tree.GetIdentity(pid);
                let round_trip = resolve_identity(&tree, root, &id_str)
                    .unwrap_or_else(|e| panic!(
                        "round-trip failed for panel {:?} (identity {:?}): {}",
                        pid, id_str, e
                    ));
                assert_eq!(
                    round_trip, pid,
                    "round-trip mismatch: identity {:?} resolved to wrong panel",
                    id_str
                );
            }
        }
    }
}
```

- [ ] **Step 2: Run tests to confirm compile failure (function doesn't exist yet).**

```bash
cargo test -p emcore --lib emCtrlSocket::resolve_identity_tests 2>&1 | tail -10
```

Expected: compile error — `resolve_identity` not found.

### Task 1.2 — Implement `resolve_identity`

**File:** `crates/emcore/src/emCtrlSocket.rs` — add above the existing `resolve_panel_path`:

- [ ] **Step 1: Write the implementation.**

```rust
/// Resolve an emCore-native identity string to a `PanelId` within
/// `tree`, starting at `root`. `GetIdentity(tree, root)` includes the
/// root's name as the first segment; the decoder consumes `names[0]` as
/// the expected root-name (erroring on mismatch) and descends from
/// `names[1..]`. An empty identity string means "the root itself".
///
/// This is the emCore-native replacement for `resolve_panel_path` which
/// used `/`-separator paths. Identity strings handle empty-named panels
/// and special characters via the existing `EncodeIdentity` /
/// `DecodeIdentity` machinery in `emPanelTree.rs`.
pub(crate) fn resolve_identity(
    tree: &PanelTree,
    root: PanelId,
    identity: &str,
) -> Result<PanelId, String> {
    use crate::emPanelTree::DecodeIdentity;
    let names = DecodeIdentity(identity);
    if names.is_empty() {
        return Ok(root);
    }
    let root_name = tree.name(root).unwrap_or("");
    if names[0] != root_name {
        return Err(format!(
            "identity root mismatch: {:?} does not match root panel name {:?}",
            names[0], root_name
        ));
    }
    let mut cur = root;
    for (i, name) in names[1..].iter().enumerate() {
        let depth = i + 1;
        let matched: Vec<PanelId> = tree
            .children(cur)
            .filter(|&c| tree.name(c) == Some(name.as_str()))
            .collect();
        match matched.len() {
            0 => {
                return Err(format!(
                    "no such panel: {} (segment {} = {:?} not found under {:?})",
                    identity,
                    depth,
                    name,
                    tree.name(cur).unwrap_or("<unnamed>")
                ));
            }
            1 => cur = matched[0],
            n => {
                return Err(format!(
                    "ambiguous identity: {} (segment {} = {:?} matches {} siblings)",
                    identity, depth, name, n
                ));
            }
        }
    }
    Ok(cur)
}
```

- [ ] **Step 2: Run tests to confirm all pass.**

```bash
cargo test -p emcore --lib emCtrlSocket::resolve_identity_tests 2>&1 | tail -15
```

Expected: all 10 tests in `resolve_identity_tests` pass.

- [ ] **Step 3: Commit.**

```bash
git add crates/emcore/src/emCtrlSocket.rs
git commit -m "$(cat <<'EOF'
feat(emCtrlSocket): resolve_identity — emCore-native panel addressing

Replaces /-separated panel_path resolution with emCore-native identity
strings (EncodeIdentity/DecodeIdentity), handling empty-named panels
and special chars via the existing ported machinery. Preserves root-name
semantics (GetIdentity includes the root name; the decoder consumes it).

resolve_panel_path is still present; its deletion is Task 4.5 once all
handlers have been cut over.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 2 — `collect_views` pre-pass

Phase goal: a traversal that walks a view's panel tree, discovers every nested `emSubViewPanel`, and builds a map `ctx_ptr → (&emView, &PanelTree)` so the dump walker can match child contexts back to their views.

### Task 2.1 — Failing test for `collect_views`

**File:** `crates/emcore/src/emTreeDump.rs` — add a `#[cfg(test)] mod collect_views_tests` inside the existing test module.

- [ ] **Step 1: Locate the existing test module header.**

```bash
grep -n "#\[cfg(test)\]\|mod .*tests\s*{" crates/emcore/src/emTreeDump.rs | head -5
```

- [ ] **Step 2: Add the test module.** Append:

```rust
#[cfg(test)]
mod collect_views_tests {
    use super::*;
    use crate::emContext::emContext;
    use crate::emPanelTree::PanelTree;
    use crate::emSubViewPanel::emSubViewPanel;
    use crate::emView::emView;
    use std::rc::Rc;

    /// Construct a minimal outer view with no sub-views.
    #[test]
    fn no_subviews_produces_single_entry() {
        let root_ctx = emContext::NewRoot();
        let home_ctx = emContext::NewChild(&root_ctx);
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", true);
        let view = emView::new(Rc::clone(&home_ctx), root, 1.0, 1.0);

        let map = collect_views(&view, &tree);

        assert_eq!(map.len(), 1);
        assert!(map.contains_key(&Rc::as_ptr(view.GetContext())));
    }

    /// Test harness: build an outer view with one emSubViewPanel
    /// installed at a child slot. Returns (outer_view, outer_tree, svp_id).
    /// Mirrors the pattern at emSubViewPanel.rs:634-664.
    fn build_outer_with_one_subview() -> (
        emView,
        PanelTree,
        crate::emPanelTree::PanelId,
    ) {
        use std::cell::RefCell;

        let root_ctx = emContext::NewRoot();
        let home_ctx = emContext::NewChild(&root_ctx);
        let mut sched = crate::emScheduler::EngineScheduler::new();

        let mut outer_tree = PanelTree::new();
        let outer_root = outer_tree.create_root("outer_root", true);
        outer_tree.init_panel_view(outer_root, None);
        let svp_id = outer_tree.create_child(outer_root, "svp_slot", None);

        let wid = winit::window::WindowId::dummy();

        let svp = {
            let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
            let cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
                RefCell::new(None);
            let pa: Rc<RefCell<Vec<crate::emGUIFramework::DeferredAction>>> =
                Rc::new(RefCell::new(Vec::new()));
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut sched,
                framework_actions: &mut fw,
                root_context: &root_ctx,
                framework_clipboard: &cb,
                current_engine: None,
                pending_actions: &pa,
            };
            // Per Phase 0's fix: SVP receives the outer view's context
            // as parent_context, not the root context.
            emSubViewPanel::new(Rc::clone(&home_ctx), svp_id, wid, &mut sc)
        };
        outer_tree.set_behavior(svp_id, Box::new(svp));

        let outer_view = emView::new(Rc::clone(&home_ctx), outer_root, 1.0, 1.0);
        (outer_view, outer_tree, svp_id)
    }

    /// Construct an outer view with one emSubViewPanel child.
    /// The map should contain two entries (outer + one inner).
    #[test]
    fn one_subview_produces_two_entries() {
        let (view, tree, _svp_id) = build_outer_with_one_subview();
        let map = collect_views(&view, &tree);
        assert_eq!(map.len(), 2, "expected outer view + one inner view");
        assert!(map.contains_key(&Rc::as_ptr(view.GetContext())));
    }

    /// Outer with 2 emSubViewPanels → 3 entries.
    #[test]
    fn multiple_subviews_all_mapped() {
        // Extend the single-subview fixture: add a second svp_slot and
        // install another emSubViewPanel there. Reuse the SchedCtx/Rc
        // pattern above.
        //
        // Implementation hint: factor build_outer_with_N_subviews(n) for
        // reuse; add via a helper `add_subview(&mut tree, outer_root,
        // name, &mut sched, &root_ctx)` that mirrors build_outer... but
        // returns the new svp_id.
        //
        // Assert: map.len() == 3 (outer + 2 inner).
        let (view, tree, _) = build_outer_with_two_subviews();
        let map = collect_views(&view, &tree);
        assert_eq!(map.len(), 3);
    }

    /// Nested: outer → subview → subview → 3 entries.
    #[test]
    fn nested_subview_recursion() {
        // Build outer with one SVP; inside that SVP's sub_tree,
        // install another emSubViewPanel. collect_views must recurse
        // both levels.
        let (view, tree) = build_nested_subview_fixture();
        let map = collect_views(&view, &tree);
        assert_eq!(map.len(), 3, "outer + two levels of nested subviews");
    }

    /// Shared scaffolding: context, scheduler, empty storage cells.
    fn fixture_base() -> (
        Rc<emContext>,
        Rc<emContext>,
        crate::emScheduler::EngineScheduler,
        winit::window::WindowId,
    ) {
        let root_ctx = emContext::NewRoot();
        let home_ctx = emContext::NewChild(&root_ctx);
        let sched = crate::emScheduler::EngineScheduler::new();
        let wid = winit::window::WindowId::dummy();
        (root_ctx, home_ctx, sched, wid)
    }

    /// Install a fresh emSubViewPanel as behavior at the given panel
    /// slot, using the given context as parent_context.
    fn install_svp(
        tree: &mut PanelTree,
        slot_id: crate::emPanelTree::PanelId,
        parent_ctx: &Rc<emContext>,
        root_ctx: &Rc<emContext>,
        sched: &mut crate::emScheduler::EngineScheduler,
        wid: winit::window::WindowId,
    ) {
        use std::cell::RefCell;
        let svp = {
            let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
            let cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
                RefCell::new(None);
            let pa: Rc<RefCell<Vec<crate::emGUIFramework::DeferredAction>>> =
                Rc::new(RefCell::new(Vec::new()));
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: sched,
                framework_actions: &mut fw,
                root_context: root_ctx,
                framework_clipboard: &cb,
                current_engine: None,
                pending_actions: &pa,
            };
            emSubViewPanel::new(Rc::clone(parent_ctx), slot_id, wid, &mut sc)
        };
        tree.set_behavior(slot_id, Box::new(svp));
    }

    fn build_outer_with_two_subviews() -> (
        emView,
        PanelTree,
        (crate::emPanelTree::PanelId, crate::emPanelTree::PanelId),
    ) {
        let (root_ctx, home_ctx, mut sched, wid) = fixture_base();
        let mut tree = PanelTree::new();
        let root = tree.create_root("outer_root", true);
        tree.init_panel_view(root, None);
        let svp1 = tree.create_child(root, "svp1", None);
        let svp2 = tree.create_child(root, "svp2", None);
        install_svp(&mut tree, svp1, &home_ctx, &root_ctx, &mut sched, wid);
        install_svp(&mut tree, svp2, &home_ctx, &root_ctx, &mut sched, wid);
        let view = emView::new(home_ctx, root, 1.0, 1.0);
        (view, tree, (svp1, svp2))
    }

    fn build_nested_subview_fixture() -> (emView, PanelTree) {
        let (root_ctx, home_ctx, mut sched, wid) = fixture_base();
        let mut tree = PanelTree::new();
        let root = tree.create_root("outer_root", true);
        tree.init_panel_view(root, None);
        let outer_svp_id = tree.create_child(root, "outer_svp", None);
        install_svp(
            &mut tree,
            outer_svp_id,
            &home_ctx,
            &root_ctx,
            &mut sched,
            wid,
        );

        // Drill into outer_svp's sub_tree via the existing
        // `as_sub_view_panel_mut` trait method, install a grandchild
        // SVP at one more level. Use with_behavior_as to scope the
        // mutable borrow.
        tree.with_behavior_as::<emSubViewPanel, _>(outer_svp_id, |outer_svp| {
            let inner_ctx = Rc::clone(outer_svp.sub_view.GetContext());
            let sub_tree = outer_svp.sub_tree_mut();
            let sub_root = sub_tree
                .GetRootPanel()
                .expect("sub_tree has a root from emSubViewPanel::new");
            let grand_svp_id = sub_tree.create_child(sub_root, "inner_svp", None);
            install_svp(sub_tree, grand_svp_id, &inner_ctx, &root_ctx, &mut sched, wid);
        });

        let view = emView::new(home_ctx, root, 1.0, 1.0);
        (view, tree)
    }
}
```

> **Engineer note:** the SVP fixture helpers (`fixture_base`, `install_svp`, `build_outer_with_one_subview`, `build_outer_with_two_subviews`, `build_nested_subview_fixture`) are given in full above. Copy them verbatim into the test module. If the `SchedCtx` surface or the `emClipboard::emClipboard` path differs from the test-base pattern at `crates/emcore/src/emSubViewPanel.rs:634-664`, match whatever that file currently uses — the fixture's job is to invoke `emSubViewPanel::new` successfully, nothing more.

- [ ] **Step 3: Run test module to confirm compile failure (`collect_views` doesn't exist).**

```bash
cargo test -p emcore --lib emTreeDump::collect_views_tests::no_subviews_produces_single_entry 2>&1 | tail -10
```

Expected: compile error — `collect_views` not found.

### Task 2.2 — Implement `collect_views`

**File:** `crates/emcore/src/emTreeDump.rs`

- [ ] **Step 1: Add the function.** Insert near the top of the module (after imports, before `dump_panel`):

```rust
/// RUST_ONLY: (language-forced-utility)
///
/// Map of `Rc::as_ptr(emContext) → (&emView, &PanelTree)` built by a
/// single panel-tree walk. C++ uses `dynamic_cast<emView*>(ctx)` inside
/// the context cascade to discover which contexts belong to views; Rust
/// composition has no equivalent cast, and the Rust port's `emView` is
/// not `Rc`'d (it's owned directly by `Window` / `emSubViewPanel`), so
/// a back-pointer on `emContext` is not feasible. Panel-side discovery
/// substitutes: walk each view's panel tree, and wherever an
/// `emSubViewPanel` appears, recurse into its inner view/tree.
///
/// Keys are raw pointer addresses (`Rc::as_ptr`) — used only for
/// equality comparison, never dereferenced.
pub(crate) type ViewMap<'a> = std::collections::HashMap<
    *const crate::emContext::emContext,
    (&'a crate::emView::emView, &'a crate::emPanelTree::PanelTree),
>;

/// Recursively walk `view`'s panel tree; for every `emSubViewPanel`
/// found, descend into its sub-view/sub-tree. Keys the resulting map by
/// each view's context pointer.
pub(crate) fn collect_views<'a>(
    view: &'a crate::emView::emView,
    tree: &'a crate::emPanelTree::PanelTree,
) -> ViewMap<'a> {
    let mut map = ViewMap::new();
    collect_into(view, tree, &mut map);
    map
}

fn collect_into<'a>(
    view: &'a crate::emView::emView,
    tree: &'a crate::emPanelTree::PanelTree,
    out: &mut ViewMap<'a>,
) {
    out.insert(std::rc::Rc::as_ptr(view.GetContext()), (view, tree));
    for pid in tree.panel_ids() {
        if let Some(svp) = tree.behavior(pid).and_then(|b| b.as_sub_view_panel()) {
            collect_into(&svp.sub_view, svp.sub_tree(), out);
        }
    }
}
```

> **Engineer note:** This uses two shared-ref accessors that must exist before Phase 2 runs. `PanelTree::behavior` is added in Task 3.2 Step 1 (as part of Phase 3). `PanelBehavior::as_sub_view_panel` (shared-ref variant — the codebase today only has `as_sub_view_panel_mut`) is added here, in Step 1a below.

- [ ] **Step 1a: Add shared-ref SVP accessor to `PanelBehavior` trait.** In `crates/emcore/src/emPanel.rs`, find the existing `as_sub_view_panel_mut` default and add a shared-ref sibling immediately below it:

```rust
fn as_sub_view_panel(&self) -> Option<&crate::emSubViewPanel::emSubViewPanel> {
    None
}
```

Then in `crates/emcore/src/emSubViewPanel.rs`, find the `fn as_sub_view_panel_mut(&mut self) -> Option<&mut emSubViewPanel>` impl (around line 243) and add a shared-ref sibling:

```rust
fn as_sub_view_panel(&self) -> Option<&emSubViewPanel> {
    Some(self)
}
```

- [ ] **Step 1b: Add `PanelTree::behavior` accessor (shared-ref).** Same change as Task 3.2 Step 1 — pull it forward here so Phase 2 can use it:

```rust
pub(crate) fn behavior(&self, id: PanelId) -> Option<&dyn PanelBehavior> {
    self.panels.get(id)?.behavior.as_deref()
}
```

(When Phase 3 is reached, Task 3.2 Step 1 will note this is already present; proceed to Step 2.)

- [ ] **Step 2: Run tests to confirm `no_subviews_produces_single_entry` passes.**

```bash
cargo test -p emcore --lib emTreeDump::collect_views_tests::no_subviews 2>&1 | tail -10
```

- [ ] **Step 3: Implement the SVP fixture helper in the test module.** Replace `todo_fixture_emSubViewPanel_setup` with a real helper that constructs an outer view with one `emSubViewPanel` child. Model it after `emSubViewPanel.rs:640-665`. When done, flesh out the remaining three tests (`one_subview`, `multiple_subviews`, `nested_subview_recursion`).

- [ ] **Step 4: Run all four tests, confirm pass.**

```bash
cargo test -p emcore --lib emTreeDump::collect_views_tests 2>&1 | tail -15
```

Expected: all four pass.

- [ ] **Step 5: Commit.**

```bash
git add crates/emcore/src/emTreeDump.rs crates/emcore/src/emPanel.rs crates/emcore/src/emSubViewPanel.rs crates/emcore/src/emPanelTree.rs
git commit -m "$(cat <<'EOF'
feat(emTreeDump): collect_views pre-pass for cross-view dump

Walks an outer view's panel tree, discovers every emSubViewPanel, and
recurses into each inner view/tree to build a ctx_ptr → (view, tree)
map. Enables the context cascade in dump_context to match each child
context back to its owning view — necessary because Rust's emContext
lacks a back-pointer to its emView (composition, not inheritance).

RUST_ONLY (language-forced utility): C++ uses dynamic_cast<emView*> at
the context-cascade step; Rust port uses panel-side discovery instead.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 3 — `dump_context` children + per-view `current_frame`

Phase goal: `dump_context` iterates `ctx.children.borrow()`, upgrading each `Weak` and dispatching to `dump_view` (via the pre-pass map) or to a plain child `dump_context`. `dump_panel` threads `current_frame` from the correct view.

### Task 3.1 — Failing test: `dump_context` enumerates children

**File:** `crates/emcore/src/emTreeDump.rs` — add to the existing `dump_context` test module (look for the `dump_context_root_vs_child_titles` test as an anchor).

- [ ] **Step 1: Add the failing test.**

```rust
#[test]
fn dump_context_iterates_child_contexts() {
    let root = crate::emContext::emContext::NewRoot();
    let _child1 = crate::emContext::emContext::NewChild(&root);
    let _child2 = crate::emContext::emContext::NewChild(&root);

    // An empty ViewMap — no views to match, so both children are
    // emitted as plain dump_context recs.
    let view_map = ViewMap::new();
    let rec = dump_context_with_cascade(&root, true, &view_map);
    let children = rec.get_array("Children").expect("Children exists");
    assert_eq!(children.len(), 2, "expected 2 child context recs, got {}", children.len());
}
```

- [ ] **Step 2: Run, confirm failure (`dump_context_with_cascade` not found).**

```bash
cargo test -p emcore --lib emTreeDump::.*dump_context_iterates 2>&1 | tail -5
```

### Task 3.2 — Convert `dump_panel` / `dump_view` to `&PanelTree` (shared borrow)

Before `dump_context_with_cascade` can work, the walker's mutable-borrow requirement on the tree needs to go. Currently `dump_panel` takes `&mut PanelTree` solely for `take_behavior/put_behavior` inside the subtype-extraction block (emTreeDump.rs:187-194). `PanelBehavior::type_name` and `PanelBehavior::dump_state` are both `&self` methods — the mutable path was an accidental requirement that a shared-ref accessor on `PanelTree` dissolves.

**Files:** `crates/emcore/src/emPanelTree.rs`, `crates/emcore/src/emTreeDump.rs`

- [ ] **Step 1: Add shared-borrow `behavior` accessor on `PanelTree`.** Place after `take_behavior` / `put_behavior` (around line 1682):

```rust
/// Shared-borrow access to a panel's behavior. Returns `None` when the
/// panel has no behavior set or has been removed. Use instead of
/// `take_behavior` / `put_behavior` when you only need to call
/// `&self` methods on the behavior (`type_name`, `dump_state`,
/// `as_sub_view_panel`).
pub(crate) fn behavior(&self, id: PanelId) -> Option<&dyn PanelBehavior> {
    self.panels.get(id)?.behavior.as_deref()
}
```

- [ ] **Step 2: Rewrite the subtype extraction in `dump_panel`.** In `emTreeDump.rs:187-194`, replace:

```rust
let (type_name, subtype_pairs) = if let Some(behavior) = tree.take_behavior(id) {
    let n = behavior.type_name().to_string();
    let p = behavior.dump_state();
    tree.put_behavior(id, behavior);
    (n, p)
} else {
    ("(no behavior)".to_string(), Vec::new())
};
```

with:

```rust
let (type_name, subtype_pairs) = match tree.behavior(id) {
    Some(behavior) => (behavior.type_name().to_string(), behavior.dump_state()),
    None => ("(no behavior)".to_string(), Vec::new()),
};
```

- [ ] **Step 3: Change `dump_panel`'s signature** at `emTreeDump.rs:153` from `tree: &mut PanelTree` to `tree: &PanelTree`. Fix all call sites (there is one recursive call at line 283, and `dump_view` at line 377).

- [ ] **Step 4: Change `dump_view`'s signature** from `tree: &mut PanelTree` to `tree: &PanelTree`. Same for call sites.

- [ ] **Step 5: Run tests, confirm compile.**

```bash
cargo check -p emcore 2>&1 | tail -20
cargo test -p emcore --lib emTreeDump 2>&1 | tail -20
```

Expected: clean compile; existing `dump_panel_*` / `dump_view_*` tests still pass.

- [ ] **Step 6: Add the cascade function.** Place after `dump_context`:

```rust
/// Like `dump_context`, but recursively emits child contexts. Each
/// upgraded child is either dispatched to `dump_view` (if present in
/// the pre-pass map) or to itself recursively.
pub(crate) fn dump_context_with_cascade(
    ctx: &crate::emContext::emContext,
    is_root: bool,
    view_map: &ViewMap<'_>,
) -> RecStruct {
    let mut rec = dump_context(ctx, is_root);

    let mut children: Vec<RecValue> = Vec::new();
    for weak in ctx.children.borrow().iter() {
        let Some(child_ctx) = weak.upgrade() else {
            continue; // dead weak — skip
        };
        let ptr = std::rc::Rc::as_ptr(&child_ctx);
        if let Some(&(view, tree)) = view_map.get(&ptr) {
            // Known view: emit the view branch. Now that dump_view
            // takes &PanelTree, we can pass the shared ref directly.
            let view_rec = dump_view(view, tree, view.window_focused);
            children.push(RecValue::Struct(view_rec));
        } else {
            // Plain context: recurse with the same view_map.
            let child_rec = dump_context_with_cascade(&child_ctx, false, view_map);
            children.push(RecValue::Struct(child_rec));
        }
    }

    set_children(&mut rec, children);
    rec
}
```

- [ ] **Step 7: Run the new cascade test, confirm pass.**

```bash
cargo test -p emcore --lib emTreeDump::.*dump_context_iterates 2>&1 | tail -10
```

Expected: `dump_context_iterates_child_contexts` passes.

### Task 3.3 — Failing test: per-view `current_frame` threading

**File:** `crates/emcore/src/emTreeDump.rs`

- [ ] **Step 1: Add the test.** In `collect_views_tests` (or a new `per_view_frame_tests` module):

```rust
#[test]
fn dump_shows_per_view_current_frame_for_subview() {
    // Construct outer view at current_frame=10, inner sub-view at
    // current_frame=3. Dump the outer. Assert: the outer-view rec's
    // child panels show `current: 10`; the inner-view rec's child
    // panels show `current: 3`.
    //
    // Use the collect_views SVP fixture from Task 2.2.
    let (outer_view, outer_tree, inner_view_ptr) = build_outer_with_one_subview();
    outer_view.current_frame.set(10);
    // Navigate to inner view via the fixture's internal access and set
    // its frame counter to 3. (Exact mechanism depends on fixture shape.)
    set_inner_view_frame_via_fixture(&outer_tree, 3);

    let rec = crate::emTreeDump::dump_from_root_context_with_home(
        outer_view.GetContext(),
        &outer_view,
        &outer_tree,
    );
    let rec_text = crate::emRec::write_rec_with_format(&rec, "emTreeDump");
    // Crude but effective: both values must appear, each in the correct
    // view-subtree section.
    assert!(rec_text.contains("current: 10"), "outer view current_frame must appear");
    assert!(rec_text.contains("current: 3"),  "inner view current_frame must appear");
}
```

- [ ] **Step 2: Run, confirm failure.** Expected: compile error (functions don't yet exist) or wrong value.

### Task 3.4 — Thread `current_frame` through the walker

**File:** `crates/emcore/src/emTreeDump.rs`

- [ ] **Step 1: Trace the call chain.** `dump_panel` already takes `current_frame: u64` (see line 153). The caller inside `dump_view` at line 377 passes it in. The problem is only at the cascade transition: when `dump_context_with_cascade` dispatches to `dump_view`, it must call `dump_view(view, tree, ...)` which internally reads `view.current_frame.get()` and passes to `dump_panel`. Check:

```bash
grep -n "current_frame" crates/emcore/src/emTreeDump.rs
```

- [ ] **Step 2: Verify `dump_view` already reads its own `view.current_frame`.** Expected: yes (it already does at line ~377 area). If so, the per-view threading falls out for free — no code change needed; the test merely verifies correctness end-to-end.

- [ ] **Step 3: Run the test, confirm pass.**

### Task 3.5 — New top-level entry point `dump_from_root_context_with_home`

**File:** `crates/emcore/src/emTreeDump.rs`

- [ ] **Step 1: Add the entry point.** Place after `dump_from_root_context`:

```rust
/// Top-level entry — builds General Info rec, runs `collect_views`
/// pre-pass over the home view/tree, then drives the context cascade
/// via `dump_context_with_cascade`.
///
/// Supersedes the split-entry design where `dump_tree` appended the
/// home view rec manually (emView.rs:5019). The cascade now discovers
/// the home view through the root context's `children` list — the home
/// view's context is a child of root by construction (emView::new line
/// 648 calls `NewChild(parent_context)`; the home window passes
/// `app.context` which is the root context).
///
/// Sub-views are reached through the cascade too: each sub-view's
/// context is a child of the home view's context (post-Phase-0 fix at
/// emMainWindow.rs), so `dump_context_with_cascade` recursing into the
/// home view's context will find and emit them.
pub(crate) fn dump_from_root_context_with_home(
    root_ctx: &crate::emContext::emContext,
    home_view: &crate::emView::emView,
    home_tree: &crate::emPanelTree::PanelTree,
) -> RecStruct {
    let title =
        "Tree Dump\nof the top-level objects\nof a running emCore-based program".to_string();
    let text = general_info_text();
    let style = VisualStyle {
        frame: Frame::Rectangle,
        bg: 0x444466,
        fg: 0xBBBBEE,
    };
    let mut rec = empty_rec(title, text, style);

    let view_map = collect_views(home_view, home_tree);
    let ctx_rec = dump_context_with_cascade(root_ctx, true, &view_map);
    set_children(&mut rec, vec![RecValue::Struct(ctx_rec)]);
    rec
}
```

- [ ] **Step 2: Deprecate the old split-entry approach.** Update `emView::dump_tree` at `emView.rs:5007`:

```rust
pub fn dump_tree(&self, tree: &PanelTree) -> std::path::PathBuf {
    let path = std::env::temp_dir().join("debug.emTreeDump");
    let root_ctx = self.GetRootContext();
    let rec = crate::emTreeDump::dump_from_root_context_with_home(&root_ctx, self, tree);
    let text = write_rec_with_format(&rec, "emTreeDump");
    if let Err(e) = std::fs::write(&path, &text) {
        eprintln!("[emView::dump_tree] write failed: {}", e);
    }
    path
}
```

> If this changes the signature of `dump_tree` from `&mut PanelTree` to `&PanelTree`, update the caller in `emWindow.rs:1092` accordingly. Grep: `grep -n "dump_tree" crates/emcore/src/emWindow.rs`.

- [ ] **Step 3: Run the full dump-related test suite, confirm pass.**

```bash
cargo test -p emcore --lib emTreeDump 2>&1 | tail -30
```

- [ ] **Step 4: Commit.**

```bash
git add crates/emcore/src/emTreeDump.rs crates/emcore/src/emView.rs crates/emcore/src/emWindow.rs
git commit -m "$(cat <<'EOF'
feat(emTreeDump): cross-subview context cascade + per-view frame counter

dump_context_with_cascade iterates ctx.children.borrow(), dispatching
each live child to dump_view (if in the ViewMap) or recursing as a
plain child context. dump_from_root_context_with_home wires the pre-
pass + cascade together, replacing the prior split-entry workaround in
emView::dump_tree which manually appended the home view rec.

With Phase 0's nested sub-view context parenting in place, the cascade
naturally reaches:
  root_ctx → home_view → [control_svp_view, content_svp_view, slider_view]

Per-view current_frame threading falls out of dump_view already reading
`view.current_frame.get()` inline (no change needed there).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 4 — `emCtrlSocket` wire-format cutover

Phase goal: `Visit` / `VisitFullsized` / `SetFocus` / `SeekTo` swap `panel_path` for `{ view?, identity }`. `resolve_target` routes by view selector. `resolve_panel_path` and all its tests are deleted.

### Task 4.1 — Failing JSON round-trip test for new command shape

**File:** `crates/emcore/src/emCtrlSocket.rs` — add to the existing JSON test module.

- [ ] **Step 1: Add the tests.**

```rust
#[test]
fn visit_cmd_deserializes_with_view_and_identity() {
    let json = r#"{"cmd":"visit","view":"root:content view","identity":"::home"}"#;
    let cmd: CtrlCmd = serde_json::from_str(json).unwrap();
    match cmd {
        CtrlCmd::Visit { view, identity, adherent } => {
            assert_eq!(view, "root:content view");
            assert_eq!(identity, "::home");
            assert!(!adherent);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn visit_cmd_view_field_defaults_to_empty_string() {
    let json = r#"{"cmd":"visit","identity":"root"}"#;
    let cmd: CtrlCmd = serde_json::from_str(json).unwrap();
    match cmd {
        CtrlCmd::Visit { view, identity, .. } => {
            assert_eq!(view, "");
            assert_eq!(identity, "root");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn seek_to_has_view_and_identity() {
    let json = r#"{"cmd":"seek_to","identity":"root:content view"}"#;
    let cmd: CtrlCmd = serde_json::from_str(json).unwrap();
    match cmd {
        CtrlCmd::SeekTo { view, identity } => {
            assert_eq!(view, "");
            assert_eq!(identity, "root:content view");
        }
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 2: Run, confirm compile error (fields don't exist).**

### Task 4.2 — Update `CtrlCmd` variants

**File:** `crates/emcore/src/emCtrlSocket.rs:68-98`

- [ ] **Step 1: Replace the four targeted variants.**

```rust
    Visit {
        #[serde(default)]
        view: String,
        identity: String,
        #[serde(default)]
        adherent: bool,
    },
    VisitFullsized {
        #[serde(default)]
        view: String,
        identity: String,
    },
    SetFocus {
        #[serde(default)]
        view: String,
        identity: String,
    },
    SeekTo {
        #[serde(default)]
        view: String,
        identity: String,
    },
```

- [ ] **Step 2: Run Phase 4 tests, confirm pass for JSON round-trip.**

```bash
cargo test -p emcore --lib emCtrlSocket 2>&1 | tail -30
```

Expected: the new JSON tests pass; the old `{"panel_path": ...}` round-trip tests now fail.

### Task 4.3 — Implement `resolve_target`

**File:** `crates/emcore/src/emCtrlSocket.rs`

- [ ] **Step 1: Write the failing test.**

```rust
#[cfg(test)]
mod resolve_target_tests {
    use super::*;
    // The full resolve_target test needs an `App` with a populated home
    // window and at least one emSubViewPanel. Factor the integration
    // test fixture here so it mirrors the Phase 7 integration test
    // harness. If that harness doesn't exist yet, add a #[ignore]'d
    // test stub here and implement for real in Phase 7.

    #[test]
    #[ignore = "needs App fixture; integrated via Phase 7"]
    fn resolve_target_outer_view_default() {
        unreachable!()
    }
}
```

- [ ] **Step 2: Add the function.** Place after `resolve_identity`:

```rust
/// Resolve `{ view, identity }` to a concrete (view, tree, panel)
/// triple. `view == ""` targets the home window's outer view; otherwise
/// `view` must resolve (via the outer view's tree) to an
/// `emSubViewPanel`, whose inner view/tree are used for `identity`.
pub(crate) fn resolve_target<'a>(
    app: &'a mut App,
    view_sel: &str,
    identity: &str,
) -> Result<
    (
        &'a mut crate::emView::emView,
        &'a mut crate::emPanelTree::PanelTree,
        PanelId,
    ),
    String,
> {
    let home_id = app
        .home_window_id
        .ok_or_else(|| "home window not initialized".to_string())?;
    let win = app
        .windows
        .get_mut(&home_id)
        .ok_or_else(|| "home window missing".to_string())?;

    if view_sel.is_empty() {
        // Outer view.
        let tree = &mut win.tree;
        let view = &mut win.view;
        let root = tree
            .GetRootPanel()
            .ok_or_else(|| "no root panel".to_string())?;
        let target = resolve_identity(tree, root, identity)?;
        return Ok((view, tree, target));
    }

    // Inner view: resolve view_sel against outer tree; require SVP.
    let outer_root = win
        .tree
        .GetRootPanel()
        .ok_or_else(|| "no root panel".to_string())?;
    let svp_id = resolve_identity(&win.tree, outer_root, view_sel)?;

    // Re-borrow the tree mutably through the SVP accessor to get the
    // inner view + tree.
    let (sub_view, sub_tree) = win
        .tree
        .with_behavior_as::<crate::emSubViewPanel::emSubViewPanel, _>(svp_id, |svp| {
            svp.sub_view_and_tree_mut()
        })
        .flatten()
        .ok_or_else(|| format!("view selector does not refer to a sub-view panel: {}", view_sel))?;

    let sub_root = sub_tree
        .GetRootPanel()
        .ok_or_else(|| "sub-view has no root panel".to_string())?;
    let inner_target = resolve_identity(sub_tree, sub_root, identity)?;
    Ok((sub_view, sub_tree, inner_target))
}
```

> **Engineer note:** The `with_behavior_as` return shape may not match exactly — `.flatten()` assumes the closure returns `Option<(&mut emView, &mut PanelTree)>` and the outer call returns `Option<...>` too. Verify with `grep -n "with_behavior_as" crates/emcore/src/emPanelTree.rs` and adapt. If the borrow checker rejects because `win.tree` is borrowed twice, restructure by computing `svp_id` first (immutable borrow drops) then entering the `with_behavior_as` scope.

- [ ] **Step 3: Run tests, verify compile + existing tests still pass.**

### Task 4.4 — Rewrite the four handlers to use `resolve_target`

**File:** `crates/emcore/src/emCtrlSocket.rs:298-398`

- [ ] **Step 1: Replace `handle_visit`.**

```rust
fn handle_visit(app: &mut App, view_sel: &str, identity: &str, adherent: bool) -> CtrlReply {
    let (view, tree, target) = match resolve_target(app, view_sel, identity) {
        Ok(t) => t,
        Err(e) => return CtrlReply::err(e),
    };
    view.VisitPanel(tree, target, adherent);
    CtrlReply::ok()
}
```

And similarly for `handle_visit_fullsized`, `handle_set_focus`, `handle_seek_to` — same shape, each calls the view's method.

- [ ] **Step 2: Update the dispatch in `handle_main_thread` at line 213-220.**

```rust
        CtrlCmd::Visit { ref view, ref identity, adherent } => handle_visit(app, view, identity, adherent),
        CtrlCmd::VisitFullsized { ref view, ref identity } => handle_visit_fullsized(app, view, identity),
        CtrlCmd::SetFocus { ref view, ref identity } => handle_set_focus(app, view, identity),
        CtrlCmd::SeekTo { ref view, ref identity } => handle_seek_to(app, view, identity),
```

- [ ] **Step 3: Run, confirm tests pass.**

### Task 4.5 — Delete `resolve_panel_path` and its tests

**File:** `crates/emcore/src/emCtrlSocket.rs`

- [ ] **Step 1: Find all references.**

```bash
grep -n "resolve_panel_path" crates/emcore/src/emCtrlSocket.rs
```

- [ ] **Step 2: Delete the function definition (lines ~24-65).**

- [ ] **Step 3: Delete the test module that exercises it.** Find with `grep -n "resolve_panel_path\|resolve_root\|mod .*resolve" crates/emcore/src/emCtrlSocket.rs`.

- [ ] **Step 4: Fix any remaining usage.** Expected: zero — the handlers no longer call it.

- [ ] **Step 5: Remove or update the stale `focused_panel_path` helper at line 400-414.** It's only used by `handle_get_state`, which Phase 5 rewrites. Leave it until Phase 5 removes its caller.

- [ ] **Step 6: Run full test suite.**

```bash
cargo-nextest ntr 2>&1 | tail -30
```

- [ ] **Step 7: Commit Phase 4.**

```bash
git add crates/emcore/src/emCtrlSocket.rs
git commit -m "$(cat <<'EOF'
feat(emCtrlSocket): cut over wire format to { view?, identity }

Visit / VisitFullsized / SetFocus / SeekTo drop panel_path: String in
favor of `view: String` (optional, default "") + `identity: String`.
resolve_target dispatches based on view selector: empty → outer view,
non-empty → resolve against outer tree, require emSubViewPanel, use
its sub-view/sub-tree.

resolve_panel_path is deleted; its tests are replaced by
resolve_identity_tests from Phase 1. No backward compat — protocol
shipped 4 days ago; only in-tree consumers.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 5 — `get_state` reshape

Phase goal: `focused_path: Option<String>` becomes `focused_view: Option<String>` + `focused_identity: Option<String>`. `view_rect` reports the focused view's rect (not always the outer view's). `loading[]` entries gain a `view` field.

### Task 5.1 — Failing test for new `CtrlReply` shape

**File:** `crates/emcore/src/emCtrlSocket.rs`

- [ ] **Step 1: Add tests.**

```rust
#[test]
fn get_state_reply_serializes_new_fields() {
    let reply = CtrlReply {
        ok: true,
        focused_view: Some("root:content view".to_string()),
        focused_identity: Some("::home".to_string()),
        view_rect: Some([0.0, 0.0, 1920.0, 1080.0]),
        loading: vec![LoadingEntry {
            view: "root:content view".to_string(),
            identity: "::home".to_string(),
            pct: 42,
        }],
        ..CtrlReply::default()
    };
    let json = serde_json::to_string(&reply).unwrap();
    assert!(json.contains("\"focused_view\":\"root:content view\""));
    assert!(json.contains("\"focused_identity\":\"::home\""));
    assert!(json.contains("\"view\":\"root:content view\""));
    assert!(!json.contains("focused_path"));
    assert!(!json.contains("panel_path"));
}
```

- [ ] **Step 2: Run, confirm compile error.**

### Task 5.2 — Update `CtrlReply` and `LoadingEntry`

**File:** `crates/emcore/src/emCtrlSocket.rs:147-186`

- [ ] **Step 1: Edit struct fields.**

```rust
#[derive(Debug, Serialize, Default)]
pub struct CtrlReply {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_frame: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_view: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_rect: Option<[f64; 4]>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub loading: Vec<LoadingEntry>,
}

#[derive(Debug, Serialize)]
pub struct LoadingEntry {
    pub view: String,
    pub identity: String,
    pub pct: u32,
}
```

- [ ] **Step 2: Fix compile errors by removing old field usages.**

### Task 5.3 — Implement `focused_pair` algorithm

**File:** `crates/emcore/src/emCtrlSocket.rs`

- [ ] **Step 1: Add function.**

```rust
/// Compute `(focused_view, focused_identity)` for the currently-focused
/// panel. `focused_view = ""` when the outer view is the owner; else
/// `focused_view` is the outer-view identity of the emSubViewPanel
/// containing the focused view.
fn focused_pair(app: &App) -> (Option<String>, Option<String>) {
    let Some(home_id) = app.home_window_id else {
        return (None, None);
    };
    let Some(win) = app.windows.get(&home_id) else {
        return (None, None);
    };
    let outer_view = win.view();
    let outer_tree = win.tree();

    // Build the pre-pass map (same as dump) but read-only.
    let view_map = crate::emTreeDump::collect_views(outer_view, outer_tree);

    // Iterate the map to find which view has focus. In Rust, only one
    // view has `IsFocused() == true` at a time (per SP7 focus
    // semantics); the outer view tracks which sub-view is "inner-
    // focused". Simpler approach: ask the outer view for the focused
    // panel and, if it lives in a sub-tree, walk back to find which
    // SVP owns it. If that panel chain dead-ends in a sub_tree, record
    // the containing SVP's identity.
    //
    // Implementation: ask each view in the map "which panel do you
    // consider focused?" — the one with a non-None answer (and whose
    // view is currently driving UI) is the winner. Then identify its
    // view selector by pointer-matching against the SVP panels in the
    // outer tree.
    for (_ptr, (view, tree)) in view_map.iter() {
        if let Some(pid) = view.GetFocusedPanel() {
            // This view has a focused panel. Now compute view_sel.
            let view_sel = if std::rc::Rc::ptr_eq(view.GetContext(), outer_view.GetContext()) {
                String::new()
            } else {
                // Find the SVP in outer_tree whose sub_view is this one.
                let Some(svp_id) = find_svp_by_inner_view(outer_tree, view) else {
                    continue;
                };
                outer_tree.GetIdentity(svp_id)
            };
            let identity = tree.GetIdentity(pid);
            return (Some(view_sel), Some(identity));
        }
    }
    (None, None)
}

/// Scan the outer tree for an emSubViewPanel whose inner view's context
/// matches `target_view`'s context. Returns the SVP's PanelId.
fn find_svp_by_inner_view(
    outer_tree: &crate::emPanelTree::PanelTree,
    target_view: &crate::emView::emView,
) -> Option<PanelId> {
    for pid in outer_tree.panel_ids() {
        let Some(b) = outer_tree.behavior_ref(pid) else { continue; };
        let Some(svp) = b.as_sub_view_panel() else { continue; };
        if std::rc::Rc::ptr_eq(svp.sub_view.GetContext(), target_view.GetContext()) {
            return Some(pid);
        }
    }
    None
}
```

- [ ] **Step 2: Update `handle_get_state` to use the new pair.**

```rust
fn handle_get_state(app: &App) -> CtrlReply {
    let Some(home_id) = app.home_window_id else {
        return CtrlReply::err("home window not initialized");
    };
    let Some(win) = app.windows.get(&home_id) else {
        return CtrlReply::err("home window missing");
    };
    let outer_view = win.view();

    // For view_rect: use the *focused* view's rect, not always the outer's.
    let (focused_view, focused_identity) = focused_pair(app);
    let view_rect = {
        // Find the focused view by re-running collect_views (cheap).
        let outer_tree = win.tree();
        let view_map = crate::emTreeDump::collect_views(outer_view, outer_tree);
        // Pick the view whose GetFocusedPanel() is Some; else outer.
        let mut picked = outer_view;
        for (_, (v, _)) in view_map.iter() {
            if v.GetFocusedPanel().is_some() {
                picked = v;
                break;
            }
        }
        [picked.CurrentX, picked.CurrentY, picked.CurrentWidth, picked.CurrentHeight]
    };

    CtrlReply {
        ok: true,
        focused_view,
        focused_identity,
        view_rect: Some(view_rect),
        loading: Vec::new(),
        ..CtrlReply::default()
    }
}
```

- [ ] **Step 3: Delete the now-unused `focused_panel_path` helper from line 400-414.**

- [ ] **Step 4: Run all tests.**

```bash
cargo-nextest ntr 2>&1 | tail -40
```

- [ ] **Step 5: Commit Phase 5.**

```bash
git add crates/emcore/src/emCtrlSocket.rs
git commit -m "$(cat <<'EOF'
feat(emCtrlSocket): get_state reports focused_view + focused_identity

Replaces focused_path: Option<String> with decomposed pair at the view
boundary: focused_view (outer-view identity of the containing SVP, ""
for outer) + focused_identity (panel identity within that view). The
pair round-trips directly into visit { view, identity }.

view_rect now reports the *focused* view's CurrentXYWH (not always the
outer view's) — the agent's Q3 question (view-layer zoom divergence)
requires this.

LoadingEntry gains a `view` field so loading panels in sub-views are
addressable by the agent.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 6 — Docs + handoff fixture update

Phase goal: `agent-control-channel.md`, `F010.md`, `ISSUES.json` reflect the new wire format.

### Task 6.1 — Update `agent-control-channel.md`

**File:** `docs/debug/agent-control-channel.md`

- [ ] **Step 1: Inventory sections that reference `panel_path` or `/cosmos/home`.**

```bash
grep -n "panel_path\|/cosmos\|focused_path" docs/debug/agent-control-channel.md
```

- [ ] **Step 2: Update the Quick Start section.** Replace the navigation example:

```bash
# Zoom outer view into the content sub-view
printf '{"cmd":"visit","identity":"root:content view"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
printf '{"cmd":"wait_idle","timeout_ms":30000}\n'         | socat -t35 - UNIX-CONNECT:$SOCK
# Zoom the content sub-view into cosmos-home
printf '{"cmd":"visit","view":"root:content view","identity":"::home"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
printf '{"cmd":"wait_idle","timeout_ms":30000}\n'         | socat -t35 - UNIX-CONNECT:$SOCK
printf '{"cmd":"dump"}\n'                                 | socat -t2 - UNIX-CONNECT:$SOCK
cat /tmp/debug.emTreeDump | head -80
```

- [ ] **Step 3: Rewrite the command table (around line 36-49).**

| Command | Payload | Notes |
|---|---|---|
| `visit` | `view?: String, identity: String, adherent?: bool` | `view` defaults to `""` (outer view). Identity is emCore-native. |
| `visit_fullsized` | `view?, identity` | as above |
| `set_focus` | `view?, identity` | as above |
| `seek_to` | `view?, identity` | as above |
| `get_state` | — | replies `{focused_view, focused_identity, view_rect, loading}` |

- [ ] **Step 4: Rewrite the "Path syntax" section.** Title it "Identity addressing". Explain that identities come from `emPanel::GetIdentity` (colon-separated, backslash-escaped); the `view` selector is the outer-view identity of the containing `emSubViewPanel`. Show examples from the live tree:
  - Outer root: `view=""`, `identity="root"`.
  - Outer SVP: `view=""`, `identity="root:content view"`.
  - Inner view's root: `view="root:content view"`, `identity=""`.
  - Cosmos: `view="root:content view"`, `identity=":"`.
  - Cosmos-home: `view="root:content view"`, `identity="::home"`.

- [ ] **Step 5: Update recipes.** The "F010-style" recipe should be replaced verbatim with the new two-call navigation. Note the between-steps `wait_idle` + `dump` pattern explicitly.

- [ ] **Step 6: Commit.**

```bash
git add docs/debug/agent-control-channel.md
git commit -m "$(cat <<'EOF'
docs(debug): update control-channel reference for identity addressing

Cuts over every recipe, command-table row, and syntax note to the new
{ view?, identity } wire format. Removes /-path references (now
deleted from the protocol). Adds an explicit two-call navigation
recipe showing the outer-then-inner idiom with wait_idle between.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 6.2 — Update F010 scratchpad preamble

**File:** `docs/debug/investigations/F010.md`

- [ ] **Step 1: Append a Phase 3 section documenting what changed.**

```markdown
### Unblocked 2026-04-24: instrumentation extension + port fix

The prior blocked state (2026-04-24 am) was caused by the tree-dump
instrumentation not crossing emSubViewPanel boundaries; see
`docs/superpowers/specs/2026-04-24-treedump-subview-crossing-design.md`
and its plan for the full scope. Key outcomes:

- **Port fix (Phase 0):** `emSubViewPanel::new` callers in
  `emMainWindow.rs` now pass the home view's context as parent_context
  (matching C++ `emSubViewPanel.cpp:114` and the Rust port's own SP7
  spec §3.1). Sub-view emContexts nest under the home view's context;
  model inheritance via `LookupInherited` now has the correct chain.
- **Dump (Phase 3):** `dump_context` iterates `ctx.children.borrow()`.
  The cascade discovers the home view and each sub-view as children,
  emitting their own View + panel-tree recs. `collect_views` pre-pass
  builds the ctx_ptr → (view, tree) map via panel-side walk.
- **Wire format (Phases 1 + 4 + 5):** `{ view?, identity }` replaces
  `panel_path`. emCore-native `GetIdentity` strings address panels.
  `get_state` reports `focused_view` + `focused_identity` (paste-back
  symmetric).

### Next steps (2026-04-24 pm — re-engaged)

1. [x] (Phase 0) Fix sub-view context parenting in emMainWindow.rs.
2. [x] (Phases 1–5) Extend dump + wire format to cross sub-view
   boundaries.
3. [x] (Phase 6) Update agent-control-channel.md and ISSUES.json.
4. [ ] (Phase 7) Run the canonical F010 capture sequence with the new
   instrumentation; answer Q1–Q4 from the dump.
5. [ ] Advance F010 based on evidence: paint-not-running, IsOpaque,
   viewed_rect gate, or view-layer zoom divergence.
```

- [ ] **Step 2: Mark the blocked item at line 34 as unblocked**; add a reference to this Phase 3 section.

- [ ] **Step 3: Update the `head_sha` in the front-matter** to the Phase 6 commit hash (`git rev-parse HEAD`).

- [ ] **Step 4: Commit.**

### Task 6.3 — Update `ISSUES.json`

**File:** `docs/debug/ISSUES.json`

- [ ] **Step 1: Read the current F010 entry.**

```bash
python3 -c "import json; d=json.load(open('docs/debug/ISSUES.json')); print(json.dumps(d.get('F010', d), indent=2))" 2>&1 | head -40
```

- [ ] **Step 2: Update `blocked_question`** to replace any `/cosmos/home`-style fiction with identity addressing: `view="root:content view", identity="::home"`.

- [ ] **Step 3: Set `status` to `re-engaged`** (or whichever status token the tracker uses for "blocker cleared, investigation resuming"). Consult `docs/debug/run_debug.md` Step 5 for the vocabulary.

- [ ] **Step 4: Commit.**

```bash
git add docs/debug/ISSUES.json docs/debug/investigations/F010.md
git commit -m "$(cat <<'EOF'
debug(F010): re-engaged — instrumentation extended + port fix landed

Phases 0–6 of the sub-view crossing plan complete:
- emMainWindow.rs nests sub-view contexts under home view's ctx (SP7 §3.1)
- emTreeDump cascades across subview boundaries via collect_views pre-pass
- Wire format cuts over to { view?, identity } with emCore-native GetIdentity
- get_state decomposes focused panel at the view boundary

Phase 7 runs the canonical F010 capture next.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 7 — Integration test + F010 re-engagement

Phase goal: one `#[ignore]`'d integration test that spawns the binary, drives cross-view navigation, parses the dump, and asserts the cosmos sub-view is reachable. Then run the canonical F010 capture and advance the investigation.

### Task 7.1 — Integration test: cross-view dump + navigation

**File:** `crates/eaglemode/tests/subview_dump_integration.rs` (new file)

- [ ] **Step 1: Write the test.**

```rust
//! Integration test for the F010-unblocking sub-view crossing work.
//! Gated `#[ignore]` because it requires a live display (X11 or Xvfb)
//! and a compiled `eaglemode` binary.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

fn spawn_and_connect() -> (Child, UnixStream) {
    let mut child = Command::new("cargo")
        .args(["run", "--bin", "eaglemode", "--quiet"])
        .env("EMCORE_DEBUG_CONTROL", "1")
        .spawn()
        .expect("spawn eaglemode");
    let pid = child.id();
    let sock_path = PathBuf::from(format!("/tmp/eaglemode-rs.{}.sock", pid));

    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if sock_path.exists() {
            if let Ok(s) = UnixStream::connect(&sock_path) {
                return (child, s);
            }
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("socket did not appear within 30s at {:?}", sock_path);
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn send(s: &mut UnixStream, line: &str) -> String {
    writeln!(s, "{}", line).unwrap();
    let mut reader = BufReader::new(s.try_clone().unwrap());
    let mut buf = String::new();
    reader.read_line(&mut buf).unwrap();
    buf
}

#[test]
#[ignore = "requires display + binary build"]
fn F010_subview_dump_nests_under_home_view_context() {
    let (mut child, mut s) = spawn_and_connect();

    // Baseline dump.
    let reply = send(&mut s, r#"{"cmd":"dump"}"#);
    assert!(reply.contains("\"ok\":true"));

    // Zoom outer → content SVP.
    send(&mut s, r#"{"cmd":"visit","identity":"root:content view"}"#);
    send(&mut s, r#"{"cmd":"wait_idle","timeout_ms":60000}"#);

    // Zoom content sub-view → home.
    send(&mut s, r#"{"cmd":"visit","view":"root:content view","identity":"::home"}"#);
    send(&mut s, r#"{"cmd":"wait_idle","timeout_ms":60000}"#);

    let reply = send(&mut s, r#"{"cmd":"dump"}"#);
    assert!(reply.contains("\"ok\":true"));
    let dump = std::fs::read_to_string("/tmp/debug.emTreeDump").expect("dump file");

    // Assert structural shape.
    assert!(dump.contains("Root Context:"), "must contain root context rec");
    assert!(
        dump.contains("View (Context):"),
        "must contain at least one view rec"
    );
    // Cross-view assertion: the home view's context has child contexts
    // (each an SVP view). At minimum the cosmos sub-view must be
    // reachable — it carries an emDirPanel for /home after the visits.
    assert!(
        dump.contains("emDirPanel"),
        "after visiting home, dump must contain emDirPanel rec"
    );
    assert!(
        dump.matches("View (Context):").count() >= 2,
        "must contain outer view + at least one sub-view"
    );

    // get_state round-trip.
    let gs = send(&mut s, r#"{"cmd":"get_state"}"#);
    assert!(gs.contains("focused_view"));
    assert!(gs.contains("focused_identity"));

    // Clean shutdown.
    send(&mut s, r#"{"cmd":"quit"}"#);
    let _ = child.wait();
}
```

- [ ] **Step 2: Run with `--ignored` against the built binary.**

```bash
cargo build --bin eaglemode
cargo test -p eaglemode --test subview_dump_integration -- --ignored --nocapture 2>&1 | tail -30
```

Expected: test passes; if it fails, the failure message points at what's wrong structurally.

- [ ] **Step 3: Commit the integration test.**

```bash
git add crates/eaglemode/tests/subview_dump_integration.rs
git commit -m "$(cat <<'EOF'
test(subview): integration test for cross-view dump + navigation

Spawns eaglemode with EMCORE_DEBUG_CONTROL=1, drives the two-step
visit sequence (outer → content SVP, then inner → home), dumps, and
asserts the output contains: Root Context, at least two View (Context)
recs (outer + inner), and an emDirPanel (the home directory's panel).

#[ignore]'d because it needs a display; run under Xvfb or desktop.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 7.2 — Run canonical F010 capture

- [ ] **Step 1: Launch the binary with the gate.**

```bash
rm -f /tmp/eaglemode-rs.*.sock /tmp/debug.emTreeDump /tmp/F010_*.emTreeDump
EMCORE_DEBUG_CONTROL=1 ./target/debug/eaglemode > /tmp/eaglemode.stdout 2> /tmp/eaglemode.stderr &
APP_PID=$!
SOCK=/tmp/eaglemode-rs.${APP_PID}.sock
for i in $(seq 1 50); do [ -S "$SOCK" ] && break; sleep 0.1; done
```

- [ ] **Step 2: Capture baseline dump.**

```bash
printf '{"cmd":"dump"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
cp /tmp/debug.emTreeDump /tmp/F010_baseline.emTreeDump
```

- [ ] **Step 3: Drive to cosmos-home.**

```bash
printf '{"cmd":"visit","identity":"root:content view"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
printf '{"cmd":"wait_idle","timeout_ms":60000}\n'         | socat -t70 - UNIX-CONNECT:$SOCK
printf '{"cmd":"visit","view":"root:content view","identity":"::home"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
printf '{"cmd":"wait_idle","timeout_ms":60000}\n'         | socat -t70 - UNIX-CONNECT:$SOCK
```

- [ ] **Step 4: Capture after-navigation dump.**

```bash
printf '{"cmd":"dump"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
cp /tmp/debug.emTreeDump /tmp/F010_after_visit.emTreeDump
printf '{"cmd":"get_state"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
```

- [ ] **Step 5: Analyze per the spec's Q1–Q4.** Locate the `emDirPanel` rec for `/home` in `/tmp/F010_after_visit.emTreeDump`. Read:
  - `loading_done` (Q1 prerequisite): must be `true`.
  - `child_count` and per-child rects (rules out hypothesis B/D-visibility at runtime).
  - **Q1:** the `emDirPanel`'s `LastPaintFrame` vs. `current` in its owning-view rec. Equal → Q1 positive (Paint ran).
  - **Q2:** each `emDirEntryPanel` child's `LastPaintFrame` vs. `current`. Equal → Q2 positive.
  - **Q3:** inner-view `Current XYWH` in the content-sub-view's view rec. Compare with outer's.
  - **Q4:** visit `/root` (identity `"::root"` or whatever identity that dir has) — different behaviour pinpoints permission path.

- [ ] **Step 6: Update `docs/debug/investigations/F010.md`** with evidence under Phase 1/2/3 as `E3.N (path:line)` references. Mark the four hypotheses in Phase 3 CONFIRMED or RULED OUT with citations. If a root cause emerges, advance to Phase 4 per `docs/debug/run_debug.md`; otherwise record new hypothesis + next step.

- [ ] **Step 7: Update `ISSUES.json` status token** per `run_debug.md` Step 5. Commit.

```bash
git add docs/debug/investigations/F010.md docs/debug/ISSUES.json
git commit -m "$(cat <<'EOF'
debug(F010): phase 3 — runtime evidence captured via new instrumentation

[Fill in with actual findings from Task 7.2.5:]
- E3.1 ... (dump path:line)
- Hypothesis X: CONFIRMED / RULED OUT
- Next: [...]

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 8: Clean shutdown.**

```bash
printf '{"cmd":"quit"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
wait $APP_PID
```

---

## Verification checkpoints

Each phase ends with a green build and all tests passing. Canonical commands:

```bash
cargo check
cargo clippy -- -D warnings
cargo-nextest ntr
cargo xtask annotations
```

All four must pass before moving to the next phase.
