# C-24 / I-11 Item Panel Routing Design

Date: 2026-05-01

## Problem

Two emTestPanel spec-compliance items are blocked by a structural gap in the Rust
emListBox: the `PanelBehavior` stored in the panel tree for each item child has no
connection to the `emListBox` that owns the item slot.

- **C-24**: `CustomItemBehavior::Input` must call `ProcessItemInput`, which dispatches
  selection/trigger to the listbox. No routing path exists from a child `PanelBehavior`
  to the parent listbox.
- **I-11**: `CustomItemBehavior::on_item_text_changed` must update `self.text` when the
  listbox changes an item's display text. `emListBox::SetItemText` notifies only the
  `ItemPanelInterface` in the item slot, never the `PanelBehavior` in the panel tree.

Both gaps apply equally to `DefaultItemPanelBehavior`. The fix upscopes to cover all
item behaviors.

## Approach

Approach A: `PanelBehavior` trait extension + parent-ID back-reference via a new
`PanelCtx::with_parent_behavior` primitive. No `Rc<RefCell<>>`.

Two routing directions:

- **C-24 (child → parent):** item behavior calls `with_parent_behavior`, which takes
  the parent behavior out of the tree and calls a new `dispatch_item_input` method on
  it with full ctx access. `ListBoxPanel` overrides `dispatch_item_input` to call
  `emListBox::process_item_input`, which calls `select_by_input` and
  `drain_pending_fires` in-place. Callbacks fire in the same frame — no one-tick drift.
- **I-11 (parent → child):** `emListBox` stores the `PanelId` of each child created by
  the behavior factory. `SetItemText` calls `on_item_text_changed` on the child's
  `PanelBehavior` via `ctx.tree.with_behavior_dyn`. `PanelBehavior` gains this as a
  default no-op; item behaviors override it.

## Changes

### `emEngineCtx.rs` — `PanelCtx`

New method:

```rust
pub fn with_parent_behavior<R>(
    &mut self,
    f: impl FnOnce(&mut dyn PanelBehavior, &mut PanelCtx) -> R,
) -> Option<R>
```

Takes the parent behavior out of the tree (breaking the borrow on `ctx.tree`), calls
`f(parent, self)` with full `ctx` access, then puts the parent back. Returns `None` if
no parent or parent has no behavior.

### `emPanelTree.rs` — `PanelTree`

New method (untyped variant of `with_behavior_as`, no downcast):

```rust
pub fn with_behavior_dyn<R>(
    &mut self,
    id: PanelId,
    f: impl FnOnce(&mut dyn PanelBehavior) -> R,
) -> Option<R>
```

### `emPanel.rs` — `PanelBehavior` trait

Two new default-no-op methods:

```rust
/// Child → parent. Called by item panel behaviors to dispatch selection/trigger.
/// Overridden by container behaviors that own item children (e.g. ListBoxPanel).
fn dispatch_item_input(
    &mut self,
    _item_index: usize,
    _event: &emInputEvent,
    _state: &PanelState,
    _ctx: &mut PanelCtx,
) -> ItemInputResult { ItemInputResult::default() }

/// Parent → child. Called by the owning listbox when an item's display text changes.
/// Overridden by item panel behaviors.
fn on_item_text_changed(&mut self, _text: &str) {}
```

### `emListBox.rs` — `Item` struct

New field:

```rust
child_panel_id: Option<PanelId>,  // set when behavior factory creates the child
```

### `emListBox.rs` — `create_item_children`

After calling the behavior factory and creating the child panel, stores the returned
`PanelId`:

```rust
self.items[i].child_panel_id = Some(child_id);
```

### `emEngineCtx.rs` — `PanelCtx::request_focus` (new method)

```rust
pub fn request_focus(&mut self)
```

Requests focus for the current panel. Forwards to `emView::focus_panel` via
`with_sched_reach`. Matches C++ `emPanel::Focus()`. Used by item behaviors after
`dispatch_item_input` signals that focus should be taken.

