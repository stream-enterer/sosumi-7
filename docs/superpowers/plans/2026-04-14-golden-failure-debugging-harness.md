# Golden Test Failure Debugging Harness

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the 5 remaining golden test failures at zero tolerance.

**Method:** Tools narrow scope. `diff_draw_ops` names the divergent ops. `git_audit --risk` names the unverified functions. You rewrite unverified functions that share a name with a divergent op, then work outward in risk order. One function at a time. The action is always: delete Rust body, read C++, rewrite from C++.

---

## MANDATORY: The Only Action

**The response to every finding is the same: delete the Rust and rewrite from C++.**

- diff_draw_ops names a divergent op → the Rust function with that name is the rewrite target
- git_audit says function is unverified and it shares a name with a divergent op → delete body, rewrite from C++
- After rewriting named ops, test still fails → rewrite the next highest-risk unverified function in the same file
- You don't understand why it's wrong → doesn't matter, delete body, rewrite from C++
- The Rust "looks correct" → doesn't matter, delete body, rewrite from C++
- You think only a small part is wrong → doesn't matter, delete the ENTIRE body, rewrite ALL of it from C++

**You are not debugging. You are not diagnosing. You are not interpreting. You are identifying unverified functions and replacing them with C++ ports.**

---

## MANDATORY: Progress Model

**A C2-rewrite is correct by definition.** It matches C++. It is NEVER reverted. Not if the test still fails. Not if metrics get worse. Not if other tests break. A function that matches C++ is correct. Period.

**The test may not pass until MULTIPLE functions are rewritten.** A single divergent test may require 2, 3, or more functions to all be rewritten before it passes. This is normal. Rewriting function A makes A correct but the test still fails because function B is still wrong. This does NOT mean A's rewrite was wrong or unhelpful.

**NEVER revert a C2-rewrite.** There is no circumstance where you undo a rewrite that matches C++. If the test gets worse, it's because compensating errors were removed — two wrongs were making a partial right. The rewrite exposed the real remaining bugs. Keep it and continue.

**Do NOT check golden test results or divergence metrics between individual rewrites.** These metrics are meaningless mid-batch. They may improve, worsen, or stay the same, and NONE of those outcomes tell you whether the rewrite was correct. The rewrite is correct because it matches C++.

**Per-rewrite gate:** Compiles + clippy clean + non-golden tests pass. That's it. Commit and move to the next function.

**Batch gate:** After ALL identified functions for a task are rewritten, run the golden tests. If the target test passes, done. If not, regenerate Rust ops, re-run diff_draw_ops to see the new divergent ops, identify new targets, and repeat.

**Workflow for each investigation task:**
1. Generate fresh Rust ops. Run diff_draw_ops to get divergent op names.
2. Run git_audit to get unverified function list with risk scores.
3. Build the rewrite list (see Targeting Rules below).
4. Rewrite all functions on the list, one at a time (Task 6), committing after each.
5. After the list is exhausted: run ALL 5 golden tests. If target passes, done. If not, go to step 1 (ops may have shifted).

---

## MANDATORY: Architectural Fidelity Rules

1. **Every divergence from C++ is a bug.** No exceptions. No "the Rust way is better." No "this is idiomatic." No "this divergence is intentional." No "out of scope."

2. **When you find divergent Rust code, DELETE IT.** Do not refactor. Do not adapt. Do not preserve. Delete the body. Read C++. Write new Rust matching C++.

3. **Delete Rust-only abstractions, helpers, wrappers** that don't exist in C++. If C++ inlines it, Rust inlines it.

4. **No compatibility shims, _old renames, "temporary" bridges.** Delete completely.

5. **Audit callers and callees too** — through git_audit, not code reading. If git_audit shows a caller is also unverified, add it to the rewrite list.

---

## Antipattern Guards

If you catch yourself doing ANY of these, STOP IMMEDIATELY:

1. **Reading Rust source to understand what's wrong** — you don't need to understand. Delete it. Rewrite from C++.
2. **Saying "I think the problem is X"** — you are not allowed to think about problems. Delete and rewrite.
3. **Saying "this divergence is intentional / out of scope"** — it is not. Delete and rewrite.
4. **Saying "this function looks correct"** — it produced wrong output. Delete and rewrite.
5. **Trying a targeted fix instead of full rewrite** — no. Delete the ENTIRE body. Rewrite ALL of it.
6. **Analyzing pixel patterns to determine bug type** — no. Find the unverified function. Delete and rewrite.
7. **Lumping multiple functions into one fix** — no. One function at a time.
8. **Running golden tests between individual rewrites** — no. Commit and move to the next function.
9. **Reverting a C2-rewrite for any reason** — no. It matches C++. It is correct.

