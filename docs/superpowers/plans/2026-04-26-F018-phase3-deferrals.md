# F018 Phase 3 Deferrals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the three deferrals from F018 Phase 1+2 remediation: cascade `canvas_color` through widget paint helpers (Phase A), retire the painter canvas-color carrier (Phase B), and pair `WakeUpUpdateEngine` with `SVPChoiceByOpacityInvalid` writes (Phase C).

**Architecture:** Each widget `Paint` method that today reads `painter.GetCanvasColor()` after calling `border.paint_border(...)` is migrated to (a) take its outer `canvas_color: emColor` from the `PanelBehavior::Paint` trait caller, and (b) replace the painter read with `self.border.content_canvas_color(canvas_color, &self.look, enabled)` — the existing painter-free helper at `emBorder.rs:1488`. Layer 2 migrates the four `emBorder` helpers themselves (`paint_border`, `paint_label`, `paint_label_colored`, `paint_label_impl`) to take a `canvas_color: emColor` parameter directly. Once no production reader of `painter.GetCanvasColor()` remains, Phase B deletes the carrier (field, methods, `DrawOp::SetCanvasColor` variant). Phase C adds `ctx: &mut SchedCtx<'_>` to `emView::InvalidatePainting` / `invalidate_painting_rect` and pairs each `SVPChoiceByOpacityInvalid = true` write with `self.WakeUpUpdateEngine(ctx)` to mirror C++ `emPanel::InvalidatePainting`.

**Tech Stack:** Rust 1.x, emcore observational port of Eagle Mode 0.96.4 C++. Build: `cargo check --workspace --tests`, `cargo clippy -- -D warnings`, `cargo-nextest ntr`. Golden-test verification: `scripts/verify_golden.sh --report`.

**Spec:** `docs/superpowers/specs/2026-04-26-F018-phase3-deferrals-design.md` (commit `7600a9ed`).

---

## Conventions used in this plan

- **Worktree:** `/home/alex/Projects/eaglemode-rs/.claude/worktrees/f018-remediation`. All commands run from there.
- **Verification command (per commit):** `cargo check --workspace --tests && cargo clippy --workspace --tests -- -D warnings`. The pre-commit hook also runs `cargo fmt` and `cargo-nextest ntr`.
- **Commit footer:** `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`
- **Pre-commit invariant for Layer 1 tasks:** before each commit, run `grep -rn '\.<helper>\b' crates/ --include="*.rs"` to confirm exactly one production caller. If more, **stop** and re-design that helper as Layer 2.

---

## Task 0: Baseline check

**Files:** none (verification only)

- [ ] **Step 1: Verify clean tree at spec commit**

```bash
git rev-parse HEAD
# Expected to be on or descended from 7600a9ed (spec commit)
git status --short
# 10 unstaged files from prior session work are acceptable (unrelated to F018)
# but must NOT touch any file this plan modifies. List of files this plan modifies:
#   crates/emcore/src/emButton.rs
#   crates/emcore/src/emCheckBox.rs
#   crates/emcore/src/emCheckButton.rs
#   crates/emcore/src/emColorField.rs
#   crates/emcore/src/emListBox.rs
#   crates/emcore/src/emRadioBox.rs
#   crates/emcore/src/emRadioButton.rs
#   crates/emcore/src/emScalarField.rs
#   crates/emcore/src/emSplitter.rs
#   crates/emcore/src/emTextField.rs
#   crates/emcore/src/emBorder.rs
#   crates/emcore/src/emPainter.rs
#   crates/emcore/src/emView.rs
#   crates/emcore/src/render/software_compositor.rs
#   crates/emcore/src/render/wgpu_compositor.rs
#   plus DrawOp definition file (TBD — find via grep)
```

- [ ] **Step 2: Verify baseline build is green**

Run: `cargo check --workspace --tests`
Expected: `Finished` with no errors.

Run: `cargo clippy --workspace --tests -- -D warnings`
Expected: `Finished` with no warnings.

- [ ] **Step 3: Snapshot golden-test status**

Run: `scripts/verify_golden.sh --report`
Expected: passes match baseline at `988e799d`. Save the output to compare against at end of Phase A and Phase B.

---

# PHASE A — Cascade `canvas_color` through helpers

## Layer 1 — Widget `Paint` methods (Tasks 1–10)

Each task migrates one widget. Pattern, every task:

1. Add `canvas_color: emColor` parameter to the widget's `Paint` method, immediately after `painter`.
2. Replace `let canvas_color = painter.GetCanvasColor();` (which currently reads the post-`paint_border` value) with `let canvas_color = self.border.content_canvas_color(canvas_color, &self.look, enabled);`. The right-hand side's `canvas_color` is the new parameter (outer panel canvas); the `let` shadows it with the post-border content canvas.
3. Update the trait-impl caller (or non-trait caller) inside the same file to pass `canvas_color` through. Most are inside `impl PanelBehavior for ...` and already have `canvas_color: emColor` available from commit `988e799d`.
4. Verify, commit.

If the widget's `Paint` method does not have `enabled` in scope when reaching the migration site, use the value of `enabled` that's already passed to `paint_border` on the line directly above. If neither is available, derive: pass `true` only when `paint_border` is called with `enabled=true` literal; otherwise stop and investigate.

If the trait-impl caller isn't in the same file, locate it with `grep -rn 'self\.<field>\.Paint\b\|<TypeName>::Paint\b' crates/`.

---

### Task 1: emButton — migrate `Paint` method

**Files:**
- Modify: `crates/emcore/src/emButton.rs:177-191` (signature + body)
- Modify: `crates/emcore/src/emButton.rs` (trait-impl caller of `self.button.Paint(...)` or similar — find via grep)

- [ ] **Step 1: Confirm caller count**

Run: `grep -rn 'emButton::Paint\b\|\.Paint(' crates/emcore/src/emButton.rs | grep -v "fn Paint"`
Expected: trait-impl callers identified. Also check external callers:
Run: `grep -rn '\.button\.Paint\b' crates/ --include="*.rs"`

