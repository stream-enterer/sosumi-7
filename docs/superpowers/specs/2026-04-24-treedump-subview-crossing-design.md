# 2026-04-24 — Tree Dump + Control Channel: Sub-View Crossing

**Status:** design approved, ready for writing-plans
**Primary motivation:** unblock F010. The 2026-04-24 tree-dump port shipped assuming a single `emView`, but the `cosmos` panel tree lives inside the home window's content `emSubViewPanel` as a nested `emView` + `emPanelTree`. Neither the dump walker nor the path resolver crosses that boundary, so the F010-relevant panels (`emDirPanel` for `/home`, its children) are unaddressable and invisible. This extension makes the nested view tree dumpable and navigable.
**Supersedes (partially):** §"View selection and path resolution" and "Multi-view navigation addressing" non-goal of `2026-04-24-treedump-port-design.md`.

---

## Motivation

The previous spec declared "Multi-view / multi-window navigation addressing" a non-goal and built the control channel around the home window's single focused view. Observed 2026-04-24 when attempting to execute the F010 handoff:

- Home window's panel tree is the outer `emView` with root `emMainPanel`, children `{control view, content view, slider}`. All three are `emSubViewPanel` instances, each holding its own `emView` + `emPanelTree` in `sub_view` / `sub_tree` fields (`emSubViewPanel.rs:23-30`).
- `emMainContentPanel` is the root of the `content view` sub-tree; cosmos is its empty-named child; directories like `home` are cosmos's children. F010 lives three levels down in this nested tree.
- `emCtrlSocket::resolve_panel_path` walks only the outer view's tree (`emCtrlSocket.rs:34-60`); it emits `no such panel: /cosmos (segment 'cosmos' not found under 'root')` for any sub-view path.
- `emTreeDump::dump_panel` recurses only within one `emPanelTree` (`emTreeDump.rs:280`); the `emSubViewPanel` rec appears as a leaf with empty children in the dump. No `emDirPanel` rec for `/home` exists in any output the channel produces.

This blocks all four F010 runtime questions (Q1–Q4 in `ISSUES.json#F010.blocked_question`). The blocker is the instrumentation's missing cross-boundary behavior; the underlying program structure is correct and upstream-faithful (verified: C++ `emMainWindow.cpp:303` constructs `emMainContentPanel` with name `""` and `emMainContentPanel.cpp:29` constructs `emVirtualCosmosPanel` with name `""` — both empty names match the Rust port verbatim).

## Goals

- Tree dump crosses `emSubViewPanel` boundaries and emits the nested `emView` + panel tree in C++-faithful context-cascade shape: each sub-view appears as a child `emContext` of its parent view's context, carrying its own View rec, its own root panel rec, and its own panel subtree.
- Per-panel `LastPaintFrame` comparison in the dump reads the *owning* view's `current_frame`, not the outer view's — so paint-skip analysis is correct per view.
- Control-channel commands (`visit`, `visit_fullsized`, `set_focus`, `seek_to`, `get_state`) address panels by `{ view?: String, identity: String }` using emCore-native `emPanel::GetIdentity` strings. The `/`-separated `panel_path` field is removed.
- Cross-view navigation is explicit and observable: the agent issues one `visit` per view, with `wait_idle` between them. The dispatcher does not cascade.
- `get_state` returns `{ focused_view, focused_identity, view_rect, loading }` with round-trip symmetry — the pair can be pasted back into `visit`.
- F010's four runtime questions become mechanically answerable from a single `dump` taken after the two-step navigation: paint state of every `emDirPanel` child in the cosmos sub-view's panel tree is visible in the emitted rec.

## Non-goals