---

## Definitions

**Verified (C2):** A function whose most recent commit message matches `git_audit.py`'s `C2_PATTERNS` regex — meaning it was deliberately rewritten to match C++ source. `git_audit.py` reports these as `[C2]`.

**Unverified (PORT):** A function whose most recent commit does NOT match `C2_PATTERNS` — meaning it was written during the original port and has never been verified line-by-line against C++. `git_audit.py` reports these as `[PORT]`. These are the rewrite candidates.

**Risk score:** `git_audit.py --risk` ranks unverified functions by `unverified_lines × age_in_days`. Higher score = more lines that have gone longer without C2 verification. Functions already marked C2 (>80% C2 lines) or matching infrastructure patterns (`new`, `default`, `fmt`, etc.) get risk score 0.

**Divergent op:** An op (paint call) where `diff_draw_ops.py` reports parameter differences between C++ and Rust. The op name (e.g., `PaintRect`, `PaintPolygon`) directly corresponds to a Rust function with the same name.

---

## Targeting Rules

How to build the rewrite list for a task WITHOUT reading Rust source:

1. **Direct name match (first priority):** diff_draw_ops names the divergent ops (e.g., `PaintRect`, `PaintPolygon`, `PaintTextBoxed`). These are Rust function names. If git_audit shows any of these functions as `[PORT]`, they go on the rewrite list.

2. **Same-file risk order (second priority):** If all directly-named functions are already `[C2]`, the issue is in a function they call internally. You do NOT need to know which one. Run `git_audit --risk` on the same file. The highest-risk unverified function in the file is the next target. Add it to the list.

3. **Expand to related files (third priority):** If all functions in the primary file are `[C2]` and the test still fails, expand to related files. For painter ops: `emPainterScanline.rs`, `emPainterScanlineTool.rs`, `emPainterInterpolation.rs`. For widget ops: `emBorder.rs`, `emLook.rs`. Run git_audit on each and take the highest-risk unverified functions.

**This ordering means you never need to read Rust code to decide what to rewrite.** Op names give you the first targets. Risk scores give you the rest.

---

## Preconditions

Run these after Task 0 Step 0.1 to verify the environment is set up:

```bash
# C++ ops files exist for 4 of 5 tests (eagle_logo gets ops in Task 1)
ls crates/eaglemode/target/golden-divergence/{tktest_1x,tktest_2x,testpanel_root,testpanel_expanded}.cpp_ops.jsonl

# gen_golden binary is current
make -C crates/eaglemode/tests/golden/gen
```

---

### Task 0: Generate Baseline Evidence

- [ ] **Step 0.1: Rebuild gen_golden and regenerate C++ data**

```bash
scripts/verify_golden.sh --regen
```

**IMPORTANT:** Always rebuild gen_golden before using C++ ops files. A stale binary silently produces wrong ops.

- [ ] **Step 0.2: Generate fresh Rust ops and debug images for all 5 tests**

```bash
DUMP_DRAW_OPS=1 DUMP_GOLDEN=1 cargo nextest run -j1 -E 'test(eagle_logo) + test(composition_tktest_1x) + test(composition_tktest_2x) + test(testpanel_root) + test(testpanel_expanded)' 2>&1 | tail -15
```

**Verify:**

```bash
ls -la crates/eaglemode/target/golden-divergence/{eagle_logo,tktest_1x,tktest_2x,testpanel_root,testpanel_expanded}.rust_ops.jsonl
ls -la crates/eaglemode/target/golden-debug/{actual,expected,diff}_{eagle_logo,testpanel_root,tktest_1x}.ppm
```

- [ ] **Step 0.3: Record baseline pass/fail counts**

```bash
cargo nextest run -E 'binary(golden)' -j1 2>&1 | tail -10
```

Record: X passed, Y failed. This is for reference only — do not use these numbers as per-rewrite gates.

---

### Task 1: eagle_logo

- [ ] **Step 1.1: Add C++ ops logging for eagle_logo**

eagle_logo has no C++ ops file. Add `open_draw_op_log("eagle_logo")` / `close_draw_op_log()` around painting in `gen_eagle_logo()` in gen_golden.cpp. Pattern from `gen_bezier_stroked()` (line ~401):