- [ ] **Step 2: Update `Paint` signature at `emButton.rs:177`**

Current signature:
```rust
pub fn Paint(
    &mut self,
    painter: &mut emPainter,
    w: f64,
    h: f64,
    enabled: bool,
    pixel_scale: f64,
) {
```

New signature:
```rust
pub fn Paint(
    &mut self,
    painter: &mut emPainter,
    canvas_color: emColor,
    w: f64,
    h: f64,
    enabled: bool,
    pixel_scale: f64,
) {
```

- [ ] **Step 3: Replace the painter read at `emButton.rs:191`**

Current:
```rust
self.border
    .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
let canvas_color = painter.GetCanvasColor();
```

New:
```rust
self.border
    .paint_border(painter, w, h, &self.look, false, true, pixel_scale, canvas_color);
let canvas_color = self
    .border
    .content_canvas_color(canvas_color, &self.look, enabled);
```

> **Note:** `paint_border` does not yet take `canvas_color`. This call site adds the new argument *prospectively* — it will fail to compile until Layer 2 Task 11 lands. To keep the tree green per task, **temporarily** keep the old `paint_border` call shape (no `canvas_color` arg) for Layer 1 tasks. Use this shape instead:
>
> ```rust
> self.border
>     .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
> let canvas_color = self
>     .border
>     .content_canvas_color(canvas_color, &self.look, enabled);
> ```
>
> This builds because `content_canvas_color` is painter-free. The `paint_border` call still mutates the painter's tracked canvas color internally, but we no longer read it. Layer 2 will add the `canvas_color` argument to `paint_border` once we migrate that helper.

- [ ] **Step 4: Update trait-impl caller**

Find the call site (likely in `impl PanelBehavior for emButton` or similar in this file). The caller already has `canvas_color: emColor` from the trait. Pass it as the second argument:

Before:
```rust
self.button.Paint(painter, w, h, enabled, pixel_scale);
```

After:
```rust
self.button.Paint(painter, canvas_color, w, h, enabled, pixel_scale);
```

