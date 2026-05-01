# emTestPanel Open Items

This document covers work left incomplete after the emTestPanel spec-compliance
session (2026-05-01). Two items could not be implemented; two golden tests
remain in the nextest skip list with known pixel divergences.

All measurements are at channel-tolerance=0. Tests are skipped via
`.config/nextest.toml`.

---

## Deferrals

### C-24 — CustomItemBehavior::Input (ProcessItemInput)

**C++ source:** `emTestPanel::CustomItemPanel::Input` (emTestPanel.cpp:930–934)

```cpp
void emTestPanel::CustomItemPanel::Input(
    emInputEvent & event, const emInputState & state, double mx, double my
)
{
    ProcessItemInput(this, event, state);
    emLinearGroup::Input(event, state, mx, my);
}
```

`ProcessItemInput` is a protected method of `emListBox::ItemPanelInterface`
(emListBox.h:282–288). It dispatches selection, deselection, and trigger
actions based on mouse and keyboard events. The C++ `CustomItemPanel` reaches
it because it inherits from `ItemPanelInterface` via the listbox mechanism.

**Rust state:** `ItemPanelInterface` exists as a trait in `emListBox.rs` but
does not include a `process_item_input` method. The Rust `CustomItemBehavior`
implements `PanelBehavior` and is stored in the panel tree. It is not connected
to `ItemPanelInterface`. There is no channel through which a `PanelBehavior`
can invoke emListBox's selection-dispatch logic.

**What is needed:** a method on `ItemPanelInterface` (or a separate mechanism)
that `CustomItemBehavior::Input` can call to dispatch selection events, wired
through to `emListBox`'s internal `SelectByInput` / `KeyWalk` logic.

---

### I-11 — CustomItemBehavior::item_text_changed

**C++ source:** `emTestPanel::CustomItemPanel::ItemTextChanged`
(emTestPanel.cpp:959–962)

```cpp
void emTestPanel::CustomItemPanel::ItemTextChanged()
{
    SetCaption(GetItemText());
}
```

**Rust state:** `ItemPanelInterface::item_text_changed(&mut self, text: &str)`
exists in `emListBox.rs:50` and is called by emListBox at line 747. However,
it is called on the `ItemPanelInterface` trait object stored inside the
listbox's item slot — not on the `PanelBehavior` stored in the panel tree for
the same panel. `CustomItemBehavior` implements `PanelBehavior`. There is no
routing from the listbox's `item_text_changed` call to the behavior held by
the panel tree.

**What is needed:** a routing path from `emListBox`'s item-update cycle to the
`PanelBehavior` of the panel the factory produced for that item slot.

---

## Golden Divergences

Both tests are skipped. To run them individually:

```
cargo test -p eaglemode --test golden test_panel::<name> -- --test-threads=1
```

Debug images (actual / expected / diff PPM) are written to
`crates/eaglemode/target/golden-debug/` when `DUMP_GOLDEN=1` is set.

---

### polydrawpanel_default_render

**Viewport:** 800×600 px. **Total pixels:** 480,000.

Measured after the `PolyDrawPanel::IsOpaque` fix (2026-05-01), which eliminated
the background mismatch (diff 128+) by changing `IsOpaque` from hardcoded
`true` to `self.group.border.IsOpaque(&self.group.look)`.

| diff range | pixels | % of total |
|------------|--------|------------|
| 2–3        | 12,695 | 2.64%      |
| 4–7        |    901 | 0.19%      |
| 8–15       |    290 | 0.06%      |
| 16–31      |    341 | 0.07%      |
| 32–63      |    560 | 0.12%      |
| 64–127     |     86 | 0.02%      |
| **total**  | **14,873** | **3.10%** |

max_diff = 122.

First reported failure: pixel (400,41) — actual `rgb(80,80,160)`,
expected `rgb(80,83,154)`.

---

### testpanel_expanded

**Viewport:** 1000×1000 px. **Total pixels:** 1,000,000.

| diff range | pixels | % of total |
|------------|--------|------------|
| 2–3        |    619 | 0.06%      |
| 4–7        |    369 | 0.04%      |
| 8–15       |    669 | 0.07%      |
| 16–31      |  1,164 | 0.12%      |
| 32–63      |  5,301 | 0.53%      |
| 64–127     | 34,981 | 3.50%      |
| 128–191    | 11,635 | 1.16%      |
| 192–255    |  4,324 | 0.43%      |
| **total**  | **59,062** | **5.91%** |

max_diff = 255.

First reported failure: pixel (700,50) — actual `rgb(0,28,56)`,
expected `rgb(136,136,136)`.

- `rgb(0,28,56)` is `DEFAULT_BG` (`emColor::rgba(0x00, 0x1C, 0x38, 0xFF)`,
  defined in `crates/emtest/src/emTestPanel.rs`).
- `rgb(136,136,136)` is the unfocused `fg` value computed in `TestPanel::Paint`
  (`emColor::rgba(136, 136, 136, 255)` when neither focused nor in focused
  path).
- Pixel (700,50) is the top-left pixel of child panel `tp1`, whose layout
  position is `(x=0.70, y=0.05)` in root-panel coordinates.
