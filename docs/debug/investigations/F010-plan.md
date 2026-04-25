---
issue: F010
created: 2026-04-25
status: ready
phases: 5
order: X → Y → Z (trivial → small → large)
---

# F010 implementation plan — dir-panel render-chain cluster

Covers HYPOTHESES X, Y, Z. **Out of scope: F017 (slow loading).**

## Phase 0 — Documentation Discovery (findings)

Verified facts from Rust source. Cite these in every phase; do not re-derive.

### Painter API (already present)

| API | File:Line | Signature |
|---|---|---|
| `emPainter::Clear` | crates/emcore/src/emPainter.rs:5783 | `Clear(&mut self, color: emColor)` |
| `emPainter::PaintBorderImage` | crates/emcore/src/emPainter.rs:4407 | `(x,y,w,h: f64, l,t,r,b: f64, image: &emImage, src_l,src_t,src_r,src_b: i32, alpha: u8, canvas_color: emColor, which_sub_rects: u16)` |
| `emPainter::PaintTextBoxed` | crates/emcore/src/emPainter.rs (search) | `(x,y,w,h: f64, text: &str, max_char_height: f64, color, canvas_color, box_h_align, box_v_align, text_alignment, min_width_scale: f64, formatted: bool, rel_line_space: f64)` |

Existing call shapes to copy:
- `Clear`: crates/emstocks/src/emStocksFilePanel.rs:34 — `painter.Clear(self.bg_color);`
- `PaintTextBoxed`: crates/eaglemode/tests/golden/painter.rs:867
- `paint_image_full`: crates/eaglemode/tests/golden/painter.rs:621

### Theme data (already complete)

`emFileManThemeData` in **crates/emfileman/src/emFileManTheme.rs:63-209** has all OuterBorder/InnerBorder dimension and image fields:
- `OuterBorderL/T/R/B`, `OuterBorderImg`, `OuterBorderImgL/T/R/B`
- `FileInnerBorder*`, `DirInnerBorder*`, `AltInnerBorder*` analogues

Parser: field-name-driven via `from_rec`/`to_rec` (lines 379-701). Both directions already wire the OuterBorder/InnerBorder fields.

Theme assets: `res/emFileMan/themes/` is **byte-identical** to `~/Projects/eaglemode-0.96.4/res/emFileMan/themes/`. `CardOuterBorder.tga`, `CardInnerBorder.tga`, etc. all present.

Theme file declaration shape (CardBlue1.emFileManTheme:37-41):
```
OuterBorderImg = "CardOuterBorder.tga"
OuterBorderImgL = 250
OuterBorderImgT = 260
OuterBorderImgR = 390
OuterBorderImgB = 340
```

### Draw-op test infrastructure

- `DUMP_DRAW_OPS` env var → JSONL at `target/golden-divergence/{name}.rust_ops.jsonl`
- Logger: `install_direct_op_logger()` at crates/eaglemode/tests/golden/draw_op_dump.rs:23-41
- `DrawOp` enum already serializes `PaintBorderImage` and `PaintBorderImageColored`
- Template test: crates/eaglemode/tests/golden/eagle_logo.rs:24-59
- Diff tool: `python3 scripts/diff_draw_ops.py <name> --no-table`

### emDirEntry data accessors (Z prerequisites — all present)

crates/emfileman/src/emDirEntry.rs: `IsRegularFile`, `IsDirectory`, `IsSymbolicLink`, `GetStat`, `GetLStat`, `GetOwner`, `GetGroup`, `GetTargetPath`, `GetTargetPathErrNo`. **No data gaps.**

### Anti-patterns to guard against

