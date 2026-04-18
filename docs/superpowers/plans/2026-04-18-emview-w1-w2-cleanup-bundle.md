# emView W1+W2 Cleanup Bundle — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clear the surviving `PHASE-6-FOLLOWUP:` marker, eliminate silent drift in the `Visit*` navigation cluster, remove `pump_visiting_va` from the public API, rename W4 arity-overload suffixes, and clear reviewer one-liners — all in one coordinated bundle.

**Architecture:** Eight independent tasks, each committed separately. Task 2 (nav gate removal) is the only one with non-trivial risk and lands last. Tasks 1, 4, 6, 7, 8 are doc-only or mechanical; 3 is Cargo-plumbing; 5 is rename-only.

**Tech Stack:** Rust 2021 (emcore, emmain, eaglemode crates); cargo workspaces; cargo-nextest; bitflags. No external additions.

**Parent spec:** `docs/superpowers/specs/2026-04-18-emview-w1-w2-cleanup-bundle-design.md`.

---

## Ground rules for implementers

- **Before every task:** read `CLAUDE.md` in the repo root. In particular: "File and Name Correspondence", "Port Fidelity", and "Do NOT" sections.
- **Commits:** one commit per task. Run the pre-commit hook (`cargo fmt` + `cargo clippy -- -D warnings` + `cargo-nextest ntr`). Never `--no-verify`.
- **Verification baseline:** `cargo-nextest ntr` at 2425/2425 (9 skipped, 0 failed); golden at 237 passed / 6 failed (same pre-existing failures).
- **Stop-and-ask triggers:** any deviation from the step text; any task where observed file state contradicts the plan; any test that was green and is now red after your change.
- **Out of scope for this plan:** any `PHASE-W4-FOLLOWUP:` marker; scheduler re-entrant borrow; per-view notice dispatch; `CoreConfig` ownership; W3 surface de-dup. Each has its own sibling sub-project.

---

## Task 1: Promote `emView::Input` animator-forward comment to `DIVERGED:` marker

**Rationale.** The reviewer flagged the forward as "missing" but the existing Rust design deliberately locates it on the animator-owner callers (`emWindow::dispatch_input`, `emSubViewPanel::Behavior::Input`). Observable behavior already matches C++ — only the *location* of the forward differs. Per CLAUDE.md, structural divergences require a `DIVERGED:` marker at the point of divergence. The current comment at `emView.rs:3778-3797` is a prose blob; promote it to the formal marker form and drop the `PHASE-6-FOLLOWUP:` deferral tag.

**Files:**
- Modify: `crates/emcore/src/emView.rs:3778-3797`

- [ ] **Step 1: Re-read the current comment to confirm what exists**

Run:
```bash
sed -n '3770,3810p' crates/emcore/src/emView.rs
```

Expected content (the target of this task):
```rust
    /// PHASE-6-FOLLOWUP: migrate the VIF-chain + panel-broadcast dispatch
    /// from `RecurseInput` once its Rust port exists. The animator forward
    /// (C++ emView.cpp:1004) is handled by the caller sites
    /// (`emWindow::dispatch_input`, `emSubViewPanel::Behavior::Input`)
    /// because the animator lives on those owners, not on `emView`.
    pub fn Input(
        &mut self,
        _tree: &mut PanelTree,
        _event: &crate::emInput::emInputEvent,
        state: &crate::emInputState::emInputState,
    ) {
        // C++ emView.cpp:1004: forward input to ActiveAnimator first.
        // Rust-arch note: the active animator lives on emWindow
        // (see emWindow::dispatch_input) and on emSubViewPanel
        // (see emSubViewPanel::Behavior::Input), not on emView — by the
        // Phase 5/6 design decision. Those callers forward the event to
        // their animator slot BEFORE invoking this method, so by the time
        // Input runs here the event may already have been eaten.

        // emView.cpp:1006-1014: cursor-invalid on mouse move.
```

If the lines or content differ, STOP and ask — the file has drifted from the plan's baseline.

- [ ] **Step 2: Replace the doc comment above `pub fn Input`**

Use the Edit tool to replace the five-line `/// PHASE-6-FOLLOWUP: ...` doc block with a proper `DIVERGED:` block.

Old text (doc comment above `pub fn Input`):
```rust
    /// PHASE-6-FOLLOWUP: migrate the VIF-chain + panel-broadcast dispatch
    /// from `RecurseInput` once its Rust port exists. The animator forward
    /// (C++ emView.cpp:1004) is handled by the caller sites
    /// (`emWindow::dispatch_input`, `emSubViewPanel::Behavior::Input`)
    /// because the animator lives on those owners, not on `emView`.
```