```cpp
open_draw_op_log("painter_bezier_stroked");
p.PaintBezierLine(...);
close_draw_op_log();
```

Rebuild and regenerate:

```bash
make -C crates/eaglemode/tests/golden/gen clean && make -C crates/eaglemode/tests/golden/gen
make -C crates/eaglemode/tests/golden/gen run
ls -la crates/eaglemode/target/golden-divergence/eagle_logo.cpp_ops.jsonl
```

- [ ] **Step 1.2: Diff ops**

```bash
python3 scripts/diff_draw_ops.py eagle_logo --summary-json
```

`diff_draw_ops.py` exits code 1 on divergence — this is normal. Read the output for divergent op names.

- [ ] **Step 1.3: Git audit**

```bash
python3 scripts/git_audit.py crates/emcore/src/emPainter.rs --risk | head -30
python3 scripts/git_audit.py crates/emcore/src/emPainterScanline.rs --risk | head -30
python3 scripts/git_audit.py crates/emcore/src/emPainterScanlineTool.rs --risk | head -30
python3 scripts/git_audit.py crates/emcore/src/emPainterInterpolation.rs --risk | head -30
```

- [ ] **Step 1.4: Build rewrite list and execute**

Apply Targeting Rules: direct name matches first, then same-file risk order.

Rewrite all functions on the list using Task 6, one at a time, committing after each.

- [ ] **Step 1.5: Fix the test setup**

Change `img.fill(emColor::BLACK)` to `img.fill(emColor::WHITE)` in `crates/eaglemode/tests/golden/eagle_logo.rs` to match C++ gen. Commit separately.

- [ ] **Step 1.6: Batch gate — run golden tests**

```bash
DUMP_DRAW_OPS=1 cargo nextest run -j1 -E 'test(eagle_logo) + test(composition_tktest_1x) + test(composition_tktest_2x) + test(testpanel_root) + test(testpanel_expanded)' 2>&1 | tail -15
```

Note which tests passed. If eagle_logo still fails: re-run `diff_draw_ops.py eagle_logo --summary-json` (Rust ops were regenerated above). The divergent ops may have shifted — new unverified functions to target. Go back to Step 1.4.

If any OTHER tests also passed: note them, skip their investigation tasks later.

---

### Task 2: testpanel_root

**Skip if testpanel_root already passed in Task 1's batch gate.**

- [ ] **Step 2.1: Generate fresh Rust ops and diff**

Rust ops from Task 0 are stale (Task 1 rewrote shared code). Regenerate:

```bash
DUMP_DRAW_OPS=1 cargo nextest run -j1 -E 'test(testpanel_root)' 2>&1 | tail -5
python3 scripts/diff_draw_ops.py testpanel_root --summary-json
```

- [ ] **Step 2.2: Git audit**

```bash
python3 scripts/git_audit.py crates/emcore/src/emPainter.rs --risk | head -30
python3 scripts/git_audit.py crates/emcore/src/emBorder.rs --risk | head -30
python3 scripts/git_audit.py crates/emcore/src/emLook.rs --risk | head -30
python3 scripts/git_audit.py crates/emcore/src/emPainterScanline.rs --risk | head -30
```

- [ ] **Step 2.3: Build rewrite list and execute**

Apply Targeting Rules. Skip any function already rewritten in Task 1.

Rewrite all functions on the list using Task 6, one at a time.

- [ ] **Step 2.4: Batch gate — run golden tests**

```bash
DUMP_DRAW_OPS=1 cargo nextest run -j1 -E 'test(testpanel_root) + test(testpanel_expanded) + test(composition_tktest_1x) + test(composition_tktest_2x)' 2>&1 | tail -15
```

If testpanel_root still fails: re-run diff_draw_ops, identify new targets, go back to Step 2.3.

Note any other tests that passed — skip their investigation tasks.

---

### Task 3: composition_tktest_1x

**Skip if tktest_1x already passed in a prior batch gate.**

- [ ] **Step 3.1: Generate fresh Rust ops and diff**

```bash
DUMP_DRAW_OPS=1 cargo nextest run -j1 -E 'test(composition_tktest_1x)' 2>&1 | tail -5
python3 scripts/diff_draw_ops.py tktest_1x --summary-json
```

- [ ] **Step 3.2: Git audit**

Same files as Task 2 plus widget files: `emButton.rs`, `emCheckButton.rs`, `emRadioButton.rs`, `emTextField.rs`, `emScalarField.rs`, `emColorField.rs`, `emListBox.rs`, `emLabel.rs`, `emSplitter.rs`, `emTunnel.rs`.