- **Inventing painter methods** — only `Clear`, `PaintBorderImage`, `PaintBorderImageColored`, `PaintBorderImageSrcRect`, `PaintTextBoxed`, `paint_image_full`, `PaintText`, `PaintRect` are confirmed-present. Verify any other call against emPainter.rs before using.
- **`f64` in pixel-arithmetic paths** — CLAUDE.md forbids it for blend/coverage/interpolation. Coordinate/layout math is `f64`; that's allowed.
- **Re-shaping C++ logic** — Port Ideology: same operation order, same formulas. If the C++ has three layout modes with magic constants (0.087, 0.483, 7.6666, 1.03, 1.4, 0.025, 0.75), reproduce them with the constants intact and a `// C++: emDirEntryPanel.cpp:NNN` comment, not a "cleaner" rewrite.
- **Skipping the C++ symlink branch in PaintInfo** — Type field has a special case (Type label split, target path painted) at cpp:589-608. Port it; don't simplify to "just paint Type".
- **Unix vs Windows permission split** — C++ has two code paths (cpp:642-668). Rust port targets Linux; port the Unix branch. Mark with `// DIVERGED:` only if you intentionally drop the Windows branch (upstream-gap-forced does not apply — C++ ships both).

---

## Phase 1 — HYPOTHESIS X: port C++ `emDirPanel::Paint` switch verbatim

**Goal**: when dir panel is in `VFS_LOADED` / `VFS_NO_FILE_MODEL` state, the background renders `DirContentColor` (light grey). In other states, delegate to `emFilePanel::Paint` (matching C++).

**Replan note (2026-04-25)**: original directive ("unconditional Clear") was wrong — C++ gates the Clear on load state. Corrected directive ports the C++ switch verbatim. The "black during loading" symptom is not addressed by this phase; investigation moved to **F018** (emFilePanel::Paint loading-state divergence).

### What to implement (copy, don't transform)

C++ reference (read first):

```cpp
// emDirPanel.cpp:159-170
void emDirPanel::Paint(const emPainter & painter, emColor canvasColor) const
{
    switch (GetVirFileState()) {
    case VFS_LOADED:
    case VFS_NO_FILE_MODEL:
        painter.Clear(Config->GetTheme().DirContentColor.Get());
        break;
    default:
        emFilePanel::Paint(painter,canvasColor);
        break;
    }
}
```

In **crates/emfileman/src/emDirPanel.rs** at the top of the existing `Paint` method (around lines 477-491), port this switch verbatim. Rust has `GetVirFileState() -> VirtualFileState` (crates/emcore/src/emFilePanel.rs:100); use the `Loaded` and `NoFileModel` variants (confirm exact names by grepping `enum VirtualFileState`). Other variants delegate to `emFilePanel::Paint`.

The current Rust `Paint` body should run inside the `Loaded`/`NoFileModel` arm AFTER the `Clear` call — i.e. preserve all existing dir-panel painting; only add the leading Clear and the state gate. If the existing body needs to run in other states too, STOP and report — read C++ to confirm.

### Documentation references

- C++: `~/Projects/eaglemode-0.96.4/src/emFileMan/emDirPanel.cpp:159-170`
- Rust painter: `crates/emcore/src/emPainter.rs:5783`
- VirtualFileState enum: `crates/emcore/src/emFilePanel.rs` (search for `VirtualFileState`, `GetVirFileState`)
- Existing Clear call shape: `crates/emstocks/src/emStocksFilePanel.rs:34`
- Existing `emFilePanel::Paint` delegation pattern: search for `emFilePanel.*Paint` calls in `crates/emcore/src/emImageFileImageFilePanel.rs` or similar file panel subclasses

### Verification checklist

1. `cargo check` clean.
2. `cargo clippy -- -D warnings` clean.
3. **State-gated draw-op test**: assert that in `VirtualFileState::Loaded` (or `NoFileModel`), the FIRST emitted op is `Clear(DirContentColor)`. In a non-good state (e.g. `Waiting` / `TooCostly`), assert no `Clear(DirContentColor)` op is emitted. Use the eagle_logo.rs template (crates/eaglemode/tests/golden/eagle_logo.rs:24-59).
4. `cargo-nextest ntr` clean.
5. Grep guard: `grep -n "Clear" crates/emfileman/src/emDirPanel.rs` — exactly one Clear call, inside the `Loaded`/`NoFileModel` arm.

### Anti-pattern guards

- **Don't add an unconditional Clear.** C++ gates it on state.
- **Don't fix the "black during loading" symptom in this phase** — that's F018. If you find yourself wanting to also Clear in the default arm "to fix the dark loading", STOP and report.
- **Don't reorder C++ logic.** `Clear` first, then the existing dir-panel painting (LayoutChildren / entry painting / etc.) inside the same arm. The default arm is just a delegation to `emFilePanel::Paint`.