New text:
```rust
    /// Port of C++ `emView::Input` (emView.cpp:1004).
    ///
    /// DIVERGED: animator-forward location. C++ `emView::Input` begins with
    /// `ActiveAnimator->Input(event, state)` at emView.cpp:1004, because
    /// C++ `emView` owns the `ActiveAnimator` field. In the Rust port the
    /// active animator instead lives on the input-dispatch *owner* —
    /// `emWindow::active_animator` (see `emWindow::dispatch_input` at
    /// emWindow.rs:840-862) and `emSubViewPanel::active_animator` (see
    /// `emSubViewPanel::Behavior::Input`). Both callers forward the event
    /// to their animator slot BEFORE invoking this method, so the
    /// C++-equivalent invariant ("animator sees input first") is preserved.
    /// This is a structural — not observable — divergence, retained because
    /// giving `emView` its own animator field would duplicate the slot and
    /// cascade ownership changes across the input dispatch chain.
```

- [ ] **Step 3: Replace the in-body comment inside `pub fn Input`**

Old text (inside the method body, immediately before the `// emView.cpp:1006-1014: cursor-invalid on mouse move.` line):
```rust
        // C++ emView.cpp:1004: forward input to ActiveAnimator first.
        // Rust-arch note: the active animator lives on emWindow
        // (see emWindow::dispatch_input) and on emSubViewPanel
        // (see emSubViewPanel::Behavior::Input), not on emView — by the
        // Phase 5/6 design decision. Those callers forward the event to
        // their animator slot BEFORE invoking this method, so by the time
        // Input runs here the event may already have been eaten.

```

New text: delete the block entirely (leave the method body starting at `// emView.cpp:1006-1014: cursor-invalid on mouse move.`). The information now lives on the doc comment, and duplicating it in the body adds noise.

- [ ] **Step 4: Also audit the duplicate comment inside `emWindow::dispatch_input`**

Run:
```bash
sed -n '840,863p' crates/emcore/src/emWindow.rs
```

The block at `emWindow.rs:840-862` contains a similar "Rust-arch note" prose comment above the actual forward code. Leave the code unchanged; replace the Rust-arch note comment block with a terse reference back to `emView::Input`'s DIVERGED block:

Old comment (lines 840-844, above `let mut event = event.clone();`):
```rust
        // C++ emView.cpp:1004: forward input to ActiveAnimator first.
        // Rust-arch note: the animator lives on emWindow (not emView) by the
        // Phase 5/6 decision, so this forward happens in the caller. A
        // `visiting` animator may eat the event here, in which case the VIF
        // chain and panel broadcast below see an empty event.
```

New comment:
```rust
        // C++ emView.cpp:1004: forward input to ActiveAnimator first. The
        // animator slot lives here (not on `emView`) per the Rust
        // structural divergence documented on `emView::Input`. A `visiting`
        // animator may eat the event, in which case the VIF chain and
        // panel broadcast below see an empty event.
```

- [ ] **Step 5: Verify the `PHASE-6-FOLLOWUP:` marker is gone**

Run:
```bash
grep -rn "PHASE-6-FOLLOWUP" crates/
```

Expected: zero output. Any hit indicates either a missed replacement in Step 2 or a second unrelated marker that was not in scope — STOP and investigate.

- [ ] **Step 6: Verify the `DIVERGED:` marker is present**

Run:
```bash
grep -n "DIVERGED: animator-forward" crates/emcore/src/emView.rs
```

Expected: exactly one match in the doc block above `pub fn Input`.

- [ ] **Step 7: Build and test**

Run:
```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```

Expected: clippy clean; 2425 tests pass, 9 skipped, 0 failed. Zero code changes means zero behavior change — any test delta is a bug.

- [ ] **Step 8: Commit**