- [ ] **Step 3.3: Build rewrite list and execute**

Apply Targeting Rules. Skip functions already rewritten in Tasks 1-2.

- [ ] **Step 3.4: Batch gate — run golden tests**

```bash
DUMP_DRAW_OPS=1 cargo nextest run -j1 -E 'test(composition_tktest_1x) + test(composition_tktest_2x)' 2>&1 | tail -15
```

---

### Task 4: composition_tktest_2x

**Skip if tktest_2x already passed in a prior batch gate.**

- [ ] **Step 4.1: Generate fresh Rust ops, diff, git audit, rewrite, batch gate**

Same process as Tasks 2-3. Skip functions already rewritten.

---

### Task 5: testpanel_expanded

**Skip if testpanel_expanded already passed in a prior batch gate.**

- [ ] **Step 5.1: Generate fresh Rust ops, diff, git audit, rewrite, batch gate**

Same process as Tasks 2-3. Skip functions already rewritten.

---

### Task 6: C2-Rewrite (repeat per function)

**ONE function at a time. This section defines exactly what a "C2-rewrite" is. Follow it mechanically.**

#### What a C2-rewrite IS

A C2-rewrite is a mechanical replacement. You do NOT read the Rust to understand it. You do NOT compare Rust to C++ to find differences. You do NOT make targeted edits. You throw away the Rust and write new Rust by translating C++ line by line.

#### What a C2-rewrite is NOT

- It is NOT "fix the bug I found by reading the code"
- It is NOT "adjust this line to match C++"
- It is NOT "the Rust looks mostly right, just change X"
- It is NOT "refactor to match the C++ structure"
- It is NOT reading the Rust first and then deciding what to change

**If you read the Rust body at any point during a C2-rewrite, you have failed.** The Rust body is dead. You do not read dead code. You delete it.

#### Procedure

- [ ] **Step 6.1: Open the C++ source file**

The C++ file is at `~/git/eaglemode-0.96.4/src/emCore/<FileName>.cpp` (or `.h` for inline methods). The function has the same name as the Rust function (per File and Name Correspondence).

Read the C++ function from first line to last line. Read every line. Read functions it calls. Read macros it uses. Do not skim. Do not skip. You must understand the C++ completely before writing any Rust.

**Do NOT open or read the Rust file yet.**

- [ ] **Step 6.2: Delete the Rust function body**

NOW open the Rust file. Select the entire function body — everything between the opening `{` and closing `}`. Delete it all. Every line. Leave only:

```rust
pub(crate) fn FunctionName(&mut self, args...) -> ReturnType {
}
```

An empty body. If the function has local helper functions or closures that exist only inside this function, they are deleted too.

**Do NOT read the Rust body before deleting it. Do NOT look at it. Do NOT "check what it does." Delete first, then proceed.**

If you feel the urge to "just glance at the Rust to see how it handles X" — that is the antipattern. The answer to every question about the Rust is: it doesn't matter, it's deleted.

- [ ] **Step 6.3: Write new Rust from the C++ you read in Step 6.1**

Translate the C++ line by line into Rust. Your only reference is the C++ source from Step 6.1. Rules:

- Same variable names as C++ (adjust for snake_case only where Rust requires it)
- Same operation order as C++
- Same formulas — if C++ uses `(x * 257 + 0x8073) >> 16`, write that, not a "simplified" version
- Same control flow — if C++ uses a for loop, use a for loop. If C++ uses early return, use early return. If C++ uses goto, restructure minimally with loop/break.
- Same function calls — if C++ calls `GetRed()`, Rust calls `GetRed()`. If C++ calls a helper, find/write the Rust equivalent of that exact helper.
- No Rust-only helpers, abstractions, or wrappers that don't exist in C++
- Integer arithmetic in blend/coverage/interpolation paths — no f64 approximations
- If you don't know how to express a C++ construct in Rust, look at how other C2-verified functions in the same file handle it. Do NOT invent a new pattern.

- [ ] **Step 6.4: Compile**

```bash
cargo check 2>&1 | head -40
```

Fix compilation errors by re-reading the C++, not by reading the old Rust (which is deleted). Common issues:
- Type mismatches: cast to match C++ types (`as i32`, `as u32`, etc.)
- Borrow checker: add `&`, `&mut`, `.clone()` as needed
- Missing imports: add `use` statements

