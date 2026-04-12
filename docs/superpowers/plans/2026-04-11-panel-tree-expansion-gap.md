# Panel Tree Expansion Gap — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the ~3148 missing ops gap between Rust (2322 ops) and C++ (5470 ops) in composition_tktest_1x by making the Rust panel tree match C++.

**Architecture:** Dump both panel trees after settle (200 cycles), diff mechanically to identify every missing panel, then fix the expansion/settle/delegation code so Rust produces the same tree. The renderer is correct — this is purely a tree structure problem.

**Tech Stack:** Rust (emcore/eaglemode crates), C++ gen_golden.cpp, JSONL panel tree dumps, Python diff script.

---

## Phase 1: Instrument panel tree dumps

**Gate:** Both C++ and Rust produce JSONL dumps of the full panel tree after settle. Every panel has: path, depth, layout rect, child_count, ae_expanded, viewed, ae_threshold_value.

### Task 1.1: Add C++ panel tree dump to gen_golden.cpp

**Files:**
- Modify: `crates/eaglemode/tests/golden/gen/gen_golden.cpp`

- [ ] **Step 1: Add recursive panel tree dump function**

Add after the existing `dump_layout` function (line ~682):

```cpp
// Recursive panel tree dump — writes one JSONL line per panel.
static void dump_panel_tree_recursive(FILE* f, emPanel* panel, int depth) {
    fprintf(f,
        "{\"path\":\"%s\",\"depth\":%d,"
        "\"lx\":%.17g,\"ly\":%.17g,\"lw\":%.17g,\"lh\":%.17g,"
        "\"children\":%d,\"ae_expanded\":%d,\"viewed\":%d,"
        "\"ae_thresh\":%.17g}\n",
        panel->GetIdentity().Get(),
        depth,
        panel->GetLayoutX(), panel->GetLayoutY(),
        panel->GetLayoutWidth(), panel->GetLayoutHeight(),
        (int)[&]{int n=0; for(auto*c=panel->GetFirstChild();c;c=c->GetNext())n++; return n;}(),
        panel->IsAutoExpanded() ? 1 : 0,
        panel->IsViewed() ? 1 : 0,
        panel->GetAutoExpansionThresholdValue()
    );
    for (emPanel* c = panel->GetFirstChild(); c; c = c->GetNext()) {
        dump_panel_tree_recursive(f, c, depth + 1);
    }
}

static void dump_panel_tree(const char* name, emPanel* root) {
    char path[512];
    snprintf(path, sizeof(path), "%s/%s.cpp_tree.jsonl",
             getenv("GOLDEN_DIVERGENCE_DIR") ? getenv("GOLDEN_DIVERGENCE_DIR")
                                              : "target/golden-divergence",
             name);
    FILE* f = fopen(path, "w");
    dump_panel_tree_recursive(f, root, 0);
    fclose(f);
    printf("  tree/%s\n", name);
}
```

- [ ] **Step 2: Call dump_panel_tree in gen_tktest_1x**

In `gen_tktest_1x()` (line ~4306), after the scheduler run and before render_and_dump_sized, add:

```cpp
    dump_panel_tree("tktest_1x", tk);
```

- [ ] **Step 3: Build and run the C++ generator**

Run:
```bash
make -C crates/eaglemode/tests/golden/gen && make -C crates/eaglemode/tests/golden/gen run
```

Expected: `target/golden-divergence/tktest_1x.cpp_tree.jsonl` is created with one JSONL line per panel. Verify with:
```bash
wc -l target/golden-divergence/tktest_1x.cpp_tree.jsonl
head -5 target/golden-divergence/tktest_1x.cpp_tree.jsonl
```

Expected: >100 lines (one per panel in the C++ tree).

- [ ] **Step 4: Commit**

```bash
git add crates/eaglemode/tests/golden/gen/gen_golden.cpp
git commit -m "feat: add panel tree dump to C++ golden generator for tktest_1x"
```

### Task 1.2: Add Rust panel tree dump to composition test

**Files:**
- Modify: `crates/eaglemode/tests/golden/composition.rs`