```bash
git add crates/emcore/src/emView.rs crates/emcore/src/emWindow.rs
git commit -m "$(cat <<'EOF'
docs(emView): promote animator-forward PHASE-6-FOLLOWUP to DIVERGED marker

The Rust port architecturally locates emView.cpp:1004's animator-forward
on the animator-owner callers (emWindow::dispatch_input, emSubViewPanel)
rather than inside emView::Input, because emView in Rust does not own
the animator slot. Observable behavior matches C++; location differs.

Replaces the ad-hoc prose comment with a formal DIVERGED: block and
removes the stale PHASE-6-FOLLOWUP: prefix now that the architectural
question is resolved.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Navigation `NO_NAVIGATE` gate removal + caller audit

**Rationale.** The seven methods `VisitNext`, `VisitPrev`, `VisitFirst`, `VisitLast`, `VisitIn`, `VisitOut`, `VisitNeighbour` each begin with `if self.flags.intersects(NO_NAVIGATE | NO_USER_NAVIGATION) { return; }`. C++ `emView.cpp:564-762` has no such gate — gating happens at the caller (input/keybinding handlers). This is silent drift: pre-wave Rust behavior that the W4 plan preserved without a `DIVERGED:` marker. Remove the gate inside the methods and audit every caller to confirm user-nav sites still gate (programmatic callers must not).

**Files:**
- Modify: `crates/emcore/src/emView.rs` — lines 2421-2570 (seven nav methods, ~7 if-blocks).
- Audit (may modify): every call site of those seven methods in `crates/**`.

- [ ] **Step 1: Enumerate current callers of each nav method**

Run these greps and capture the output for the audit:

```bash
for m in VisitNext VisitPrev VisitFirst VisitLast VisitIn VisitOut VisitNeighbour; do
    echo "=== $m ==="
    grep -rn "\.${m}(" crates/ | grep -v '^crates/emcore/src/emView\.rs:24[0-9][0-9]:' | grep -v '^crates/emcore/src/emView\.rs:25[0-9][0-9]:' | grep -v '^crates/emcore/src/emView\.rs:26[0-9][0-9]:' || true
done
```

Record the list of caller file:line pairs. Each will be classified in Step 2 as user-nav (needs gate) or programmatic (no gate).

Expected: callers live in at least `emViewInputFilter.rs`, keybinding/input handler sites, possibly test code, and the internal `VisitLeft`/`VisitRight`/`VisitUp`/`VisitDown` four-line delegators at `emView.rs:2506-2524` that call `VisitNeighbour`.

- [ ] **Step 2: Classify each caller**

For each caller from Step 1, read ~20 lines of context and classify:

- **User-nav caller:** responds to an input event (keyboard, mouse, touch, keybinding). MUST gate on `NO_USER_NAVIGATION` before calling the Visit* method. The four `VisitLeft/Right/Up/Down` delegators at `emView.rs:2506-2524` count as internal plumbing and are NOT user-nav — their callers are.
- **Programmatic caller:** invoked from animator, internal state machine, test, or any non-input path. MUST NOT gate.
- **Uncertain:** if the context doesn't make classification clear, STOP and ask before proceeding.

Write the classification into a temporary comment block at the top of this task in your working notes (scratch file, not committed). Example:
```
emViewInputFilter.rs:XXX — user-nav (key handler) — already gates at XXX: yes/no
emViewInputFilter.rs:YYY — user-nav — already gates: yes/no
emView.rs:2508 VisitLeft — internal delegator — no gate needed here
tests/... — programmatic — no gate
```

Expected-ish caller set based on C++ emView.cpp: key handlers in `emView::Input`-adjacent code (the four arrow-key bindings landed in `emWindow::dispatch_input` fallback during W4 Phase 4), VIF entries in `emViewInputFilter.rs`, internal delegators, and tests.

- [ ] **Step 3: Stop-and-ask check — is the caller set what you expected?**

The closeout doc expects: input-dispatch sites + VIF + internal delegators. If the audit reveals an unexpected caller class (e.g., a timer, a notice handler, a random panel-side call), STOP and escalate — it may encode a Rust-only feature that the gate was papering over.

- [ ] **Step 4: For each user-nav caller that doesn't already gate, add the gate**

At each user-nav site that currently doesn't check `NO_USER_NAVIGATION`, insert:
```rust
if view.flags.contains(ViewFlags::NO_USER_NAVIGATION) {
    return;
}
```
immediately before the `VisitNext/Prev/...` call. (Field access may differ — use the local `view` reference or `self.view` as appropriate.) Do NOT gate on `NO_NAVIGATE` at user-nav sites; C++ doesn't.

- [ ] **Step 5: Remove the gate from all seven nav methods**

Use Edit with `replace_all` to strike each of the seven if-blocks. Find them at `emView.rs:2421-2570`:

For `VisitNext` (lines 2421-2427), replace:
```rust
    pub fn VisitNext(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let Some(active) = self.active else { return };
```
with:
```rust
    pub fn VisitNext(&mut self, tree: &mut PanelTree) {
        let Some(active) = self.active else { return };
```

Repeat for `VisitPrev` (lines 2447-2453), `VisitFirst` (lines 2473-2479), `VisitLast` (lines 2490-2496), `VisitIn` (lines 2527-2533), `VisitOut` (lines 2543-2549), `VisitNeighbour` (lines 2565-2571). Each has the identical 5-line if-block to remove.

- [ ] **Step 6: Add a comment explaining the new gating contract**

At the top of the navigation-methods block (above `/// Port of C++ \`emView::VisitNext()\` (emView.cpp:564-578).` at emView.rs:2420), add:
```rust
    // Navigation methods (`Visit{Next,Prev,First,Last,Left,Right,Up,Down,In,Out,Neighbour}`)
    // match C++ emView.cpp:564-762 exactly — no internal `NO_NAVIGATE`/
    // `NO_USER_NAVIGATION` gate. User-navigation callers (input dispatch,
    // keybindings, VIF cheats) gate on `NO_USER_NAVIGATION` before calling
    // these methods; programmatic callers (animators, tests) do not gate.
```

- [ ] **Step 7: Build and test**

Run:
```bash
cargo clippy -- -D warnings
cargo-nextest ntr
cargo test --test golden -- --test-threads=1
```

Expected: clippy clean; 2425/2425 nextest; golden 237 passed / 6 failed (same baseline). Any regression beyond the baseline six golden failures is a bug — most likely an unflagged user-nav site. STOP and inspect.

- [ ] **Step 8: Verify gate removal is complete**

Run:
```bash
grep -n "NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION" crates/emcore/src/emView.rs
```

Expected: zero matches in the nav-method region (lines 2420-2600). Other hits are fine (e.g., `new_flags.insert(ViewFlags::NO_USER_NAVIGATION);` at line 1342 is unrelated).

- [ ] **Step 9: Commit**

```bash
git add crates/emcore/src/emView.rs crates/emcore/src/emViewInputFilter.rs
# plus any other modified caller files
git commit -m "$(cat <<'EOF'
refactor(emView): remove NO_NAVIGATE gate from Visit* nav methods; gate at callers

C++ emView.cpp:564-762 has no internal NO_NAVIGATE/NO_USER_NAVIGATION
check in the seven Visit* navigation methods; gating happens at user-nav
call sites. The Rust port had drifted by embedding the gate inside each
method, which silently blocked programmatic callers (animators, tests)
that C++ would have admitted.

Strip the internal gate and audit every caller: user-nav sites already
gated (or now gate) on NO_USER_NAVIGATION; programmatic callers go
ungated, matching C++ behavior.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: `pump_visiting_va` visibility fix via `test-support` feature

**Rationale.** `emView::pump_visiting_va` (`emView.rs:3113`) is currently `pub`, which leaks the symbol to every `emcore` consumer. It's a test-only helper; consumers are `crates/eaglemode/tests/golden/interaction.rs` (many), `crates/eaglemode/tests/support/pipeline.rs:87`, and inline tests in `emView.rs`. Gate it behind a cargo feature so non-test consumers of emcore can't see it.

**Files:**
- Modify: `crates/emcore/Cargo.toml` — add `[features] test-support = []`.
- Modify: `crates/emcore/src/emView.rs:3108-3126` — add `#[cfg(any(test, feature = "test-support"))]` above `pub fn pump_visiting_va`.
- Modify: `crates/eaglemode/Cargo.toml` — add `features = ["test-support"]` to the `emcore` dev-dependency entry.

- [ ] **Step 1: Add the feature to emcore's Cargo.toml**

Edit `crates/emcore/Cargo.toml`. After the `[dependencies]` section, before `[dev-dependencies]`, add:

```toml
[features]
default = []
test-support = []
```

Verify the resulting file has a `[features]` section with exactly those two lines (plus the header).

- [ ] **Step 2: Gate `pump_visiting_va` with the feature**

In `crates/emcore/src/emView.rs`, locate `pub fn pump_visiting_va` at line 3113. The current declaration is:

```rust
    /// Test-only: drive `VisitingVA::animate` directly until it deactivates
    /// or the iteration limit is hit. Production path goes through
    /// `VisitingVAEngineClass::Cycle` which requires a window registry; unit
    /// tests without a window use this to observe the post-convergence active
    /// panel after a `Visit*` call.
    pub fn pump_visiting_va(&mut self, tree: &mut PanelTree) {
```

Replace with:

```rust
    /// Test-only: drive `VisitingVA::animate` directly until it deactivates
    /// or the iteration limit is hit. Production path goes through
    /// `VisitingVAEngineClass::Cycle` which requires a window registry; unit
    /// tests without a window use this to observe the post-convergence active
    /// panel after a `Visit*` call.
    ///
    /// Gated behind the `test-support` cargo feature so the symbol does not
    /// leak into the non-test public API. Cross-crate test consumers must
    /// enable `features = ["test-support"]` on their `emcore` dev-dependency.
    #[cfg(any(test, feature = "test-support"))]
    pub fn pump_visiting_va(&mut self, tree: &mut PanelTree) {
```

- [ ] **Step 3: Add the feature to eaglemode's dev-dep**

Edit `crates/eaglemode/Cargo.toml`. The current `[dev-dependencies]` block has:

```toml
[dev-dependencies]
emfileman = { path = "../emfileman" }
emstocks = { path = "../emstocks" }
rand = { workspace = true }
slotmap = { workspace = true }
criterion = { workspace = true }
gungraun = { workspace = true }
```

`emcore` is in `[dependencies]`, not `[dev-dependencies]`. To enable the feature *only for tests*, we need to add an explicit dev-dep entry for emcore with the feature flag. Add this line inside `[dev-dependencies]`:

```toml
emcore = { workspace = true, features = ["test-support"] }
```

This tells cargo to re-activate emcore for dev builds with the additional `test-support` feature, while normal builds see only the default feature set.

- [ ] **Step 4: Check other crates that might reference `pump_visiting_va`**

Run:
```bash
grep -rn "pump_visiting_va" crates/ --include="*.rs"
```

Expected caller locations:
- `crates/emcore/src/emView.rs` (definition + inline tests — both covered by `cfg(test)` branch of the gate)
- `crates/eaglemode/tests/golden/interaction.rs` — covered by the eaglemode dev-dep feature
- `crates/eaglemode/tests/support/pipeline.rs:87` — same

If any *other* crate references `pump_visiting_va`, its `Cargo.toml` also needs the `features = ["test-support"]` dev-dep line. STOP and add if so.

- [ ] **Step 5: Build and test**

Run:
```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```

Expected: clippy clean; 2425/2425 nextest. The eaglemode integration tests must still compile and pass — failure here almost certainly means a missing feature flag on a dev-dep.

- [ ] **Step 6: Verify non-test builds can no longer see the symbol**

Run:
```bash
cargo build --release 2>&1 | grep -i pump_visiting_va || echo "OK: not referenced in release build"
cargo check --lib -p emcore 2>&1 | grep -i pump_visiting_va || echo "OK: emcore lib-only build hides pump_visiting_va"
```

Expected: both print "OK:". Any compile error mentioning `pump_visiting_va` indicates a non-test path still depends on it.

- [ ] **Step 7: Commit**

```bash
git add crates/emcore/Cargo.toml crates/emcore/src/emView.rs crates/eaglemode/Cargo.toml
git commit -m "$(cat <<'EOF'
refactor(emView): gate pump_visiting_va behind test-support feature

pump_visiting_va is a test-only pump for driving VisitingVA convergence
without a scheduler/window registry. Previously `pub` — leaked to every
emcore consumer. Now `#[cfg(any(test, feature = "test-support"))]`, with
eaglemode enabling the feature on its dev-dep entry.

Closes §5.1/item 7 from the emView closeout: test API no longer visible
on the production public surface.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Co-locate `VisitByIdentity` with `Visit`

**Rationale.** File-and-Name Correspondence expects `VisitByIdentity` adjacent to its panel-form counterpart `Visit`. C++ places them together at `emView.cpp:492-523`. The Rust port has `Visit` at `emView.rs:857-869` but the 7-arg `VisitByIdentity` at `emView.rs:3085-3098`, ~2200 lines away. Mechanical move; no logic change.

**Files:**
- Modify: `crates/emcore/src/emView.rs` — cut `VisitByIdentity` from ~3085 and paste adjacent to `Visit` at ~857.

- [ ] **Step 1: Locate and read `VisitByIdentity`**

Run:
```bash
sed -n '3074,3099p' crates/emcore/src/emView.rs
```

Expected: the doc comment starting `/// Port of C++ \`emView::Visit(identity, relX, relY, relA, adherent, subject)\``, the `PHASE-W4-FOLLOWUP:` comment, and the `pub fn VisitByIdentity(...)` body, ending with `va.Activate();` and the closing brace on line 3098.

Copy the full block (lines 3074-3098, inclusive of the doc comment and closing brace) to a scratch buffer.

- [ ] **Step 2: Locate the insertion point**

Run:
```bash
sed -n '854,920p' crates/emcore/src/emView.rs
```

Expected: `Visit` at ~857 (ends on the line containing `self.VisitByIdentity(&identity, rel_x, rel_y, rel_a, adherent, &subject);` followed by a closing `}`), then `VisitFullsized` at ~872, then `VisitFullsizedByIdentity` at ~889.

The natural insertion point is *immediately after `Visit`'s closing brace* (so `Visit` and `VisitByIdentity` are adjacent, with `VisitFullsized`+`VisitFullsizedByIdentity` forming the next adjacent pair below).

- [ ] **Step 3: Delete `VisitByIdentity` at its current location**

Use Edit to delete the block at lines 3074-3098. Verify with:
```bash
grep -n "pub fn VisitByIdentity" crates/emcore/src/emView.rs
```
Expected: output shows `VisitByIdentity` at roughly line 870-880 (its new home after step 4) — no result from the old location. (If both appear before step 4 completes, only one is expected; if both appear after, abort and reset.)

- [ ] **Step 4: Insert `VisitByIdentity` adjacent to `Visit`**

Paste the block from Step 1 immediately after `Visit`'s closing brace at ~869. Ensure blank-line spacing matches surrounding methods (one blank line between methods).

- [ ] **Step 5: Verify only one definition exists, at the new location**

Run:
```bash
grep -n "pub fn VisitByIdentity" crates/emcore/src/emView.rs
```
Expected: exactly one match, and its line is in the 860-880 range (adjacent to `Visit`).

- [ ] **Step 6: Build and test**

Run:
```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: clippy clean; 2425/2425 nextest. Pure move — any test regression is a copy/paste error.

- [ ] **Step 7: Commit**

```bash
git add crates/emcore/src/emView.rs
git commit -m "$(cat <<'EOF'
refactor(emView): co-locate VisitByIdentity with Visit

C++ colocates Visit/VisitByIdentity at emView.cpp:492-523. Rust had
them ~2200 lines apart. Mechanical move; no logic change.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Rename `VisitByIdentityShort` → `VisitByIdentityBare` and `SetGoalWithCoords` → `SetGoalCoords`

**Rationale.** The W4 arity-overload family picked antonym-style suffixes (`Short` paired with `Fullsized`; `WithCoords` reading as if coords were optional). Uniform semantic suffixes make the family self-describing. Rename while external consumers are still zero; later the cost rises.

**Files:**
- Modify: `crates/emcore/src/emView.rs` — definitions + call sites of `VisitByIdentityShort`.
- Modify: `crates/emcore/src/emViewAnimator.rs` — definition of `SetGoalWithCoords`.
- Modify: `crates/emcore/src/emView.rs:3096` — call to `SetGoalWithCoords`.
- Modify: `crates/emmain/src/emMainWindow.rs:635, 709` — call sites of `SetGoalWithCoords`.

- [ ] **Step 1: Enumerate all occurrences**

Run:
```bash
grep -rn "VisitByIdentityShort\|SetGoalWithCoords" crates/ --include="*.rs"
```

Expected hits (from pre-plan grep):
- `crates/emcore/src/emView.rs:911` — call in `VisitPanel` body
- `crates/emcore/src/emView.rs:915` — `DIVERGED:` comment
- `crates/emcore/src/emView.rs:919` — `pub fn VisitByIdentityShort`
- `crates/emcore/src/emView.rs:3076` — comment
- `crates/emcore/src/emView.rs:3096` — call
- `crates/emcore/src/emView.rs:5658` — test comment
- `crates/emcore/src/emViewAnimator.rs:793` — `pub fn SetGoalWithCoords`
- `crates/emmain/src/emMainWindow.rs:635, 709` — call sites

If the actual output differs materially, STOP and re-enumerate.

- [ ] **Step 2: Rename `VisitByIdentityShort` → `VisitByIdentityBare`**

Use Edit with `replace_all: true` on `crates/emcore/src/emView.rs` to replace every occurrence of `VisitByIdentityShort` with `VisitByIdentityBare`.

- [ ] **Step 3: Update the `DIVERGED:` comment wording**

At the `DIVERGED:` block (was line 915, now potentially renumbered), find the text:
```
/// — Rust cannot overload by arity; renamed `VisitByIdentityShort` to disambiguate from
```
Replace with:
```
/// — Rust cannot overload by arity; renamed `VisitByIdentityBare` ("bare" = without
/// relX/relY/relA coords) to disambiguate from the 7-arg `VisitByIdentity`.
```

- [ ] **Step 4: Rename `SetGoalWithCoords` → `SetGoalCoords`**

Use Edit with `replace_all: true` on each of:
- `crates/emcore/src/emViewAnimator.rs`
- `crates/emcore/src/emView.rs`
- `crates/emmain/src/emMainWindow.rs`

replacing `SetGoalWithCoords` with `SetGoalCoords` in each.

- [ ] **Step 5: Update the `DIVERGED:` comment wording for `SetGoalCoords`**

At the `pub fn SetGoalCoords` definition site in `emViewAnimator.rs`, find the existing `DIVERGED:` block (it references `SetGoal(identity, relX, relY, relA, adherent, subject)`) and update any mention of "WithCoords" to "Coords".

- [ ] **Step 6: Verify no stale references remain**

Run:
```bash
grep -rn "VisitByIdentityShort\|SetGoalWithCoords" crates/ --include="*.rs" --include="*.md" --include="*.toml"
```

Expected: zero output from `crates/**/*.rs`. Matches in `.md` files (old plans, closeout notes, specs) are historical references and MUST NOT be rewritten (they document history).

- [ ] **Step 7: Build and test**

Run:
```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: clippy clean; 2425/2425 nextest. Pure rename — any regression is a missed call site.