### `emListBox.rs` — `process_item_input` (new public method)

Extracted from `input_impl`'s `MouseLeft` / `Space` / `Enter` arms:

```rust
pub struct ItemInputResult {
    pub consumed: bool,
    pub focus_self: bool,  // true on MouseLeft — matches C++ panel->Focus()
}

pub fn process_item_input(
    &mut self,
    item_index: usize,
    event: &emInputEvent,
    state: &PanelState,
    ctx: &mut PanelCtx,
) -> ItemInputResult
```

Calls `self.select_by_input(...)` and `self.drain_pending_fires(ctx)`. Sets
`focus_self = true` for `MouseLeft` (matching C++ `panel->Focus()` call). The three
arms in `input_impl` become calls to this method (focus on the listbox's own panel for
the direct-input path, which was not previously implemented either).

### `emListBox.rs` — `SetItemText`

Signature change:

```rust
pub fn SetItemText(&mut self, index: usize, text: String, ctx: Option<&mut PanelCtx>)
```

After the existing `iface.item_text_changed(&text)` call, if `child_panel_id` is
`Some(id)` and `ctx` is `Some(ctx)`:

```rust
ctx.tree.with_behavior_dyn(id, |child| child.on_item_text_changed(&text));
```

No explicit repaint call is needed; painting is continuous and the next frame picks up
the updated text.

Callers that have ctx pass `Some(ctx)`; callers at construction time (before
`AutoExpand`) pass `None`. No notification fires when the child panel does not yet exist.

### `emTestPanel.rs` / `emListBox.rs` — `ListBoxPanel`

Implements `dispatch_item_input`:

```rust
fn dispatch_item_input(
    &mut self,
    item_index: usize,
    event: &emInputEvent,
    state: &PanelState,
    ctx: &mut PanelCtx,
) -> ItemInputResult {
    self.widget.process_item_input(item_index, event, state, ctx)
}
```

No changes to `Input`, `AutoExpand`, `LayoutChildren`, or `notice`.

### `emListBox.rs` — `DefaultItemPanelBehavior`

Add `item_index: usize` field (not currently present). Pass it from `create_item_children` at the factory call site alongside the existing `i` loop variable.

Implements `Input`:

```rust
fn Input(&mut self, event: &emInputEvent, state: &PanelState, _is: &emInputState, ctx: &mut PanelCtx) -> bool {
    let idx = self.item_index;
    let result = ctx.with_parent_behavior(|parent, ctx| {
        parent.dispatch_item_input(idx, event, state, ctx)
    }).unwrap_or_default();
    if result.focus_self {
        ctx.request_focus();
    }
    result.consumed
}
```

Implements `on_item_text_changed`:

```rust
fn on_item_text_changed(&mut self, text: &str) {
    self.text = text.to_string();
}
```

### `emTestPanel.rs` — `CustomItemBehavior`

Gains `item_index: usize` field. Factory call site passes the index.

Same `Input` and `on_item_text_changed` implementations as `DefaultItemPanelBehavior`.

## Testing

Unit tests in `emListBox.rs` `#[cfg(test)]`:

**C-24:**
- Expand a listbox with `item_behavior_factory`. Synthesize `MouseLeft` press on item 2,
  call the item behavior's `Input`. Assert item 2 is selected and `on_selection` fired.
- Synthesize `Space` (select) and `Enter` (trigger → `on_trigger`).
- Shift-click item 4 after item 2 is selected; assert range selected.

**I-11:**
- Expand a listbox with `item_behavior_factory`. Call `SetItemText(0, "new", Some(&mut ctx))`.
  Assert the item behavior's `self.text` is `"new"`.
- Call `SetItemText` before `AutoExpand` (ctx = None). Assert no panic and no child
  notification.

No golden test changes required. The two skipped tests (`polydrawpanel_default_render`,
`testpanel_expanded`) are unrelated and remain skipped.