- [ ] **Step 1: Add panel tree dump function**

Add after the `settle` function (line ~70):

```rust
/// Dump the full panel tree as JSONL — one line per panel.
/// Controlled by DUMP_PANEL_TREE=1 env var.
fn dump_panel_tree_enabled() -> bool {
    std::env::var("DUMP_PANEL_TREE").map_or(false, |v| v == "1")
}

fn dump_panel_tree(name: &str, tree: &PanelTree, root: PanelId) {
    let dir = std::env::var("GOLDEN_DIVERGENCE_DIR")
        .unwrap_or_else(|_| "target/golden-divergence".to_string());
    std::fs::create_dir_all(&dir).unwrap();
    let path = format!("{dir}/{name}.rust_tree.jsonl");
    let mut lines = Vec::new();
    dump_panel_recursive(tree, root, 0, &mut lines);
    std::fs::write(&path, lines.join("")).unwrap();
    eprintln!("  tree/{name} ({} panels)", lines.len());
}

fn dump_panel_recursive(
    tree: &PanelTree,
    id: PanelId,
    depth: usize,
    out: &mut Vec<String>,
) {
    let panel = tree.GetRec(id).unwrap();
    let name = tree.name(id).unwrap_or("?");
    let path = tree.GetIdentity(id);
    let lr = panel.layout_rect;
    let child_count = tree.child_count(id);
    out.push(format!(
        "{{\"path\":\"{path}\",\"depth\":{depth},\
         \"lx\":{:.17},\"ly\":{:.17},\"lw\":{:.17},\"lh\":{:.17},\
         \"children\":{child_count},\"ae_expanded\":{},\"viewed\":{},\
         \"ae_thresh\":{:.17}}}\n",
        lr.x, lr.y, lr.w, lr.h,
        panel.ae_expanded as u8,
        panel.viewed as u8,
        panel.ae_threshold_value,
    ));
    for child in tree.children(id) {
        dump_panel_recursive(tree, child, depth + 1, out);
    }
}
```

Note: This requires `PanelTree::GetRec` (public read access to PanelData) and `PanelTree::GetIdentity` (panel path string). Check that these exist; if `GetIdentity` doesn't exist, build the path by walking parents:

```rust
fn panel_identity(tree: &PanelTree, id: PanelId) -> String {
    let mut parts = Vec::new();
    let mut cur = Some(id);
    while let Some(c) = cur {
        parts.push(tree.name(c).unwrap_or("?").to_string());
        cur = tree.parent(c);
    }
    parts.reverse();
    parts.join(":")
}
```

- [ ] **Step 2: Add dump call to composition_tktest_1x**

After `settle(&mut tree, &mut view, 200)` (line ~843), add:

```rust
    if dump_panel_tree_enabled() {
        dump_panel_tree("tktest_1x", &tree, root);
    }
```

- [ ] **Step 3: Run and verify**

```bash
DUMP_PANEL_TREE=1 cargo test --test golden composition_tktest_1x -- --test-threads=1 2>&1 | head -5
wc -l crates/eaglemode/target/golden-divergence/tktest_1x.rust_tree.jsonl
head -5 crates/eaglemode/target/golden-divergence/tktest_1x.rust_tree.jsonl
```

Expected: JSONL file created with one line per panel.

- [ ] **Step 4: Run `cargo clippy -- -D warnings`**

Expected: PASS (no warnings).

- [ ] **Step 5: Commit**

```bash
git add crates/eaglemode/tests/golden/composition.rs
git commit -m "feat: add panel tree dump to Rust composition test for tktest_1x"
```

### Task 1.3: Write diff script

**Files:**
- Create: `scripts/diff_panel_tree.py`

- [ ] **Step 1: Write the diff script**