If `self.button` is the wrong field name, use whatever field this struct uses (e.g. `self.body`, or the type's own `Paint` is called via `impl PanelBehavior::Paint`).

- [ ] **Step 5: Verify**

Run: `cargo check --workspace --tests`
Expected: `Finished` with no errors.

Run: `cargo clippy --workspace --tests -- -D warnings`
Expected: `Finished` with no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emButton.rs
git commit -m "$(cat <<'EOF'
refactor(F018): migrate emButton::Paint to use content_canvas_color

Replace painter.GetCanvasColor() read after paint_border with the
painter-free border.content_canvas_color() helper. Thread outer
canvas_color from the PanelBehavior::Paint trait caller.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: emCheckBox — migrate `Paint` method

**Files:**
- Modify: `crates/emcore/src/emCheckBox.rs:121-135`
- Modify: same file's trait-impl caller

- [ ] **Step 1: Confirm caller count**

Run: `grep -rn '\.check_box\.Paint\b\|emCheckBox::Paint\b' crates/ --include="*.rs"`

- [ ] **Step 2: Update `Paint` signature at `emCheckBox.rs:121`**

Current:
```rust
pub fn Paint(
    &mut self,
    painter: &mut emPainter,
    w: f64,
    h: f64,
    enabled: bool,
    pixel_scale: f64,
) {
```

New:
```rust
pub fn Paint(
    &mut self,
    painter: &mut emPainter,
    canvas_color: emColor,
    w: f64,
    h: f64,
    enabled: bool,
    pixel_scale: f64,
) {
```

- [ ] **Step 3: Replace the painter read at `emCheckBox.rs:135`**

Current:
```rust
self.border
    .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
let canvas_color = painter.GetCanvasColor();
```

New:
```rust
self.border
    .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
let canvas_color = self
    .border
    .content_canvas_color(canvas_color, &self.look, enabled);
```

- [ ] **Step 4: Update trait-impl caller**

Find caller in same file, add `canvas_color` as second argument.

- [ ] **Step 5: Verify**

Run: `cargo check --workspace --tests && cargo clippy --workspace --tests -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emCheckBox.rs
git commit -m "refactor(F018): migrate emCheckBox::Paint to use content_canvas_color

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: emCheckButton — migrate `Paint` method

**Files:**
- Modify: `crates/emcore/src/emCheckButton.rs:83-97`
- Modify: same file's trait-impl caller

- [ ] **Step 1: Confirm caller count**

Run: `grep -rn '\.check_button\.Paint\b\|emCheckButton::Paint\b' crates/ --include="*.rs"`

- [ ] **Step 2: Update `Paint` signature at `emCheckButton.rs:83`**

Current:
```rust
pub fn Paint(
    &mut self,
    painter: &mut emPainter,
    w: f64,
    h: f64,
    enabled: bool,
    pixel_scale: f64,
) {
```

New:
```rust
pub fn Paint(
    &mut self,
    painter: &mut emPainter,
    canvas_color: emColor,
    w: f64,
    h: f64,
    enabled: bool,
    pixel_scale: f64,
) {
```

- [ ] **Step 3: Replace the painter read at `emCheckButton.rs:97`**

Current:
```rust
self.border
    .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
let canvas_color = painter.GetCanvasColor();
```

New:
```rust
self.border
    .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
let canvas_color = self
    .border
    .content_canvas_color(canvas_color, &self.look, enabled);
```

- [ ] **Step 4: Update trait-impl caller** (pass `canvas_color` as second argument).

- [ ] **Step 5: Verify** — `cargo check --workspace --tests && cargo clippy --workspace --tests -- -D warnings` clean.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emCheckButton.rs
git commit -m "refactor(F018): migrate emCheckButton::Paint to use content_canvas_color

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: emColorField — migrate `Paint` method

**Files:**
- Modify: `crates/emcore/src/emColorField.rs:439-445`
- Modify: same file's trait-impl caller

- [ ] **Step 1: Confirm caller count**

Run: `grep -rn '\.color_field\.Paint\b\|emColorField::Paint\b' crates/ --include="*.rs"`

- [ ] **Step 2: Update `Paint` signature at `emColorField.rs:439`**

Current: `pub fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, pixel_scale: f64) {`

New: `pub fn Paint(&mut self, painter: &mut emPainter, canvas_color: emColor, w: f64, h: f64, pixel_scale: f64) {`

> Note: `emColorField::Paint` does not currently take `enabled`. Read the body to see what `enabled` value its `paint_border` call uses (line 444). Use that same value when calling `content_canvas_color`. If the call passes a literal `true`, use `true`. If it passes a state read like `self.enabled`, use that.

- [ ] **Step 3: Replace the painter read at `emColorField.rs:445`**

Current:
```rust
let mut canvas_color = painter.GetCanvasColor();
```

(Note `mut` — body mutates `canvas_color` later. Preserve `mut`.)

New (the `let mut canvas_color =` pattern stays; only the right-hand side changes):
```rust
let mut canvas_color = self
    .border
    .content_canvas_color(canvas_color, &self.look, /* enabled value matching paint_border arg */);
```

> **Watch out:** the parameter and the local share a name. Rust shadowing handles this fine — the right-hand side `canvas_color` resolves to the parameter, and the `let` introduces the local. Keep the mut.

- [ ] **Step 4: Update trait-impl caller** (pass `canvas_color` as second argument).

- [ ] **Step 5: Verify** — clean.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emColorField.rs
git commit -m "refactor(F018): migrate emColorField::Paint to use content_canvas_color

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: emListBox — migrate `Paint` method

**Files:**
- Modify: `crates/emcore/src/emListBox.rs:1105` (signature) and `:1221` (read site)
- Modify: same file's trait-impl `fn Paint` at `:160`

- [ ] **Step 1: Confirm caller count**

Run: `grep -rn '\.list_box\.Paint\b\|emListBox::Paint\b' crates/ --include="*.rs"`

- [ ] **Step 2: Update inherent `Paint` signature at `emListBox.rs:1105`**

Current: `pub fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, pixel_scale: f64) {`

New: `pub fn Paint(&mut self, painter: &mut emPainter, canvas_color: emColor, w: f64, h: f64, pixel_scale: f64) {`

- [ ] **Step 3: Replace the painter read at `emListBox.rs:1221`**

Locate the line:
```rust
                    painter.GetCanvasColor(),
```

This is inline-passed to some paint op (`PaintRect`/`PaintRoundRect`/etc.). Replace with the local `canvas_color` if one is in scope above this point in the function. If not, introduce a local at the top of the function body:

```rust
let canvas_color = self
    .border
    .content_canvas_color(canvas_color, &self.look, /* enabled */);
```

Read the surrounding context to determine whether the read at `:1221` represents the post-border content canvas (most likely, since it's after `paint_border` at `:1110`) or the outer canvas. If post-border: use `content_canvas_color`. If pre-border: use the parameter directly.

- [ ] **Step 4: Update the trait-impl `fn Paint` at `emListBox.rs:160`**

This is the trait method (already takes `canvas_color` from F018 commit `988e799d`). It calls `self.list_box.Paint(...)` somewhere — pass `canvas_color` as second argument.

- [ ] **Step 5: Verify** — clean.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emListBox.rs
git commit -m "refactor(F018): migrate emListBox::Paint to use content_canvas_color

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: emRadioBox — migrate `Paint` method

**Files:**
- Modify: `crates/emcore/src/emRadioBox.rs:114` (signature) and `:165` (read site)
- Modify: same file's trait-impl caller

- [ ] **Step 1: Confirm caller count**

Run: `grep -rn '\.radio_box\.Paint\b\|emRadioBox::Paint\b' crates/ --include="*.rs"`

- [ ] **Step 2: Update `Paint` signature at `emRadioBox.rs:114`**

Add `canvas_color: emColor` as the parameter immediately after `painter: &mut emPainter`. Match the existing parameter style in this file.

- [ ] **Step 3: Replace the painter read at `emRadioBox.rs:165`**

Current:
```rust
painter.PaintRoundRect(fx, fy, fw, fh, fr, fr, face_color, painter.GetCanvasColor());
```

This is inline. Introduce a local right after `paint_border` (line 126) using `content_canvas_color`:

```rust
self.border
    .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
let canvas_color = self
    .border
    .content_canvas_color(canvas_color, &self.look, enabled);
```

Then at `:165` replace `painter.GetCanvasColor()` with `canvas_color`:
```rust
painter.PaintRoundRect(fx, fy, fw, fh, fr, fr, face_color, canvas_color);
```

> Watch out: this file may have multiple inline `painter.GetCanvasColor()` reads. Replace **all** post-`paint_border` reads with `canvas_color`. Pre-`paint_border` reads (if any) take the parameter directly — but the spec asserts none exist; flag if you find one.

- [ ] **Step 4: Update trait-impl caller** (pass `canvas_color` as second argument).

- [ ] **Step 5: Verify** — clean.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emRadioBox.rs
git commit -m "refactor(F018): migrate emRadioBox::Paint to use content_canvas_color

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: emRadioButton — migrate `Paint` method

**Files:**
- Modify: `crates/emcore/src/emRadioButton.rs:323` (signature) and `:352` (read site)
- Modify: same file's trait-impl caller

- [ ] **Step 1: Confirm caller count**

Run: `grep -rn '\.radio_button\.Paint\b\|emRadioButton::Paint\b' crates/ --include="*.rs"`

- [ ] **Step 2: Update `Paint` signature at `emRadioButton.rs:323`**

Add `canvas_color: emColor` after `painter`.

- [ ] **Step 3: Replace the painter read at `emRadioButton.rs:352`**

Current:
```rust
painter.PaintRoundRect(fx, fy, fw, fh, fr, fr, face_color, painter.GetCanvasColor());
```

Same pattern as Task 6: add a local after `paint_border` (line 336):

```rust
self.border
    .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
let canvas_color = self
    .border
    .content_canvas_color(canvas_color, &self.look, /* enabled — match paint_border arg */);
```

Then replace `painter.GetCanvasColor()` at `:352` with `canvas_color`.

- [ ] **Step 4: Update trait-impl caller** (pass `canvas_color` as second argument).

- [ ] **Step 5: Verify** — clean.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emRadioButton.rs
git commit -m "refactor(F018): migrate emRadioButton::Paint to use content_canvas_color

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: emScalarField — migrate `Paint` method

**Files:**
- Modify: `crates/emcore/src/emScalarField.rs:319` (signature) and `:344` (read site)
- Modify: same file's trait-impl caller

- [ ] **Step 1: Confirm caller count**

Run: `grep -rn '\.scalar_field\.Paint\b\|emScalarField::Paint\b' crates/ --include="*.rs"`

- [ ] **Step 2: Update `Paint` signature at `emScalarField.rs:319`**

Add `canvas_color: emColor` after `painter`.

- [ ] **Step 3: Replace the painter read at `emScalarField.rs:344`**

Current:
```rust
let mut canvas_color = painter.GetCanvasColor();
```

New:
```rust
let mut canvas_color = self
    .border
    .content_canvas_color(canvas_color, &self.look, enabled);
```

Preserve `mut`. Body mutates `canvas_color` later; keep that intact.

- [ ] **Step 4: Update trait-impl caller** (pass `canvas_color` as second argument).

- [ ] **Step 5: Verify** — clean.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emScalarField.rs
git commit -m "refactor(F018): migrate emScalarField::Paint to use content_canvas_color

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 9: emSplitter — migrate `PaintContent`

**Files:**
- Modify: `crates/emcore/src/emSplitter.rs:140` (read site) — find enclosing fn signature for the parameter add
- Modify: same file's caller

- [ ] **Step 1: Find the enclosing function**

Run: `awk '/^    (pub )?fn / {sig=NR" "$0} NR==140 {print sig; exit}' crates/emcore/src/emSplitter.rs`
Expected: prints the function definition line. Likely `PaintContent`.

- [ ] **Step 2: Confirm caller count**

Run: `grep -rn '\.splitter\.PaintContent\b\|emSplitter::PaintContent\b\|\.PaintContent(' crates/emcore/src/emSplitter.rs`

- [ ] **Step 3: Update the function signature**

Add `canvas_color: emColor` after `painter`. Match existing parameter ordering.

- [ ] **Step 4: Replace the read at `emSplitter.rs:140`**

Current:
```rust
let canvas = painter.GetCanvasColor();
```

`emSplitter` does NOT use `border.paint_border` upstream — confirm by reading the function body. If the function paints chrome via a different path, the `canvas_color` from the parameter IS the canvas color; replace the read with the parameter directly:

```rust
let canvas = canvas_color;
```

If the function does call `paint_border` first, use `content_canvas_color` instead:

```rust
let canvas = self
    .border
    .content_canvas_color(canvas_color, &self.look, /* enabled */);
```

Read the surrounding code to determine which pattern applies.

- [ ] **Step 5: Update trait-impl caller** (pass `canvas_color` as second argument).

- [ ] **Step 6: Verify** — clean.

- [ ] **Step 7: Commit**

```bash
git add crates/emcore/src/emSplitter.rs
git commit -m "refactor(F018): migrate emSplitter::PaintContent to use canvas_color parameter

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 10: emTextField — migrate `Paint` method

**Files:**
- Modify: `crates/emcore/src/emTextField.rs:1220` (signature) and `:1234` (read site)
- Modify: same file's trait-impl caller

- [ ] **Step 1: Confirm caller count**

Run: `grep -rn '\.text_field\.Paint\b\|emTextField::Paint\b' crates/ --include="*.rs"`

- [ ] **Step 2: Update `Paint` signature at `emTextField.rs:1220`**

Add `canvas_color: emColor` after `painter`.

- [ ] **Step 3: Replace the painter read at `emTextField.rs:1234`**

Current:
```rust
let canvas_color = painter.GetCanvasColor();
```

New:
```rust
let canvas_color = self
    .border
    .content_canvas_color(canvas_color, &self.look, enabled);
```

- [ ] **Step 4: Update trait-impl caller** (pass `canvas_color` as second argument).

- [ ] **Step 5: Verify** — clean.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emTextField.rs
git commit -m "refactor(F018): migrate emTextField::Paint to use content_canvas_color

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 10.5: Layer 1 milestone verification

**Files:** none

- [ ] **Step 1: Verify all Layer 1 reads are gone**

Run:
```bash
grep -rn 'painter\.GetCanvasColor()' crates/ --include="*.rs" | grep -v tests/ | grep -v emBorder.rs
```
Expected: zero matches. Only `emBorder.rs` remains (Layer 2 next).

- [ ] **Step 2: Run full test suite**

Run: `cargo-nextest ntr` (or `cargo nextest run --workspace`)
Expected: all pass.

- [ ] **Step 3: Run golden-test report**

Run: `scripts/verify_golden.sh --report`
Expected: matches Task 0 baseline. No new divergences.

If a golden test regresses, **stop**. Use `python3 scripts/diff_draw_ops.py <name>` to identify the divergent op. Likely cause: wrong `enabled` value passed to `content_canvas_color`. Fix and re-verify before proceeding to Layer 2.

---

## Layer 2 — `emBorder` shared helpers (Tasks 11–14)

The four `emBorder` helpers each take `canvas_color: emColor` and stop reading from `painter`. Callers were already migrated in Layer 1 (the widget `Paint` methods now have `canvas_color` in scope). Layer 2 also touches every other caller — there are ~39 production callers of `paint_border` across widgets, groups, dialogs, control panels.

---

### Task 11: emBorder::paint_border — accept `canvas_color`

**Files:**
- Modify: `crates/emcore/src/emBorder.rs:1654` (signature) and `:1668` (read site)
- Modify: every caller of `paint_border` across the workspace (~30 sites)
- Modify: tests at `crates/emcore/src/emBorder.rs:3455, 3578, 3586`

- [ ] **Step 1: Inventory all callers**

Run: `grep -rn '\.paint_border\b' crates/ --include="*.rs"`
Expected: ~33 sites (production + tests). Save the list.

- [ ] **Step 2: Update `paint_border` signature**

Current (`emBorder.rs:1652-1663`):
```rust
/// Paint the border chrome.
#[allow(clippy::too_many_arguments)]
pub fn paint_border(
    &self,
    painter: &mut emPainter,
    w: f64,
    h: f64,
    look: &emLook,
    _focused: bool,
    enabled: bool,
    pixel_scale: f64,
) {
```

New:
```rust
/// Paint the border chrome.
#[allow(clippy::too_many_arguments)]
pub fn paint_border(
    &self,
    painter: &mut emPainter,
    canvas_color: emColor,
    w: f64,
    h: f64,
    look: &emLook,
    _focused: bool,
    enabled: bool,
    pixel_scale: f64,
) {
```

- [ ] **Step 3: Replace the read at `emBorder.rs:1668`**

Current:
```rust
let mut canvas_color = painter.GetCanvasColor();
```

New (parameter-shadowed local; the body's mutation pattern is preserved):
```rust
let mut canvas_color = canvas_color;
```

> **Note on `painter.SetCanvasColor` calls inside `paint_border`:** these stay for now. They're how `paint_border` publishes the post-border canvas color to subsequent painter reads in callers that haven't yet been migrated. They become dead in Phase B.

- [ ] **Step 4: Update every production caller**

For each caller in the inventory list, insert `canvas_color` as the **second** argument (after `painter`). Every caller is in a function that has `canvas_color: emColor` in scope (post-Layer 1). Examples:

`crates/emcore/src/emButton.rs:189-190`:

Before:
```rust
self.border
    .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
```

After:
```rust
self.border
    .paint_border(painter, canvas_color, w, h, &self.look, false, true, pixel_scale);
```

> **`canvas_color` source per caller:** the caller's *outer* canvas color (the `Paint` method's `canvas_color` parameter), NOT a `content_canvas_color()` value — `paint_border`'s job is to consume the outer canvas and produce the content canvas.

> **If a caller doesn't have `canvas_color` in scope, STOP and re-design.** This is the failure mode that broke the previous subagent attempt. Do not invent a value, do not fall back to `painter.GetCanvasColor()`. The spec's Risk 2 explicitly forbids it.

Production caller files (from the spec inventory; verify with grep):
- `emButton.rs:190`, `emCheckBox.rs:134`, `emColorField.rs:444`, `emRadioBox.rs:126`, `emScalarField.rs:343`, `emPackGroup.rs:43`, `emDialog.rs:653`, `emTextField.rs:1233`, `emCoreConfigPanel.rs` (8 sites), `emTunnel.rs:140`, `emAutoplayControlPanel.rs:411,674`, `emRadioButton.rs:336`, `emMainControlPanel.rs:255,887`, `emListBox.rs:1110`, `emLabel.rs:105`, `emRasterGroup.rs:43`, `emCheckButton.rs:96`, `emLinearGroup.rs:47`, `emFileSelectionBox.rs:1387`

For each of these files, the enclosing function/method that contains the call site already has `canvas_color: emColor` in scope (post-`988e799d` for trait impls, or already established in the function locally). Verify and pass it through.

- [ ] **Step 5: Update test callers**

In `emBorder.rs:3455`:
```rust
border.paint_border(&mut painter, 100.0, 100.0, &look, false, true, 1.0);
```
becomes:
```rust
border.paint_border(&mut painter, emColor::TRANSPARENT, 100.0, 100.0, &look, false, true, 1.0);
```

Apply the same pattern to `emBorder.rs:3578` and `emBorder.rs:3586`. Use `emColor::TRANSPARENT` for these unit tests unless the test specifically asserts a canvas color value (read context — adjust if so).

- [ ] **Step 6: Verify**

Run: `cargo check --workspace --tests && cargo clippy --workspace --tests -- -D warnings`
Expected: clean.

Run: `cargo-nextest ntr`
Expected: all pass. If a golden test regresses, the cause is most likely an incorrect `canvas_color` source at one of the callers — investigate with `scripts/diff_draw_ops.py`.

- [ ] **Step 7: Commit**

```bash
git add -u crates/
git commit -m "$(cat <<'EOF'
refactor(F018): emBorder::paint_border takes canvas_color parameter

Replace internal painter.GetCanvasColor() read with an explicit
canvas_color parameter. Update all ~30 production call sites and
unit tests to pass canvas_color from their PanelBehavior::Paint
trait scope.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 12: emBorder::paint_label_impl — accept `canvas_color`

**Files:**
- Modify: `crates/emcore/src/emBorder.rs:1557` (signature) and `:1610`, `:1632` (read sites)
- Modify: `paint_label` and `paint_label_colored` (the only callers) to pass through

- [ ] **Step 1: Read current signatures**

Run: `awk 'NR==1557,/^    \}/ && /^    (pub )?fn / {if (NR>1557) exit; print NR": "$0}' crates/emcore/src/emBorder.rs | head -30`

Confirm `paint_label_impl` signature, `paint_label` (line ~1525), and `paint_label_colored` (line ~1537).

- [ ] **Step 2: Add `canvas_color: emColor` parameter to all three signatures**

Each signature gains `canvas_color: emColor` immediately after `painter: &mut emPainter`. The two thin wrappers (`paint_label`, `paint_label_colored`) pass it through to `paint_label_impl`.

- [ ] **Step 3: Replace `paint_label_impl` reads**

At `emBorder.rs:1610`:
```rust
let label_canvas = painter.GetCanvasColor();
```
becomes:
```rust
let label_canvas = canvas_color;
```

At `emBorder.rs:1632`:
```rust
let label_canvas = painter.GetCanvasColor();
```
becomes:
```rust
let label_canvas = canvas_color;
```

- [ ] **Step 4: Update wrapper bodies**

Inside `paint_label` and `paint_label_colored`, the call to `paint_label_impl` adds `canvas_color` as the second argument.

- [ ] **Step 5: Update all callers of `paint_label` and `paint_label_colored`**

Run: `grep -rn '\.paint_label\b\|\.paint_label_colored\b' crates/ --include="*.rs"`

For each caller, pass `canvas_color` as the second argument. Source: the caller's `canvas_color: emColor` from its `Paint` trait scope.

> **Watch out — `paint_label_impl` reads were post-`paint_border`.** The label is painted AFTER the border, so the appropriate value is *content* canvas, not outer canvas. But the C++ original passes the painter's tracked canvas (which IS the post-border content canvas at that point). For migration: pass the post-`paint_border` `content_canvas_color` (the local already established in each Layer 1 widget) — NOT the outer trait `canvas_color`.
>
> Concretely: in `emButton::Paint` (after Task 1), the call site at line 230:
>
> ```rust
> self.border.paint_label_colored(
>     painter,
>     Rect::new(lx, ly, lw, lh),
>     &self.look,
>     color,
>     true,
> );
> ```
>
> becomes:
>
> ```rust
> self.border.paint_label_colored(
>     painter,
>     canvas_color,  // local, post-content_canvas_color shadowing
>     Rect::new(lx, ly, lw, lh),
>     &self.look,
>     color,
>     true,
> );
> ```
>
> Same rule for `emCheckButton::Paint` at line 136, `emCheckBox::Paint` at line 202, `emRadioButton::Paint` at line 378, `emRadioBox::Paint` at line 160, `emLabel::Paint` at line 115. Verify by reading each call site.

- [ ] **Step 6: Verify** — `cargo check --workspace --tests && cargo clippy --workspace --tests -- -D warnings && cargo-nextest ntr` clean.

- [ ] **Step 7: Commit**

```bash
git add -u crates/
git commit -m "$(cat <<'EOF'
refactor(F018): emBorder paint_label{,_colored,_impl} take canvas_color

Replace painter.GetCanvasColor() reads at emBorder.rs:1610 and :1632
with an explicit canvas_color parameter threaded through paint_label
and paint_label_colored from the caller's content canvas color.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 13: Layer 2 milestone verification

**Files:** none

- [ ] **Step 1: Verify zero `painter.GetCanvasColor()` reads remain in production**

Run:
```bash
grep -rn 'painter\.GetCanvasColor()' crates/ --include="*.rs" | grep -v tests/
```
Expected: zero matches. Only test files may still contain reads (Phase B Task 16 will reassess them).

- [ ] **Step 2: Full test + golden suite**

Run: `cargo-nextest ntr`
Expected: all pass.

Run: `scripts/verify_golden.sh --report`
Expected: matches Task 0 baseline.

If anything fails, isolate with `scripts/diff_draw_ops.py <failing_test>` and check whether a `canvas_color` source mismatch caused divergent ops.

---

# PHASE B — Retire painter canvas-color carrier

## Task 14: Locate the carrier

**Files:** none (research only)

- [ ] **Step 1: Find `DrawOp::SetCanvasColor` definition**

Run: `grep -rn 'SetCanvasColor' crates/emcore/src/render/ crates/emcore/src/emPainter.rs --include="*.rs" | head -30`
Expected: identifies the `DrawOp` enum file (likely `crates/emcore/src/render/draw_op.rs` or inside `painter.rs`), the variant name, and the compositor handlers in `software_compositor.rs` and `wgpu_compositor.rs`.

- [ ] **Step 2: Find `emPainter::canvas_color` field and methods**

Run: `grep -n 'canvas_color\|GetCanvasColor\|SetCanvasColor\|fn set_canvas_color' crates/emcore/src/emPainter.rs | head -30`

Note the line numbers of:
- Field definition (struct member of `emPainter` or its inner state).
- `pub fn GetCanvasColor(&self) -> emColor` at `:720`.
- `pub fn SetCanvasColor(&mut self, ...)`.

- [ ] **Step 3: Find all `painter.SetCanvasColor` call sites in production**

Run: `grep -rn 'painter\.SetCanvasColor\|\.SetCanvasColor(' crates/ --include="*.rs" | grep -v tests/`

This is the inventory for Task 15.

---

## Task 15: Remove SetCanvasColor call sites + DrawOp variant

**Files:**
- Modify: every file from Task 14 Step 3
- Modify: `crates/emcore/src/render/<draw_op_file>.rs` — remove `DrawOp::SetCanvasColor` variant
- Modify: `crates/emcore/src/render/software_compositor.rs` — remove the variant's match arm
- Modify: `crates/emcore/src/render/wgpu_compositor.rs` — remove the variant's match arm
- Modify: any DrawOp serialization / JSONL dump code that mentions the variant

- [ ] **Step 1: Delete `painter.SetCanvasColor` call sites in production**

For each call site from Task 14 Step 3, delete the line (it's a no-op with no reader after Phase A).

> **Watch out:** `paint_border` at `emBorder.rs:2038` and `:2246` calls `SetCanvasColor`. Delete both. Verify no nearby code still depends on the side effect (it shouldn't — Phase A removed all readers).

- [ ] **Step 2: Remove `DrawOp::SetCanvasColor` variant**

In the DrawOp enum file, delete the variant. In each compositor's match expression, delete the corresponding arm.

- [ ] **Step 3: Delete the public `SetCanvasColor` method on `emPainter`**

If `painter.SetCanvasColor(...)` was emitting the DrawOp variant, the method body is now unreachable. Delete the method.

- [ ] **Step 4: Verify**

Run: `cargo check --workspace --tests`
Expected: `Finished` with no errors.

> **If errors mention `DrawOp::SetCanvasColor` in JSONL serialization or `scripts/diff_draw_ops.py` schemas:** update those references too. They're tooling-side, not production semantics.

Run: `cargo clippy --workspace --tests -- -D warnings`
Expected: clean.

Run: `cargo-nextest ntr`
Expected: all pass.

- [ ] **Step 5: Regenerate golden ops baseline if tooling depends on the variant**

If `target/golden-divergence/divergence.jsonl` mentions `SetCanvasColor`, regenerate:
```bash
scripts/verify_golden.sh --regen
```

Run: `scripts/verify_golden.sh --report`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add -u crates/
git commit -m "$(cat <<'EOF'
refactor(F018): remove DrawOp::SetCanvasColor + painter.SetCanvasColor

After Phase A migration, no production code reads painter.GetCanvasColor(),
making SetCanvasColor (the publish path) dead. Remove the variant, its
compositor handlers, and all production call sites.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 16: Remove painter.canvas_color field + GetCanvasColor

**Files:**
- Modify: `crates/emcore/src/emPainter.rs` — remove field and `GetCanvasColor` method
- Modify: any test file that calls `painter.GetCanvasColor()` for assertions

- [ ] **Step 1: Remove the field from `emPainter`**

Locate the `canvas_color: emColor` field (likely in `struct emPainter` or its `PainterState` inner type). Delete the field. Remove any initialization in `emPainter::new(...)` and any update sites left over.

- [ ] **Step 2: Remove `pub fn GetCanvasColor` at `emPainter.rs:720`**

Delete the method.

- [ ] **Step 3: Fix test fallout**

Run:
```bash
cargo check --workspace --tests
```
Expected errors: test files that still call `painter.GetCanvasColor()`. From the spec inventory:
- `crates/eaglemode/tests/golden/test_panel.rs:1259, 1512, 2188, 2702`
- `crates/eaglemode/tests/golden/painter.rs:492`
- `crates/eaglemode/tests/golden/composition.rs:1146`

For each test:
- If the test uses `painter.GetCanvasColor()` to capture the post-`paint_border` content canvas, replace with `border.content_canvas_color(outer_canvas, &look, enabled)`.
- If the test asserts a value that no longer corresponds to anything (the field is gone), delete the assertion.
- If the test uses `ctx.GetCanvasColor()` (panel-side, not painter), no change — different API.

Use `cargo check` errors to drive the cleanup.

- [ ] **Step 4: Verify**

Run: `cargo check --workspace --tests && cargo clippy --workspace --tests -- -D warnings && cargo-nextest ntr`
Expected: clean.

Run: `scripts/verify_golden.sh --report`
Expected: matches baseline.

- [ ] **Step 5: Commit**

```bash
git add -u crates/
git commit -m "$(cat <<'EOF'
refactor(F018): remove emPainter::canvas_color field and GetCanvasColor

The painter no longer tracks canvas color: every paint operation
takes canvas_color explicitly, and the trait surface threads it
from the panel's declared canvas through PanelBehavior::Paint.
Test files that captured the field for assertions are migrated
to border.content_canvas_color() or the assertion is deleted.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# PHASE C — `WakeUp` pairing

## Task 17: Pair `WakeUpUpdateEngine` with `SVPChoiceByOpacityInvalid` writes

**Files:**
- Modify: `crates/emcore/src/emView.rs:3181-3189` (no-arg `InvalidatePainting`)
- Modify: `crates/emcore/src/emView.rs:3196-3239` (`invalidate_painting_rect`)
- Modify: callers — `emSubViewPanel.rs:463`, `emGUIFramework.rs:1394`, `emWindow.rs:1226`, `pipeline.rs:431`, `widget_interaction.rs:829`, `f018_iv3_svpchoice_invalidation.rs:35,52`, `emView.rs:5612, 5630, 5638`
- Modify: `crates/emcore/tests/f018_iv3_svpchoice_invalidation.rs` — add WakeUp assertion

- [ ] **Step 1: Write the failing assertion (TDD)**

Open `crates/emcore/tests/f018_iv3_svpchoice_invalidation.rs`. Find the test that calls `view.InvalidatePainting(&tree, panel_id)` at `:35`. Below the existing post-call assertion, add:

```rust
// F018 Phase 2 follow-up: InvalidatePainting must pair the
// SVPChoiceByOpacityInvalid write with a WakeUp() — mirrors C++
// emPanel::InvalidatePainting (emPanel.cpp:1282-1289).
assert!(
    sched.borrow().is_woken(eid),
    "InvalidatePainting must wake the engine paired with SVPChoiceByOpacityInvalid"
);
```

> **API note:** the exact scheduler "is woken" API may differ. Read `crates/emcore/src/emScheduler.rs` to find the right method (e.g. `is_woken`, `pending_wakeups`, etc.). If no public observer exists, add one as a `#[cfg(any(test, feature = "test-support"))]` helper. Do not change runtime semantics to make this observable.

- [ ] **Step 2: Verify the test fails for the right reason**

Run:
```bash
cargo nextest run --features test-support -p emcore --test f018_iv3_svpchoice_invalidation
```
Expected: FAIL with the new assertion failing (engine not woken). If it fails for a compile reason (signature mismatch), proceed to Step 3 and revisit.

- [ ] **Step 3: Update `InvalidatePainting` signature and body**

At `emView.rs:3181`, change signature:

Current:
```rust
pub fn InvalidatePainting(&mut self, tree: &PanelTree, panel: PanelId) {
```

New:
```rust
pub fn InvalidatePainting(
    &mut self,
    ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    tree: &PanelTree,
    panel: PanelId,
) {
```

Body: after the existing `self.SVPChoiceByOpacityInvalid = true;` at `:3188`, add:

```rust
self.WakeUpUpdateEngine(ctx);
```

- [ ] **Step 4: Update `invalidate_painting_rect` signature and body**

At `emView.rs:3196`, change signature:

Current:
```rust
pub fn invalidate_painting_rect(
    &mut self,
    tree: &PanelTree,
    panel: PanelId,
    x: f64, y: f64, w: f64, h: f64,
) {
```

New:
```rust
pub fn invalidate_painting_rect(
    &mut self,
    ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    tree: &PanelTree,
    panel: PanelId,
    x: f64, y: f64, w: f64, h: f64,
) {
```

Body: after the existing `self.SVPChoiceByOpacityInvalid = true;` at `:3237`, add:

```rust
self.WakeUpUpdateEngine(ctx);
```

> **Important:** the existing write is inside `if vw > 0.0 && vh > 0.0 { ... }`. The WakeUp must be inside the same block so it only fires when the rect actually contributes to dirty state — matching C++ emPanel.cpp:1294-1300.

- [ ] **Step 5: Update production callers**

Each caller receives `ctx` as the first argument after `&mut self`:

`crates/emcore/src/emSubViewPanel.rs:463`:

Before:
```rust
self.sub_view.InvalidatePainting(&self.sub_tree, panel_id);
```

After (use whatever ctx variable is in scope at the call site — read the enclosing function):
```rust
self.sub_view.InvalidatePainting(ctx, &self.sub_tree, panel_id);
```

Same pattern for `emGUIFramework.rs:1394`, `emWindow.rs:1226`. If a caller doesn't have a `SchedCtx` in scope, **stop and investigate** — that's an architectural question, not a plumbing question.

- [ ] **Step 6: Update test callers**

For each test caller (`pipeline.rs:431`, `widget_interaction.rs:829`, `f018_iv3_svpchoice_invalidation.rs:35,52`, `emView.rs:5612, 5630, 5638`), pass the `SchedCtx` available in scope. The IV.3 test harness already constructs a `SchedCtx`; pass it. Inline tests in `emView.rs` may need to construct one — follow the patterns in adjacent tests in the same file.

- [ ] **Step 7: Verify the new assertion passes**

Run:
```bash
cargo nextest run --features test-support -p emcore --test f018_iv3_svpchoice_invalidation
```
Expected: PASS.

- [ ] **Step 8: Full verification**

Run: `cargo check --workspace --tests && cargo clippy --workspace --tests -- -D warnings && cargo-nextest ntr`
Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add -u crates/
git commit -m "$(cat <<'EOF'
fix(F018): pair WakeUpUpdateEngine with SVPChoiceByOpacityInvalid writes

emView::InvalidatePainting and invalidate_painting_rect now take
ctx: &mut SchedCtx<'_> and call WakeUpUpdateEngine immediately after
each SVPChoiceByOpacityInvalid = true write. Mirrors C++
emPanel::InvalidatePainting (emPanel.cpp:1282-1300) which pairs the
flag write with View.UpdateEngine->WakeUp().

Phase 2 reviewer Minor #1 follow-up.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Closeout

## Task 18: Update ISSUES.json + final verification

**Files:**
- Modify: `docs/debug/ISSUES.json` — F018 entry's `fix_note`

- [ ] **Step 1: Final full-workspace verification**

Run: `cargo check --workspace --tests && cargo clippy --workspace --tests -- -D warnings && cargo-nextest ntr`
Expected: clean.

Run: `scripts/verify_golden.sh --report`
Expected: matches Task 0 baseline (or differs only by intentional improvements like the absence of `DrawOp::SetCanvasColor` ops).

- [ ] **Step 2: Confirm zero `painter.GetCanvasColor()` reads anywhere**

Run:
```bash
grep -rn 'painter\.GetCanvasColor\b\|painter\.SetCanvasColor\b' crates/ --include="*.rs"
```
Expected: zero matches (or only references in a comment / removed-code commit message you missed).

Run:
```bash
grep -rn 'DrawOp::SetCanvasColor' crates/ --include="*.rs"
```
Expected: zero matches.

- [ ] **Step 3: Update F018 fix_note**

Edit `docs/debug/ISSUES.json`. The F018 entry's `fix_note` currently lists Phase 3 deferrals and the WakeUp follow-up. Append a closing paragraph:

```
Phase 3 (rule II.5) and Phase 2 follow-up #1 completed in commits
[list the commit shas of Tasks 1–17]: all painter.GetCanvasColor()
reads removed from production, painter.canvas_color field +
DrawOp::SetCanvasColor variant retired, emView::InvalidatePainting
and invalidate_painting_rect now pair SVPChoiceByOpacityInvalid
writes with WakeUpUpdateEngine. Manual visual verification (loading-
state grey background) still pending — see "Manual verification"
note above. Status remains needs-manual-verification until that
runtime check is signed off.
```

Use `python3 -c "import json; d=json.load(open('docs/debug/ISSUES.json')); ..."` or direct edit. Preserve JSON formatting.

- [ ] **Step 4: Commit**

```bash
git add docs/debug/ISSUES.json
git commit -m "$(cat <<'EOF'
docs(F018): record phase 3 deferrals + WakeUp follow-up complete

F018 status remains needs-manual-verification (the runtime visual
gate is independent of the deferral fixes).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Self-review notes

**Spec coverage:** every spec section has tasks — Phase A Layer 1 (Tasks 1–10), Layer 1 milestone (10.5), Layer 2 (Tasks 11–13), Phase B (Tasks 14–16), Phase C (Task 17), closeout (Task 18).

**Risk mitigations:**
- Risk 1 (hidden helper-to-helper calls): every Layer 1 task has a Step 1 grep to confirm caller count.
- Risk 2 (Layer 2 callers without canvas_color in scope): Task 11 Step 4 explicitly forbids inventing values and references the spec's stop-and-redesign rule.
- Risk 3 (DrawOp tooling): Task 15 Step 5 covers regeneration.
- Risk 4 (Phase C ctx not in scope): Task 17 Step 5 explicitly forbids papering over.

**Open question for the implementer:** Layer 2 (Task 12) "what canvas color does paint_label use" needs verification at implementation time. The plan asserts post-`content_canvas_color` value (the local `canvas_color` after Layer 1 shadowing), based on C++ semantics where the painter's tracked canvas at label-paint time is the post-border content canvas. If diff_draw_ops shows divergence after Task 12, this is the first thing to re-check.