- [ ] **Step 8: Commit**

```bash
git add crates/emcore/src/emView.rs crates/emcore/src/emViewAnimator.rs crates/emmain/src/emMainWindow.rs
git commit -m "$(cat <<'EOF'
refactor(emView): rename W4 arity-overload suffixes to semantic form

- VisitByIdentityShort → VisitByIdentityBare ("bare" = without rel coords)
- SetGoalWithCoords    → SetGoalCoords         (drop redundant "With")

The W4 suffixes were antonym heuristics. Uniform semantic suffixes make
the arity-overload family self-describing. External consumers are still
zero, so the rename is cheap now.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: `GeometrySignal` double-fire C++ reference comment

**Rationale.** The popup-teardown branch at `emView.rs:~1733` fires `GeometrySignal` a second time after `SwapViewPorts(true)` already fired it once. This matches C++ `emView.cpp:1678 + 1995` exactly, but the Rust code lacks an explanatory reference. One-line comment tying both fires to their C++ counterparts.

**Files:**
- Modify: `crates/emcore/src/emView.rs:~1733` — add one comment line.

- [ ] **Step 1: Locate the explicit GeometrySignal fire**

Run:
```bash
sed -n '1700,1760p' crates/emcore/src/emView.rs
```

Find the `self.CurrentViewPort.borrow_mut().RequestFocus();` line at ~1723. The explicit second `GeometrySignal` fire should be shortly after (within ~30 lines). If the file structure has changed and the explicit fire is in a different location, STOP and ask.

Actually, the spec says the double-fire happens because `SwapViewPorts(true)` fires once and then there's another fire. Both fires are in the popup-teardown branch. Find the second fire by grepping:

```bash
grep -n "GeometrySignal" crates/emcore/src/emView.rs | head -30
```

Identify the one in the popup-teardown branch near line 1733.

- [ ] **Step 2: Add the C++ reference comment**

Immediately above the second `GeometrySignal` fire line, add:

```rust
// C++ emView.cpp:1678 + 1995: SwapViewPorts(true) above fired GeometrySignal
// once; this second fire matches the C++ pair exactly. Do not collapse.
```

- [ ] **Step 3: Build and test**

Run:
```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: clippy clean; 2425/2425 nextest. Comment-only change.