```python
#!/usr/bin/env python3
"""Diff C++ vs Rust panel trees from JSONL dumps.

Usage: python3 scripts/diff_panel_tree.py <test_name>
  Reads: target/golden-divergence/<test_name>.cpp_tree.jsonl
         target/golden-divergence/<test_name>.rust_tree.jsonl
"""
import json, sys

def load_tree(path):
    panels = {}
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or not line.startswith('{'):
                continue
            p = json.loads(line)
            panels[p['path']] = p
    return panels

def main():
    name = sys.argv[1] if len(sys.argv) > 1 else "tktest_1x"
    base = "crates/eaglemode/target/golden-divergence"
    cpp = load_tree(f"{base}/{name}.cpp_tree.jsonl")
    rust = load_tree(f"{base}/{name}.rust_tree.jsonl")

    cpp_paths = set(cpp.keys())
    rust_paths = set(rust.keys())

    missing_in_rust = sorted(cpp_paths - rust_paths)
    extra_in_rust = sorted(rust_paths - cpp_paths)
    common = sorted(cpp_paths & rust_paths)

    print(f"C++ panels: {len(cpp)}, Rust panels: {len(rust)}")
    print(f"Common: {len(common)}, Missing in Rust: {len(missing_in_rust)}, Extra in Rust: {len(extra_in_rust)}")

    if missing_in_rust:
        print(f"\n=== MISSING IN RUST ({len(missing_in_rust)}) ===")
        # Group by parent path
        by_parent = {}
        for path in missing_in_rust:
            parts = path.rsplit(':', 1)
            parent = parts[0] if len(parts) > 1 else "<root>"
            by_parent.setdefault(parent, []).append(path)
        for parent in sorted(by_parent):
            print(f"\n  Under {parent}:")
            for p in by_parent[parent]:
                c = cpp[p]
                print(f"    {p}  depth={c['depth']} children={c['children']} ae={c['ae_expanded']} viewed={c['viewed']}")

    if extra_in_rust:
        print(f"\n=== EXTRA IN RUST ({len(extra_in_rust)}) ===")
        for p in extra_in_rust:
            r = rust[p]
            print(f"  {p}  depth={r['depth']} children={r['children']}")

    if common:
        diffs = []
        for path in common:
            c, r = cpp[path], rust[path]
            dd = []
            if c['children'] != r['children']:
                dd.append(f"children: C++={c['children']} Rust={r['children']}")
            if c['ae_expanded'] != r['ae_expanded']:
                dd.append(f"ae_expanded: C++={c['ae_expanded']} Rust={r['ae_expanded']}")
            if c['viewed'] != r['viewed']:
                dd.append(f"viewed: C++={c['viewed']} Rust={r['viewed']}")
            if dd:
                diffs.append((path, dd))
        if diffs:
            print(f"\n=== COMMON PANELS WITH DIFFERENCES ({len(diffs)}) ===")
            for path, dd in diffs:
                print(f"  {path}")
                for d in dd:
                    print(f"    {d}")

if __name__ == '__main__':
    main()
```

- [ ] **Step 2: Run the diff**

```bash
python3 scripts/diff_panel_tree.py tktest_1x
```

Expected: Shows panels missing in Rust grouped by parent. This is the ground truth for Phase 2.

- [ ] **Step 3: Commit**

```bash
git add scripts/diff_panel_tree.py
git commit -m "feat: add panel tree diff script for C++ vs Rust comparison"
```

## Phase 1 Gate

**STOP.** Before proceeding to Phase 2, verify:
1. Both JSONL files exist and contain data
2. The diff script runs and produces output
3. The diff shows which panels are missing in Rust

**Report the diff output.** The output determines what Phase 2 tasks are needed. If C++ and Rust trees are identical, the problem is in rendering not expansion — stop and re-evaluate.

Expected: Many panels missing in Rust, grouped by parent. The missing panels reveal which widget types need expansion delegation or which expansion paths are broken.

---

## Phase 2: Diagnose the gap

**Gate:** Root cause identified for each class of missing panels. Documented as categories (e.g., "emColorField sub-panels missing", "emListBox items missing", "settle cycles insufficient").

### Task 2.1: Categorize missing panels

**Files:** None (analysis only)

- [ ] **Step 1: Analyze the diff output from Phase 1**