**If the logic doesn't compile and you can't fix it from the C++ alone**, the surrounding Rust code (callers, types, API) diverges from C++. That surrounding code also needs C2-rewrite. Add it to the rewrite list. Do NOT hack the new function to fit the wrong surrounding code.

- [ ] **Step 6.5: Clippy**

```bash
cargo clippy -- -D warnings 2>&1 | head -40
```

Fix warnings. Do NOT use `#[allow(...)]` or `#[expect(...)]`.

- [ ] **Step 6.6: Run non-golden tests only**

```bash
cargo nextest run -j1 -E 'not binary(golden)' 2>&1 | tail -10
```

This verifies the rewrite doesn't break unit tests, integration tests, or kani proofs. If non-golden tests fail: fix the callers to match the new (correct) API. Do NOT revert the rewrite.

**Do NOT run golden tests here.** Golden test results are meaningless mid-batch.

- [ ] **Step 6.7: Commit**

```bash
git add <specific files>
git commit -m "feat(C2): rewrite <function_name> — match C++ <file>:<lines>"
```

- [ ] **Step 6.8: Next function**

Return to the investigation task's rewrite list. Pick the next function. Repeat Task 6 from Step 6.1.

Continue until the rewrite list is exhausted. Then return to the investigation task's batch gate step.

---

### Task 7: Final Verification

- [ ] **Step 7.1:** `cargo nextest run -E 'binary(golden)' -j1` — 243/243 pass
- [ ] **Step 7.2:** `cargo-nextest ntr` — all tests pass
- [ ] **Step 7.3:** `cargo clippy -- -D warnings` — clean
- [ ] **Step 7.4:** `scripts/strict_baseline.sh` — clean
- [ ] **Step 7.5:** Update `golden_test_status.md`

---

## Ordering

1. **Task 0** — generate baseline evidence
2. **Task 1** (eagle_logo) — isolated painter, simplest pipeline
3. **Task 2** (testpanel_root) — before expanded (strict subset)
4. **Task 5** (testpanel_expanded) — skip if passed in prior batch gate
5. **Task 3** (tktest_1x) — before 2x
6. **Task 4** (tktest_2x) — skip if passed in prior batch gate
7. **Task 7** — final verification

Each investigation task has a batch gate that runs ALL remaining failing tests. Tests that pass in an earlier batch gate are skipped in later tasks.

## Cross-Cutting Concerns

Shared code (e.g., `emPainter` blend functions, `emBorder` rendering) appears on the divergent op path of MULTIPLE failing tests. Rules:

**Rewrite shared functions ONCE.** If PaintRect is on the rewrite list for eagle_logo, rewrite it during Task 1. It stays rewritten for all subsequent tasks. The batch gate at the end of Task 1 runs all 5 tests, so you immediately see which other tests benefited.

**If a rewrite breaks a previously-passing test**, the rewrite is correct — it matches C++. The previously-passing test was passing due to compensating errors. The newly-broken test now needs its own unverified functions identified and rewritten. Add it back to the task list.

**If a rewrite makes a different FAILING test worse**, the same logic applies. The rewrite is correct. Continue with the plan.

**Do NOT revert a correct C2-rewrite for any reason.** A C2-rewrite that matches C++ is always correct.

**Do NOT defer a rewrite** because "it might affect other tests." Rewrite it. Run the batch gate. Deal with the results.

---

## When ALL Identified Functions Are Rewritten and the Test Still Fails

After exhausting the rewrite list for a task and the test still fails:

1. **Regenerate Rust ops** — the divergent ops have changed because you rewrote functions. `DUMP_DRAW_OPS=1 cargo nextest run -j1 -E 'test(<name>)'`
2. **Re-run diff_draw_ops** — see the NEW divergent ops.
3. **Re-run git_audit** — some functions are now C2. The risk rankings have changed.
4. **Build a new rewrite list** from the new tool output. Apply Targeting Rules.
5. **Repeat** until the test passes or all functions in all implicated files are C2-verified.

If ALL functions are C2-verified and the test still fails: the test setup is wrong, or a C2-rewrite has a subtle C++ misread. Re-read the C++ for the most recently rewritten functions. Compare your Rust line-by-line against C++. Do NOT read the old (deleted) Rust.

If 3+ full passes through this loop don't help: the divergence is architectural. The Rust file's structure (type layout, field names, composition pattern) diverges from C++. Compare the Rust file's struct/type definitions against C++ holistically and restructure to match.