- [ ] **Step 4: Commit**

```bash
git add crates/emcore/src/emView.rs
git commit -m "$(cat <<'EOF'
docs(emView): explain GeometrySignal double-fire in popup teardown

SwapViewPorts(true) fires GeometrySignal once; the explicit second fire
in the popup-teardown branch mirrors C++ emView.cpp:1678 + 1995. Add a
two-line comment so future readers don't collapse the pair.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Audit re-entrancy doc comments on `PaintView` / `InvalidatePainting`

**Rationale.** §5.1 item 1 flagged a need for re-entrancy doc comments. Discovered during plan-writing: detailed warnings already exist at `emViewPort.rs:164-174` and `~265-275`. This task verifies completeness and lightly polishes if needed.

**Files:**
- Possibly modify: `crates/emcore/src/emViewPort.rs` — touch-up doc comments.

- [ ] **Step 1: Read the existing comments**

Run:
```bash
sed -n '160,185p' crates/emcore/src/emViewPort.rs
sed -n '260,290p' crates/emcore/src/emViewPort.rs
```

Expected: both methods already have `**Re-entrancy warning:**` blocks mentioning `render`/`dispatch_input`/`handle_touch`.

- [ ] **Step 2: Verify each block covers the required points**

Each doc comment must state:
1. The upgrade pattern (`Weak<RefCell<emWindow>>` → `upgrade()` → `borrow()` or `borrow_mut()`).
2. The re-entrancy hazard — runtime `RefCell` panic, not compile-time check.
3. The three named problematic callers: `render`, `dispatch_input`, `handle_touch`.
4. A follow-up mandate — "full audit required when production call sites are first wired" (`PaintView`) or equivalent (`InvalidatePainting`).

`PaintView` at 164-174: check against this list. `InvalidatePainting` at ~265-275+: check against this list.

- [ ] **Step 3: Fill in any gap**

If either block is missing one of the four points above, edit to add it. Use the other block's wording as a template for consistency. If both blocks are already complete, this task requires no edit.

- [ ] **Step 4: Build and test (only if any edit was made)**

```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```

- [ ] **Step 5: Commit (skip if no change)**

If an edit was made:
```bash
git add crates/emcore/src/emViewPort.rs
git commit -m "$(cat <<'EOF'
docs(emViewPort): complete re-entrancy warning on PaintView/InvalidatePainting

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If no edit was needed, commit nothing; note in the task log that the audit found both blocks complete.