From the diff output, classify missing panels into categories:
1. **Widget sub-panels** — children of ColorField, ListBox, FileSelectionBox that should be created by auto-expansion
2. **Category group internals** — children that should exist within raster groups
3. **Missing threshold** — panels that exist in both trees but aren't ae_expanded in Rust
4. **Missing `viewed`** — panels that exist but aren't marked viewed in Rust (so GetViewCondition returns 0)

- [ ] **Step 2: Check settle cycle sufficiency**

Add temporary debug output to composition_tktest_1x to count panels per settle round:

```rust
    for round in 0..200 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
        if round < 20 || round % 50 == 0 {
            eprintln!("round {round}: {} panels", tree.all_ids().len());
        }
    }
```

Run: `DUMP_PANEL_TREE=1 cargo test --test golden composition_tktest_1x -- --test-threads=1 --nocapture 2>&1 | grep "^round"`

Expected: Panel count should stabilize well before round 200. If it stabilizes at a low number, the issue is in expansion logic, not cycle count.

- [ ] **Step 3: For common panels with differing child counts, check why**

For each panel that exists in both trees but has fewer children in Rust:
- Check if the Rust behavior's LayoutChildren creates children
- Check if auto_expand is implemented
- Check if SetAutoExpansionThreshold is called

- [ ] **Step 4: Document findings**

Write a summary: which categories of panels are missing and likely root cause for each.

### Task 2.2: Verify expansion thresholds on child panels

**Files:** None (analysis only)

- [ ] **Step 1: Check which C++ panels have ae_expanded=1**

```bash
python3 -c "
import json
cpp = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.cpp_tree.jsonl')]
for p in cpp:
    if p['ae_expanded']:
        print(f\"{p['path']}  thresh={p['ae_thresh']} viewed={p['viewed']} children={p['children']}\")
"
```

- [ ] **Step 2: Check same for Rust**

```bash
python3 -c "
import json
rust = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.rust_tree.jsonl')]
for p in rust:
    if p['ae_expanded']:
        print(f\"{p['path']}  thresh={p['ae_thresh']} viewed={p['viewed']} children={p['children']}\")
"
```

- [ ] **Step 3: Compare the two lists**

Panels that are ae_expanded in C++ but not in Rust are the ones whose expansion is broken. This narrows the fix targets.

## Phase 2 Gate

**STOP.** Before proceeding, verify:
1. You have a categorized list of missing panels with root causes
2. You know exactly which behaviors need fixing (LayoutChildren, auto_expand, threshold)
3. The settle cycle count is NOT the issue (confirmed by round-by-round panel count)

**Report your diagnosis.** The diagnosis determines Phase 3 tasks.

---

## Phase 3: Fix expansion gaps

**Gate:** Rust panel tree matches C++ panel tree (same panel paths, same ae_expanded states). Ops count approaches C++ level.

**IMPORTANT:** The specific tasks in Phase 3 depend on Phase 2 findings. The tasks below cover the most likely causes based on the architecture analysis. If Phase 2 reveals different causes, adjust accordingly.

### Task 3.1: Fix missing auto-expansion for widget panels that expand in C++

**Context from architecture analysis:** In C++, emColorField, emListBox, and emFileSelectionBox override AutoExpand(). In Rust, the composition.rs wrappers for ColorFieldPanel and ListBoxPanel already have `auto_expand() -> true` and LayoutChildren that creates children. But the **threshold** may not be set on them, or the viewed condition may not reach their threshold.

**Files:**
- Modify: `crates/eaglemode/tests/golden/composition.rs`

- [ ] **Step 1: Check if auto-expanding panels have thresholds set**

Look at Phase 2 output. For each panel type that expands in C++ but not Rust:
- Is `SetAutoExpansionThreshold` called on the panel?
- Is the default threshold (150.0 Area) appropriate, or does C++ set a specific threshold?

For ColorField: C++ constructor calls `SetAutoExpansionThreshold(9, VCT_MIN_EXT)` — this IS already set in composition.rs (lines 543-566). Verify it's actually reached by the view condition.