- Multi-window addressing. The `view` selector's namespace is restricted to sub-views inside the home window's outer view. Dialogs, popups, and additional toplevel windows remain unaddressable by this channel. Extending to cross-window requires an `emContext` child-context enumeration discipline across windows plus a protocol-level `window` selector — deferred until a concrete bug needs it.
- Nested sub-views (a sub-view inside another sub-view). The `view` selector is a single string today; two-deep nesting would need a list-typed selector. The **dump walker handles arbitrary nesting depth** (by construction of the recursive pre-pass); only the addressing protocol is one-level-only. If a two-deep case appears, the protocol extension is mechanical (`view: Vec<String>`).
- Back-pointer from `emContext` to its owning `emView`. The ownership model (`emView` owned directly by `Window` or `emSubViewPanel`, not `Rc`) does not admit a cheap back-pointer; discovery runs panel-side instead. (See Design §(A) below.)
- Panel-path escape syntax. Using identity strings sidesteps this entirely — empty names, colons, and backslashes are handled by the existing `EncodeIdentity`/`DecodeIdentity` machinery in `emPanelTree.rs:65-142`.
- Porting `emTreeDumpFilePanel` / the in-app renderer for saved dumps. Schema stays C++-faithful per the original spec so this remains a future mechanical port.
- Backward compatibility on the `panel_path` wire field. The previous protocol shipped four days ago; only in-tree tests and one handoff doc reference it.

---

## Design

### (A) Dump: context-cascade across sub-views

The walker keeps the C++ cascade shape: `dump_from_root_context` → `dump_context(root_context)` → iterates `root_context.children` → for each child, emits the applicable rec kind (context / view / model) with field-accumulation order matching C++.

Two changes from today's walker:

**1. `dump_context` iterates `ctx.children.borrow()` instead of emitting an empty child list.** The stale comment at `emTreeDump.rs:601` is removed. Dead weak references are skipped. Live children are upgraded to `Rc<emContext>` and dispatched:
- If the context corresponds to a known `emView`: emit the view branch (existing `dump_view` path).
- Else: emit a plain `dump_context` rec (plain emContext, no view fields).

"Corresponds to a known `emView`" is resolved via a pre-built map — see (2).

**2. A pre-pass builds `ctx_ptr → (&emView, &PanelTree)`.** `dump_from_root_context` performs one panel-tree traversal before the cascade runs:

```
fn collect_views(
    view: &emView,
    tree: &PanelTree,
    out: &mut HashMap<*const emContext, (&emView, &PanelTree)>,
) {
    out.insert(Rc::as_ptr(&view.context), (view, tree));
    for panel_id in tree.all_panels() {
        if let Some(svp) = tree.behavior_as::<emSubViewPanel>(panel_id) {
            collect_views(&svp.sub_view, svp.sub_tree(), out);
        }
    }
}
```

Keys are raw pointer addresses from `Rc::as_ptr` — pointer comparison only (no unsafe deref). Map lifetime ties to the root walker frame. `ctx.children` iteration compares by `Rc::as_ptr` on each upgraded child against the map.

**3. Per-view `current_frame` threading.** `dump_panel`'s `current_frame: u64` parameter is set from the view whose panel tree is currently being walked. When the cascade dispatches a child context to `dump_view(child_view)`, the walker switches to `child_view.current_frame` for all panels in `child_view`'s tree. Cross-view comparison is never performed — each panel's `LastPaintFrame: N (current: M)` pair references the same counter.

**4. Sub-view branching in the output.** The `emSubViewPanel` rec in the parent view's panel tree remains a leaf (no sub-tree children inlined under it). The sub-view's View rec + panel tree emerges as a sibling branch of the parent view, under the parent's context, via the `ctx.children` cascade. This matches C++'s `emTreeDumpFromObject` output shape when sibling contexts contain `emView`s.

**Borrow chain.** The walker uses the existing disjoint-borrow accessor on `emSubViewPanel` (method at `emSubViewPanel.rs:142`) when it needs both `sub_view` and `sub_tree` borrowed simultaneously. The pre-pass holds shared borrows only (`&emView`, `&PanelTree`); the cascade re-borrows per-view as it descends, so there is no long-lived mutable borrow of any behavior.