### Phase exit gate

All five verification items pass. Commit message: `fix(F010 X): port emDirPanel::Paint state-gated Clear from C++`.

---

## Phase 2 — HYPOTHESIS Y: paint outer/inner border image in `emDirEntryPanel::Paint`

**Goal**: dir entries render Card-Blue gradient outer border (and inner border for file/dir content area) instead of flat chrome.

### What to implement (copy, don't transform)

C++ reference: **`~/Projects/eaglemode-0.96.4/src/emFileMan/emDirEntryPanel.cpp:318-335`** — open this and identify exactly which `PaintBorderImage` calls are made, with which theme fields, in what order.

Translate each call into Rust using the `PaintBorderImage` signature at emPainter.rs:4407. The theme fields are already on `emFileManThemeData` (Phase 0) — no struct or parser changes needed.

Insert the call(s) in **crates/emfileman/src/emDirEntryPanel.rs** at the appropriate point in `Paint` (mirror the C++ ordering: outer border early, inner border around the content rect — read C++ to confirm).

Image loading: confirm whether the theme already loads `OuterBorderImg` into an `emImage` (search `emFileManTheme.rs` for image loading; if loaded eagerly the asset is already a struct field; if lazy, copy whatever pattern exists for other theme images — likely `FileInnerBorderImg` is loaded the same way).

### Documentation references

- C++: `~/Projects/eaglemode-0.96.4/src/emFileMan/emDirEntryPanel.cpp:318-335`
- Painter primitive: `crates/emcore/src/emPainter.rs:4407-4425`
- Theme fields: `crates/emfileman/src/emFileManTheme.rs:63-209` (search "OuterBorder", "FileInnerBorder", "DirInnerBorder")
- Theme file (verify field shape live): `res/emFileMan/themes/CardBlue1.emFileManTheme:37-41, 65-69, 82-86, 117-121`

### Verification checklist

1. `cargo check` clean.
2. `cargo clippy -- -D warnings` clean.
3. Draw-op test: assert `PaintBorderImage` is emitted at least once in `emDirEntryPanel::Paint`, with `image` matching the loaded `OuterBorderImg` (test by image dimensions or a recorded asset hash — see how `eagle_logo.rs` asserts).
4. `cargo-nextest ntr` clean.
5. **C++ ↔ Rust draw-op diff**: `python3 scripts/diff_draw_ops.py f010 --no-table` (or whichever golden test exercises the dir panel — find with `grep -rn "f010\|dir.*panel" tests/golden/`). Expect zero `PaintBorderImage` divergences for this entry's chrome.

### Anti-pattern guards