For ListBox: C++ `emListBox` inherits default threshold (150.0 Area). Check if Rust ListBox panels get this check. The composition.rs ListBoxPanel has `auto_expand() -> true` but does NOT call `SetAutoExpansionThreshold`. The default 150.0 Area should apply... but verify that `update_auto_expansion` actually processes these panels.

For FileSelectionBox: Already implements PanelBehavior with auto_expand() -> true.

- [ ] **Step 2: Apply fixes based on Phase 2 findings**

For each missing panel class, apply the minimal fix. Possible fixes:
- Add `SetAutoExpansionThreshold` call if threshold mismatch
- Fix LayoutChildren if it doesn't create the right children
- Fix viewed coordinate computation if panels aren't marked viewed
- Add missing `auto_expand() -> true` if not implemented

- [ ] **Step 3: Run the panel tree dump and diff again**

```bash
make -C crates/eaglemode/tests/golden/gen run
DUMP_PANEL_TREE=1 cargo test --test golden composition_tktest_1x -- --test-threads=1
python3 scripts/diff_panel_tree.py tktest_1x
```

Expected: Missing panel count significantly reduced.

- [ ] **Step 4: Run clippy**

```bash
cargo clippy -- -D warnings
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -p  # stage only relevant changes
git commit -m "fix: close panel tree expansion gap for composition_tktest_1x"
```

### Task 3.2: Fix any remaining panel mismatches (iterative)

Repeat Task 3.1 Steps 2-5 until the diff shows no missing panels (or only panels that are intentionally different with documented DIVERGED reasons).

- [ ] **Step 1: Re-run diff and check remaining gaps**
- [ ] **Step 2: Fix the next class of missing panels**
- [ ] **Step 3: Re-run diff to confirm progress**
- [ ] **Step 4: Commit each fix separately**

## Phase 3 Gate

**STOP.** Before proceeding, verify:
1. `python3 scripts/diff_panel_tree.py tktest_1x` shows 0 missing panels (or only intentional divergences)
2. `cargo clippy -- -D warnings` passes
3. Ops count check:

```bash
DUMP_DRAW_OPS=1 cargo test --test golden composition_tktest_1x -- --test-threads=1
python3 -c "
import json
cpp = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.cpp_ops.jsonl') if l.strip().startswith('{')]
rust = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.rust_ops.jsonl') if l.strip().startswith('{')]
print(f'C++ ops: {len(cpp)}, Rust ops: {len(rust)}')
from collections import Counter
cc = Counter(o['op'] for o in cpp)
rc = Counter(o['op'] for o in rust)
for op in sorted(set(list(cc.keys()) + list(rc.keys()))):
    if cc[op] != rc[op]:
        print(f'  {op}: C++={cc[op]} Rust={rc[op]} delta={cc[op]-rc[op]}')
"
```

Expected: Ops count gap significantly reduced (Rust approaching 5470).

**Report ops count comparison.** If gap remains >10%, investigate remaining divergences before Phase 4.

---

## Phase 4: Verify golden tests

**Gate:** All 243 golden tests pass (or the same 13 that failed before still fail for known reasons, with no regressions).

### Task 4.1: Run full golden test suite

- [ ] **Step 1: Run golden tests**

```bash
cargo test --test golden -- --test-threads=1
```

Expected: Same pass/fail as before (229/243) or better. No regressions.

- [ ] **Step 2: If composition_tktest_1x now passes, update the tracking**

Check if the pixel output now matches. If it does, that's a win. If it doesn't (pixel differences remain due to other issues like canvas_color or clip), that's expected — the tree structure is now correct even if pixel arithmetic still diverges.

- [ ] **Step 3: Run the full test suite including non-golden tests**

```bash
cargo-nextest ntr
```

Expected: All tests pass.

- [ ] **Step 4: Final commit if needed**

```bash
git add -A
git commit -m "fix: panel tree expansion matches C++ for composition_tktest_1x"
```

## Phase 4 Gate

**STOP.** Verify:
1. `cargo-nextest ntr` passes
2. `cargo clippy -- -D warnings` passes
3. No regressions in golden test pass count
4. Panel tree diff shows matching trees

**Done.** Report final ops count comparison and test results.