---

## Task 8: Minor cleanups batch (§5.2 items)

**Rationale.** Three one-liner reviewer nits. (The fourth cleanup — `popup_window.rs` dead gate — was dropped during plan-writing after the file was found already clean. The fifth — suffix rename — is Task 5.)

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs:48`
- Modify: `crates/emcore/src/emGUIFramework.rs:~393` (the `let mut win = ...; let win = &mut *win;` site) and `emGUIFramework.rs:~155` or `~395` (the `dispatch_forward_events` doc comment)

- [ ] **Step 1: `emSubViewPanel.rs:48` — comment the literal 1.0 pixel tallness**

Read:
```bash
sed -n '44,52p' crates/emcore/src/emSubViewPanel.rs
```

Find the line:
```rust
        sub_tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);
```

The existing comment on line 48 (`// Last arg is pixel tallness; sub_view.CurrentPixelTallness starts at 1.0.`) already ties the literal to `CurrentPixelTallness`. This cleanup is already present. Verify and skip if so. If the comment is missing or weaker, add/strengthen it.

- [ ] **Step 2: `emGUIFramework.rs:~393` — collapse the Touch branch borrow pattern**

Read:
```bash
sed -n '390,400p' crates/emcore/src/emGUIFramework.rs
```

Current code:
```rust
                if let Some(rc) = self.windows.get(&window_id) {
                    let mut win = rc.borrow_mut();
                    win.handle_touch(touch, &mut self.tree);
                    Self::dispatch_forward_events(&mut win, &mut self.tree, &mut self.input_state);
                    win.invalidate();
                    win.request_redraw();
                }
```