- Don't reload the .tga asset on each Paint call. Theme images are loaded once at theme-load time. Verify by reading how an existing border image is referenced (e.g. `theme_rec.FileInnerBorderImg` is likely already an `&emImage` reference, not a path).
- Don't change the call order vs C++. The border-image calls must stay in the C++-mandated sequence relative to other paint ops, or the draw-op diff will flag it.
- If `theme_rec.OuterBorderImg` resolves to an empty/None image (theme didn't declare one), match C++ behavior: skip the call. Read C++ to confirm — likely there's an `if (Img.IsValid())` or equivalent guard.

### Phase exit gate

All five verification items pass. Commit: `fix(F010 Y): paint outer/inner border images on dir entries`.

---

## Phase 3 — HYPOTHESIS Z: port `PaintInfo` body to `emDirEntryPanel::Paint` info area

**Goal**: info pane shows Type, Permissions, Owner, Group, Size, Time fields with labels (currently shows only Time).

**This is the heaviest phase. Recommend bumping reasoning to high or dispatching to a subagent with extended thinking.**

### What to implement (copy, don't transform)

Open **`~/Projects/eaglemode-0.96.4/src/emFileMan/emDirEntryPanel.cpp:484-725`** and port the entire `PaintInfo` body (or its inlined equivalent in the C++ Paint) into Rust at **crates/emfileman/src/emDirEntryPanel.rs:592-609**, replacing the current Time-only stub.

#### Required structural elements (all from C++)

1. **Three layout modes** (cpp:512-562) keyed on aspect ratio `t = infoH / infoW`:
   - **Tall** (t > 0.9, cpp:512-529): 6 rows stacked vertically, Time row doubled height.
   - **Medium** (0.04 < t ≤ 0.9, cpp:531-544): 2×2 grid (Type full, Perms+Owner split, Group+Size split, Time full).
   - **Wide** (t ≤ 0.04, cpp:546-562): 6 fields in one horizontal row.

2. **Magic constants — preserve exactly with comments**:
   - `0.087` — row/column spacing ratio
   - `0.483` — horizontal gap scale (medium mode)
   - `7.6666` — label height divisor (`lh = th / 7.6666`)
   - `1.4` — initial height scale (tall mode heuristic)
   - `1.03` — timestamp adjustment (medium mode)
   - `0.025` — wide-mode minimum width factor
   - `0.75`, `1/5` — magnitude suffix positioning (Size field)

3. **Label paint loop** (cpp:565-574): paint all 6 labels via `PaintTextBoxed` if `lh * GetViewedWidth() > 1.0`; then shift `by[i] += lh; bh[i] -= lh` so values use remaining height.

4. **Six field blocks** (line ranges from C++):
   - **Type**: cpp:578-634 — branches on regular/dir/fifo/blk/chr/sock + symlink (cpp:589-608 paints label half + target path half).
   - **Permissions**: cpp:642-668 — port the **Unix branch only** (cpp:650-668), three `PaintText` calls for owner/group/other rwx groups. Drop the Windows branch (cpp:642-648) — mark with `// DIVERGED: Linux-only port; Windows attribute branch (emDirEntryPanel.cpp:642-648) intentionally omitted` and category `upstream-scope-decision` (or whichever category Port Ideology specifies — re-read CLAUDE.md §"Forced divergence" before annotating).
   - **Owner**: cpp:670-676 — single `PaintTextBoxed` from `entry.GetOwner()`.
   - **Group**: cpp:678-684 — single `PaintTextBoxed` from `entry.GetGroup()`.
   - **Size**: cpp:689-709 — `emUInt64ToStr(st_size)` with thousands separator + magnitude suffix (k/M/G/T/P/E/Z/Y) via separate `PaintText` calls in a loop. The magnitude suffix is positioned at `(by[4] + bh[4]*0.75, bh[4]/5)`.
   - **Time**: cpp:714-721 — `FormatTime(st_mtime, ...)` two-arg form with bw/bh ratio for compact mode. (Current stub paints this field; replace with the C++ shape.)

5. **Pixel-arithmetic rule (CLAUDE.md)**: layout coordinates are `f64` (allowed). Do **not** introduce `f64` for any blend/coverage path — but this phase is layout-only, so this rule is mostly informational.

### Documentation references

- C++ body: `~/Projects/eaglemode-0.96.4/src/emFileMan/emDirEntryPanel.cpp:484-725`
- Helper: `emUInt64ToStr` (search C++ src for definition; confirm Rust equivalent — likely on a string utility module).
- Helper: `FormatTime` (already used in stub at emDirEntryPanel.rs:594).
- `PaintTextBoxed` template: `crates/eaglemode/tests/golden/painter.rs:867`
- `PaintText` template: search `crates/eaglemode/tests/golden/` for an existing call.
- Data accessors: `crates/emfileman/src/emDirEntry.rs` (`GetOwner`, `GetGroup`, `GetStat`, `IsSymbolicLink`, `GetTargetPath`, etc.)
- Layer rule: `CLAUDE.md` §"Pixel arithmetic" / §"Geometry"

### Verification checklist

1. `cargo check` clean.
2. `cargo clippy -- -D warnings` clean.
3. **All three layout modes draw-op tested**: three small tests with `(InfoX, InfoY, InfoW, InfoH)` chosen to land in tall, medium, and wide regimes. Each asserts the correct number of `PaintTextBoxed` + `PaintText` ops emitted (tall=12+ for labels+values, medium=12+, wide=12+).
4. **Field content draw-op test**: with a known `emDirEntry` (constructed in test), assert the painted text strings match the C++ output for at least Type, Owner, Group, Size, Time. (Permissions string is harder to assert literally — defer to the diff tool.)
5. `cargo-nextest ntr` clean.
6. **C++ ↔ Rust draw-op diff**: `python3 scripts/diff_draw_ops.py f010 --no-table` — expect zero `PaintTextBoxed`/`PaintText` divergences in the info-area sub-rect.
7. Annotation lint: `cargo xtask annotations` clean (validates the `DIVERGED:` tag on the Windows branch).

### Anti-pattern guards

- **Don't collapse the three layout modes into one**. The C++ has three distinct branches with different `th/spy/tw/spx` formulas; collapsing them changes the draw-op output and breaks the diff.
- **Don't substitute the magic constants with named cleaner values** unless you also rename them in C++. Per Port Ideology, structure is load-bearing. Acceptable: declare them as `const` with a comment citing the C++ line. Not acceptable: rounding `0.087` to `0.09` or `7.6666` to `7.667`.
- **Don't skip the symlink branch**. Type field has a special case at cpp:589-608 — port it.
- **Don't write the Windows permission branch**. Linux-only port. Mark the omission per Port Ideology.
- **Don't use `format!("{}", size)` for the Size field** — C++ uses a thousands-separator loop with a magnitude suffix. Port the loop.
- **Don't reorder the field paint sequence**. C++ paints labels first (loop), then values. Match exactly.

### Phase exit gate

All seven verification items pass. Commit: `fix(F010 Z): port PaintInfo six-field info pane with three layout modes`.

---

## Phase 4 — Final Verification

### Static checks

1. `cargo check` — clean.
2. `cargo clippy -- -D warnings` — clean.
3. `cargo fmt --check` — clean.
4. `cargo xtask annotations` — clean.
5. `cargo-nextest ntr` — full suite green.
6. `python3 scripts/divergence_report.py` — F010-related golden tests at zero or improved divergence vs prior baseline. Compare with `--diff` against pre-Phase-1 state.

### Manual GUI verification (single pass at end)

1. `cargo run --bin eaglemode` (apply CLAUDE.md timeout-on-launch rule).
2. Navigate cosmos → zoom into File System or `/` directory.
3. **X verified**: while loading, background is `DirContentColor` (light grey), not black.
4. **Y verified**: dir entries show Card-Blue gradient outer border + inner border around content area, matching screenshots/f010-cpp.png at the same zoom.
5. **Z verified**: each entry's left info pane shows Type, Permissions (rwxr-x-r-x style), Owner, Group, Size (with thousands separator + suffix), Time — labels and values both visible. Compare against C++ reference screenshot.
6. Re-test at multiple zoom levels to exercise all three Z layout modes (deep zoom = tall, medium = 2×2, full-cosmos = wide).

### Anti-pattern grep guards (final sweep)

- `grep -rn "f64" crates/emfileman/src/emDirEntryPanel.rs | grep -iE "blend|coverage|interp"` — empty.
- `grep -n "OuterBorderImg" crates/emfileman/src/emDirEntryPanel.rs` — at least one painter call.
- `grep -n "Clear" crates/emfileman/src/emDirPanel.rs` — exactly one Clear at top of Paint.

### ISSUES.json closure

After all checks pass:

1. Update `docs/debug/ISSUES.json` F010: `status` → `needs-manual-verification`, `fixed_in_commit` → final commit SHA, `fixed_date` → ISO date, `fix_note` → "Cluster X+Y+Z landed; F017 still open separately."
2. Add an F010 root-cause file at `docs/debug/investigations/F010-root-cause.md` summarizing the three divergences and their fixes.
3. After human GUI confirmation, flip status to `closed`.

### Phase exit gate

All static checks + manual verification + ISSUES.json updates committed. F017 (slow-loading) remains the only F010-derived issue still open.

---

## Plan-level reminders

- **One commit per phase**, not one combined commit. Easier to bisect if a regression appears.
- **Re-read CLAUDE.md §Port Ideology and §Port Fidelity before Phase 3** — Z is the phase most likely to drift if the executor loses fidelity discipline.
- **F017 is out of scope.** If during Phase 3 the executor notices the loading-progress overlay, do not fix it — file observations on F017 instead.
- **Phase 0 findings are authoritative.** If a later phase contradicts them (e.g. discovers a missing API), update Phase 0 with the correction before proceeding.