### (B) Addressing: `{ view?, identity }`

**Wire-format change.** All commands currently carrying `panel_path: String` — `visit`, `visit_fullsized`, `set_focus`, `seek_to` — swap that field for two:

```
{
  view:     String (optional, default = ""),
  identity: String (required)
}
```

- `view` is the **outer-view identity** of the `emSubViewPanel` that contains the target view. `""` (or field omitted) means the home window's outer view. Examples from the live tree: `"root:content view"`, `"root:control view"`. **Single-level only:** `view` addresses one `emSubViewPanel` within the home window's outer view. Nested sub-views (an `emSubViewPanel` inside another sub-view) cannot be expressed with a single-string selector and are out of scope for this spec — a future extension would promote `view` to a list of selectors, one per boundary crossed.
- `identity` is an emCore-native identity string within that view, as produced by `emPanel::GetIdentity`. Examples: `""` (the view's root panel), `":"` (root's empty-named child — e.g. cosmos), `"::home"` (cosmos's child named `"home"`).

**Resolution.** New private helper in `emCtrlSocket`:

```
fn resolve_target(
    app: &mut App,
    view_sel: &str,
    identity: &str,
) -> Result<(&mut emView, &mut PanelTree, PanelId), String>
```

Algorithm:
1. Resolve `view_sel` against the home window's outer view + tree using `resolve_identity(outer_tree, outer_root, view_sel)`. If `view_sel == ""`, target is the outer view itself.
2. If the resolved panel is not the outer root, require it to be an `emSubViewPanel`; otherwise return `view selector does not refer to a sub-view panel: <view_sel>`.
3. With the selected view + its `PanelTree`, call `resolve_identity(tree, root, identity)` → `PanelId`.
4. Return `(&mut view, &mut tree, panel_id)`.

`resolve_identity` is the emCore-native version of today's `resolve_panel_path`. emCore's `GetIdentity(root)` includes the root's name as the first segment (verified: `emPanelTree.rs:999-1013`), so the decoder must consume `names[0]` as the expected root-name and descend from `names[1..]`. An empty identity string means "the root itself":

```
fn resolve_identity(tree: &PanelTree, root: PanelId, identity: &str) -> Result<PanelId, String> {
    let names = DecodeIdentity(identity);
    if names.is_empty() {
        return Ok(root);  // "" addresses the root itself
    }
    if names[0] != tree.name(root) {
        return Err(format!(
            "identity root mismatch: {:?} does not match root panel name {:?}",
            names[0], tree.name(root)
        ));
    }
    let mut cur = root;
    for (i, name) in names[1..].iter().enumerate() {
        let depth = i + 1;
        let matched: Vec<_> = tree.children(cur)
            .filter(|&c| tree.name(c) == name)
            .collect();
        match matched.len() {
            0 => return Err(format!(
                "no such panel: {} (segment {} = {:?} not found under {:?})",
                identity, depth, name, tree.name(cur)
            )),
            1 => cur = matched[0],
            _ => return Err(format!(
                "ambiguous identity: {} (segment {} = {:?} matches {} siblings)",
                identity, depth, name, matched.len()
            )),
        }
    }
    Ok(cur)
}
```

**Round-trip correctness.** For any live `PanelId p`, `resolve_identity(tree, root, &GetIdentity(tree, p)) == Ok(p)`. Tested.

**Ambiguity.** Unlike the old `/`-path which was vulnerable to the "two empty-named siblings" case as a silent path collision, `resolve_identity` emits an explicit `ambiguous identity` error. Today, no production panel tree in the Rust port produces ambiguous siblings; the error is a safety net for future code.

**`resolve_panel_path` removal.** The old function is deleted. Its tests (`emCtrlSocket.rs:1008+`) are replaced by `resolve_identity` tests. Test-only JSON fixtures using `panel_path` (e.g. `emCtrlSocket.rs:753`) are rewritten.

### (C) Cross-view navigation

Each `visit` / `visit_fullsized` / `set_focus` / `seek_to` operates on exactly one view. The dispatcher resolves the target via `resolve_target`, extracts `(view, tree, panel_id)`, and calls the view's own `VisitPanel` / `VisitFullsized` / `set_focus` / (seek). No cascading.

**Two-call navigation pattern.** An agent zooming into cosmos-home writes:

```
→ {"cmd":"visit","view":"","identity":"root:content view"}
← {"ok":true}
→ {"cmd":"wait_idle","timeout_ms":60000}
← {"ok":true,"idle_frame":...}
→ {"cmd":"visit","view":"root:content view","identity":"::home"}
← {"ok":true}
→ {"cmd":"wait_idle","timeout_ms":60000}
← {"ok":true,"idle_frame":...}
→ {"cmd":"dump"}
← {"ok":true,"path":"/tmp/debug.emTreeDump"}
```

Each step's side effects are independently observable via `dump` or `get_state`. This is the design's primary evidence-gathering affordance for F010-class bugs: the agent inspects state between view transitions.

**`wait_idle` scope.** Unchanged. `EngineScheduler::is_idle()` is process-global — it checks the shared scheduler, which holds engines from all views (sub-view `emView::RegisterEngines` registers on the same scheduler with a distinct scope, `emSubViewPanel.rs:89`). Waiting until idle waits for all views' animators and loading engines, which is what the agent wants.

### (D) `get_state` fields

Reply shape:

```json
{
  "ok": true,
  "focused_view": "root:content view",
  "focused_identity": "::home",
  "view_rect": [x, y, w, h],
  "loading": [{"view": "...", "identity": "...", "pct": 42}, ...]
}
```

- `focused_view` + `focused_identity`: identity pair for the currently-focused panel. Algorithm: (a) find the focused panel and its owning view by iterating the pre-pass `ctx_ptr → (view, tree)` map; (b) if it's the outer view, `focused_view = ""` and `focused_identity = GetIdentity(outer_tree, focused)`; (c) else find the outer-view `emSubViewPanel` whose `sub_view == owning_view`, and set `focused_view = GetIdentity(outer_tree, containing_svp)`, `focused_identity = GetIdentity(owning_tree, focused)`. Paste-back symmetric with `visit`. Example: outer-root focused → `focused_view=""`, `focused_identity="root"`; cosmos-home focused → `focused_view="root:content view"`, `focused_identity="::home"`.
- `view_rect`: the focused view's `CurrentX/Y/Width/Height`. (Not the outer's. The agent wanted the inner view's zoom state in Q3; this gives it directly.)
- `loading`: list of in-progress loads across all views. Each entry carries its own `view` selector plus the panel's `identity` within that view, so the agent can target any loading panel with `visit` / `seek_to`.

Today's `get_state` has `focused_path` + `view_rect`. Both are replaced. Test-only fixture at `emCtrlSocket.rs:940+` is rewritten.

---

## Protocol changes summary

| Command        | Field changes                                                                |
|----------------|------------------------------------------------------------------------------|
| `visit`        | `panel_path` → `view?` + `identity`                                           |
| `visit_fullsized` | same                                                                      |
| `set_focus`    | same                                                                         |
| `seek_to`      | same                                                                         |
| `get_state`    | `focused_path` → `focused_view` + `focused_identity`; `loading[]` entries gain `view` field |
| `dump`         | unchanged wire format; output content adds sub-view branches                 |
| `wait_idle`    | unchanged                                                                    |
| `quit`         | unchanged                                                                    |
| `input` / `input_batch` | unchanged                                                           |

---

## Annotation summary

- `resolve_identity` in `emCtrlSocket.rs` — no annotation required. It is the port of C++'s identity-based navigation, 1:1 with `emView::VisitByIdentity`'s path decoder semantics. If the signature deviates from any specific C++ function (e.g. returning `Result` instead of pointer), add a prose comment at the definition; no `DIVERGED:` tag (ownership-idiom adaptation, no structural change).
- `collect_views` pre-pass and the `ctx_ptr → (&emView, &PanelTree)` map — RUST_ONLY, category **language-forced utility**. Rationale: C++ uses `dynamic_cast<emView*>` to discover views during the context cascade; Rust composition has no inheritance cast, and the ownership model (§Non-goals) precludes a back-pointer. The pre-pass preserves C++ observable output via a language-idiom substitute.
- `dump_context` change to iterate `ctx.children` — no annotation required. It removes a stub that diverged from C++; the fix restores fidelity. Stale comment at `emTreeDump.rs:601` is deleted.

---

## Testing

### Unit tests

- **`resolve_identity` — root, single-segment, multi-segment, empty-named segments, round-trip.**
  - `resolve_identity(tree, root, "")` → root (empty identity addresses the root).
  - On an outer tree with root named `"root"`: `resolve_identity(tree, root, "root")` → root; `resolve_identity(tree, root, "root:content view")` → content SVP.
  - On an inner tree with sub-tree root named `""`: `resolve_identity(sub_tree, sub_root, "")` → sub_root; `resolve_identity(sub_tree, sub_root, ":")` → sub_root's empty-named child (cosmos); `resolve_identity(sub_tree, sub_root, "::home")` → home.
  - `resolve_identity(tree, root, &GetIdentity(tree, p))` → `Ok(p)` for every panel — parametric round-trip test.
  - Root-name mismatch (identity's first segment doesn't match root's name) → `identity root mismatch: ...`.
  - Missing segment → `no such panel: ... (segment N = "..." not found under "...")`.
  - Ambiguous → `ambiguous identity: ... (segment N = "..." matches K siblings)`.
- **`resolve_target` — view selector semantics.**
  - `view=""`, `identity="root"` → outer view root.
  - `view="root:content view"`, `identity=""` → content sub-view root.
  - `view="root:content view"`, `identity="::home"` on a seeded cosmos tree → the home panel inside the sub-view.
  - `view="root:slider"` (not a sub-view, but some panel): rejected with `view selector does not refer to a sub-view panel`.
- **`collect_views` pre-pass.**
  - Outer tree with two `emSubViewPanel`s → map contains three entries (outer + two inner).
  - Nested `emSubViewPanel` (sub-view inside sub-view) → map contains all four entries (recursion works).
- **Per-view `current_frame` threading.**
  - Dump of a scene with an outer view at `current_frame=10` and inner view at `current_frame=3`: panels of the outer view all show `current: 10`; inner view's panels all show `current: 3`.
- **`dump_context` iterates children.**
  - Synthetic root context with two child contexts (one view-bearing via the pre-pass map, one plain) → dump emits one view rec and one plain context rec as children of the root context rec.
- **JSON round-trip.**
  - Every command variant (`visit` / `visit_fullsized` / `set_focus` / `seek_to`) with both `view` present and absent.
  - `get_state` reply with sub-view focused and with outer-view focused.

### Integration tests (`#[ignore]`-gated per existing convention)

- **Cross-view dump after navigation.**
  - Launch binary with `EMCORE_DEBUG_CONTROL=1`.
  - `visit view="" identity="root:content view"`, `wait_idle`, `visit view="root:content view" identity="::home"`, `wait_idle`, `dump`.
  - Parse `/tmp/debug.emTreeDump`. Assert the emitted tree contains:
    - The outer view's context (root) with `content view` sub-tree's context as a child.
    - A View rec for the inner (content) view under that child context.
    - An `emDirPanel` rec somewhere in the inner view's panel tree, with `path` matching the user's home directory and `loading_done: true`.
- **`get_state` round-trip.**
  - After the above navigation, `get_state` returns `focused_view="root:content view"` + `focused_identity="::home"` (or equivalent for whatever focus the visit moved).
  - `visit { focused_view, focused_identity }` pasted back resolves without error.

### Golden tests

Not applicable — the dump walker and control channel do not participate in the pixel-output surface.

---

## Files created / modified

**Modified:**
- `crates/emcore/src/emTreeDump.rs`
  - Remove stale "emContext doesn't enumerate children" comment (line ~601).
  - `dump_context` iterates `ctx.children.borrow()`, upgrades each `Weak`, skips dead, emits via the pre-pass map.
  - `dump_from_root_context` runs `collect_views` pre-pass first.
  - Per-view `current_frame` threaded through `dump_panel`'s recursion.
  - New helper `collect_views` (RUST_ONLY, language-forced utility).
- `crates/emcore/src/emCtrlSocket.rs`
  - Replace `resolve_panel_path` with `resolve_identity` + `resolve_target`.
  - Rewrite `CtrlCmd` variants: `Visit`, `VisitFullsized`, `SetFocus`, `SeekTo` swap `panel_path` for `view?` + `identity`.
  - Rewrite `CtrlReply`: replace `focused_path` with `focused_view` + `focused_identity`; `loading[]` entries gain `view`.
  - Rewrite all affected handlers (`handle_visit`, `handle_visit_fullsized`, `handle_set_focus`, `handle_seek_to`, `handle_get_state`).
  - Rewrite JSON fixture tests; rewrite path-resolution tests under the `resolve_identity` / `resolve_target` module.
- `docs/debug/agent-control-channel.md` — update "Path syntax" section, every command's payload table row, and every recipe's example commands.
- `docs/debug/investigations/F010.md` — update the blocked-state preamble and next-steps to point at the new wire format.
- `docs/debug/ISSUES.json` — `F010.blocked_question` text that references `/cosmos/home` gets replaced with the identity form.

**Not modified (deliberate):**
- `crates/emcore/src/emContext.rs` — program code is adequate; children already tracked (line 45). No new fields, no back-pointer.
- `crates/emcore/src/emPanel.rs`, `emPanelTree.rs` — `EncodeIdentity`/`DecodeIdentity` already ported and correct.
- Any behavior-side `dump_state` implementations — per-view rendering is transparent to them.

---

## Implementation ordering (for writing-plans)

Suggested phasing; writing-plans will finalize:

1. **`resolve_identity` + tests.** Pure function; no program state. TDD-friendly.
2. **`collect_views` pre-pass + tests.** Synthetic tree with two nested `emSubViewPanel`s; map-contents assertions.
3. **`dump_context` iterates children; per-view `current_frame` threading.** Unit-tested against synthetic context trees.
4. **`resolve_target` + handler rewrites in `emCtrlSocket`.** Wire-format cutover happens here; old field names stop parsing.
5. **`get_state` field rewrite.**
6. **Doc/test-fixture updates** (agent-control-channel.md, F010 investigation, ISSUES.json).
7. **Integration test: cross-view dump + navigation.** Gated `#[ignore]` per existing convention.
8. **F010 re-engagement.** Canonical capture sequence with new wire format; advance or close the issue based on dump evidence.

---

## Risk register

- **Borrow discipline in `collect_views`.** Holding `&emView` and `&PanelTree` across recursion into an `emSubViewPanel`'s sub-view means the walker cannot simultaneously mutate the outer view. Pre-pass is read-only; cascade re-borrows per view. If this proves awkward, the map can key by panel-id (not pointer) and do late lookup — cheaper change than redesigning.
- **`Rc::as_ptr` stability.** Pointer addresses are stable while the `Rc` is alive. The map lives only within the walker frame and the outer `Rc<emContext>` is held by `emView` for the call duration. No lifetime risk.
- **Nested-nested sub-views.** `collect_views` is recursive; two-deep sub-views work by construction. No panel tree in today's codebase nests more than one deep; the integration test covers one level.
- **Empty-name ambiguity at scale.** If any future panel tree grows multiple empty-named siblings under one parent, `resolve_identity` errors explicitly rather than silently picking one. The error message tells the agent to address siblings by sibling index (future follow-up if ever needed).