This *cannot* be collapsed to `&mut *rc.borrow_mut()` because `win` is used four times. The reviewer's suggestion applies to a different borrow site. Search for the actual pattern:

```bash
grep -nB1 -A2 "let mut win = rc.borrow_mut" crates/emcore/src/emGUIFramework.rs
grep -nB1 -A2 "let win = &mut \*win" crates/emcore/src/emGUIFramework.rs
```

If no site actually contains the pattern `let mut win = rc.borrow_mut(); let win = &mut *win;` (the reviewer's pattern), this cleanup is stale — note and skip.

If the pattern does exist somewhere, replace it with a single `let mut win = rc.borrow_mut();` if `win` is used multiple times, or with `let win = &mut *rc.borrow_mut();` only if used once.

- [ ] **Step 3: `emGUIFramework.rs::dispatch_forward_events` — fix doc comment placement**

Read the function definition and its call site:
```bash
sed -n '150,170p' crates/emcore/src/emGUIFramework.rs
sed -n '390,400p' crates/emcore/src/emGUIFramework.rs
```

The reviewer item says: the function-site doc comment describes caller-side usage, not the function itself. Read it and decide:
- If the doc comment at the function definition (~line 155) describes what *callers* should do rather than what the function does, rewrite it to describe the function (what it takes, what it does, what it returns). If useful caller-side context exists, move *that* portion to a comment at the single call site (~line 395).
- If the doc comment already describes the function, this cleanup is stale — note and skip.

- [ ] **Step 4: Build and test (only if any edit was made)**

```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```

- [ ] **Step 5: Commit (skip if no change)**

If edits were made:
```bash
git add crates/emcore/src/emSubViewPanel.rs crates/emcore/src/emGUIFramework.rs
git commit -m "$(cat <<'EOF'
style: reviewer cleanups batch from emView closeout §5.2

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If no edits were needed, note in the task log and skip the commit.

---

## Final acceptance

- [ ] **Step 1: Marker audit**

Run:
```bash
grep -rn "PHASE-6-FOLLOWUP" crates/
```
Expected: zero output.

Run:
```bash
grep -rn "PHASE-W4-FOLLOWUP" crates/
```
Expected: exactly 3 matches (`emView.rs:897, 921, 3080`). Unchanged from baseline — these belong to sub-project 3, out of scope for this plan.

- [ ] **Step 2: Full test run**

```bash
cargo clippy -- -D warnings
cargo-nextest ntr
cargo test --test golden -- --test-threads=1
```
Expected:
- clippy: clean
- nextest: 2425 passed, 9 skipped, 0 failed
- golden: 237 passed, 6 failed (same six pre-existing failures: `composition_tktest_{1x,2x}`, `notice_window_resize`, `testpanel_{expanded,root}`, `widget_file_selection_box`)

- [ ] **Step 3: Smoke**

```bash
timeout 20 cargo run --release --bin eaglemode
echo "exit: $?"
```
Expected: exit code 143 (SIGTERM) or 124 (SIGKILL). Either means the program stayed alive for the full 20 seconds.

- [ ] **Step 4: Symbol-visibility audit**

```bash
cargo build --release 2>&1 | grep -c pump_visiting_va
```
Expected: 0.

- [ ] **Step 5: Confirm commit count**

```bash
git log --oneline main..HEAD
```
Expected: 3 to 8 commits (one per task; tasks 7 and 8 may produce zero commits if the cleanups were already done).

- [ ] **Step 6: Close the plan**

The bundle is done. Remaining emView closeout residuals (items 2, 10, 11, 12, 13, 14 from §8) are separate sub-projects with their own specs.
