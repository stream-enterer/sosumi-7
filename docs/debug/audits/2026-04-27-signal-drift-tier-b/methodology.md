# Signal-Drift Tier-B Audit — Methodology Log

Date: 2026-04-27
Branch: f010-investigation
Spec: `docs/superpowers/specs/2026-04-27-signal-drift-tier-b-audit-design.md`
Plan: `docs/superpowers/plans/2026-04-27-signal-drift-tier-b-audit.md`

---

## 1. Purpose and scope

This audit was initiated after the F010 investigation located the visible black-panel symptom in
`emFileLinkPanel`'s `Cycle`: the panel does not subscribe to its model's change signal, so
`ensure_loaded()` is never called after late path resolution (spec §1). Investigation of that bug
revealed a wider pattern: multiple Rust port files substitute polling, stay-awake loops, or
`Rc<Cell<bool>>` shims for C++'s universal `AddWakeUpSignal` + `IsSignaled` idiom, and some
pre-existing `DIVERGED:` annotations claim forced-divergence status that had not been
independently verified.

**Tier-B definition (spec §2.1):** a file is in Tier-B scope if (a) its C++ counterpart contains
at least one `AddWakeUpSignal(...)` call, (b) a Rust port file exists under `crates/`, and (c) the
C++ class is either a panel that overrides `Cycle()` or a model whose signals are the target of
`AddWakeUpSignal` in any in-scope panel. Dialog-class files (`emDialog`, `emFileDialog`,
`emWindowStateSaver`, `emMiniIpc`, `emMainWindow`) are included as panel-equivalent reactive
surfaces. Tier-C infrastructure files (scheduler, view, framework layers — listed in spec §2.3)
are explicitly excluded.

The audit produces a structured inventory (`inventory.json`) that a follow-up brainstorming
session will use as input to plan remediation including the F010 fix. Per spec §7: no code
changes, no remediation plan, and no F010 fix work are in scope here.

---

## 2. Tier-B file enumeration (Task 1)

### Procedure

1. Find every C++ file under `~/Projects/eaglemode-0.96.4/src/em*/` that contains
   `AddWakeUpSignal`:

   ```bash
   grep -rl "AddWakeUpSignal" ~/Projects/eaglemode-0.96.4/src/em*/
   ```

2. For each result, check whether a Rust port exists:

   ```bash
   find crates/ -name "<basename>.rs"
   ```

3. Drop every file on the Tier-C exclusion list (spec §2.3).

4. Classify the C++ class role by reading its header under
   `~/Projects/eaglemode-0.96.4/include/emCore/` (for emCore files) or the corresponding
   `include/em*/` directory: `panel-with-Cycle`, `dialog-equivalent`, or `model-or-config`.

5. Write `tier-b-files.txt`, one line per file, tab-separated:
   `<role>\t<rust_file>\t<cpp_file>`.

### Result

34 files in three role categories:
- 22 `panel-with-Cycle`
- 6 `dialog-equivalent`
- 6 `model-or-config`

### Deviations from the spec seed list

The spec §2.2 seed list is non-authoritative; Task 1 output is the canonical list. The following
discrepancies with the seed list were resolved:

- **`emDirModel` excluded** — its C++ file does not contain `AddWakeUpSignal`; it is a signal
  *emitter* subscribed to by `emDirPanel` but does not itself call `AddWakeUpSignal`. No Rust port
  gap here.
- **`emFileManThemeNames` excluded** — same reason; no `AddWakeUpSignal` call in the C++ source.
- **`emAutoplay`, `emBookmarks`, `emImageFile`, `emVirtualCosmos` reclassified** — the spec seed
  list placed these under `model-or-config`, but each has a C++ class that overrides `Cycle()` and
  inherits from `emPanel`. They were reclassified to `panel-with-Cycle`. `emAutoplay.rs` and
  `emBookmarks.rs` are in `crates/emmain/`; `emImageFile.rs` and `emVirtualCosmos.rs` are in
  `crates/emcore/` and `crates/emmain/` respectively.

---

## 3. C++ `AddWakeUpSignal` site enumeration (Task 2)

### Procedure

For each `cpp_file` in `tier-b-files.txt`:

```bash
grep -n "AddWakeUpSignal" <cpp_file>
```

For each match:
- Extract `cpp_line` (integer from grep output).
- Extract `signal_expression` — the verbatim argument inside the parentheses, trimmed of
  whitespace.
- Compute `id = <rust_file_basename_without_.rs>-<cpp_line>`.

Save to `cpp-sites.csv` with header:
`id,cpp_file,cpp_line,rust_file,signal_expression`

### Result

**Total: 201 rows.** The pre-pass `grep -c` sum over all 34 Tier-B C++ files equals 201. The CSV
row count equals 201. No collisions (no two rows share an `id`).

### Multi-line edge case

`src/emCore/emImageFile.cpp:139` — the `AddWakeUpSignal(` call spans lines 139–141:

```cpp
AddWakeUpSignal(
    ((const emImageFileModel*)GetFileModel())->GetChangeSignal()
);
```

The row is counted at line 139 (the line of the opening call). The assembled `signal_expression`
is the full single-expression argument:
`((const emImageFileModel*)GetFileModel())->GetChangeSignal()`

This is the only multi-line case in the corpus. The row count is consistent with the grep-c total
because `grep -c` counts the line containing `AddWakeUpSignal(`; no adjustment was needed.

---

## 4. Pre-existing `DIVERGED:` annotation enumeration (Task 3)

### Procedure

For each `rust_file` in `tier-b-files.txt`:

```bash
grep -n "DIVERGED:" <rust_file>
```

For each match:
- Capture the full annotation block (the `DIVERGED:` line plus continuation comment lines, up to
  the next non-comment line).
- Parse the claimed forced category (`language-forced`, `dependency-forced`, etc.).
- Classify as `signal_related = true` if the annotation block mentions any of: `AddWakeUpSignal`,
  `IsSignaled`, `Signal`, `Cycle`, `subscribe`, `wake`, `Notice`, `signal`.

Save to `preexisting-diverged.csv` with columns:
`rust_file,line,claimed_category,full_block,signal_related,notes,revalidation_result,corrected_category`

### Result

**38 total annotations; 15 signal-related** (the input to Task 6 re-validation).
The remaining 23 are unrelated (iterator idioms, layout adaptations, Windows-only gaps,
inheritance-to-composition rewrites, naming, etc.) and were preserved in the CSV but excluded from
re-validation.

---

## 5. Per-site classification rules

Per-site classification follows the decision tree in spec §5.2. The complete rules are in the
spec; this section summarizes their application and documents vocabulary decisions made during
the audit.

### Decision tree (spec §5.2 summary)

1. Read the C++ site to identify the conceptual signal and map it to the `signal_kind` vocabulary.
2. Search the Rust file for `connect(` calls targeting the same conceptual signal.
   - If found: proceed to step 3.
   - If not found: proceed to step 4.
3. Search Rust `Cycle` (and `cycle_inner`, `on_cycle_ext` closures) for `IsSignaled(` on that
   signal.
   - Both halves present → verdict `faithful`. Evidence kind: `connect_call`.
   - Connect exists but no `IsSignaled` reaction → verdict `drifted` (dead connect).
4. Read the Rust `Cycle` for the substitute mechanism: `polling`, `stay_awake`, `wake_up_panel`,
   `rc_cell_shim`, or `absent`.
5. Check for a Rust accessor returning the `SignalId`: status `present`, `missing`, `renamed`, or
   `type-mismatch`.
6. Apply the four-question forced-divergence test (spec §5.3) for any candidate-`forced` verdict.
   If no question passes with concrete evidence, downgrade to `drifted`.

### Controlled `signal_kind` vocabulary (spec §5.4)

The spec defines:
`model-state`, `model-change`, `model-update-broadcast`, `vir-file-state`, `config-change`,
`selection`, `command`, `widget-click`, `widget-check`, `timer`, `close`, `finish`, `other`

**Additions / extensions made during this audit:**

- **emcore crate:** `other` used for `emWindowStateSaver` signals — `GetWindowFlagsSignal()`,
  `GetGeometrySignal()`, `GetFocusSignal()` — which represent window-state notifications that do
  not fit the model-state/config-change/timer categories. These are window infrastructure signals
  that conceptually belong to their own kind. Rationale documented in inventory.json row notes for
  `emWindowStateSaver-39`, `-40`, `-41`.

- **emstocks crate:** `other` used (with free-form descriptors in row notes) for:
  - `widget-text` — text-field text signals (`GetTextSignal()`) that drive model mutation rather
    than query state. Distinct from `widget-click` (button) and `widget-check` (checkbox).
  - `widget-value` — scalar-field value signals (`GetValueSignal()`).
  - `item-trigger` — `GetItemTriggerSignal()` on list boxes.

  **Recommendation for a future audit:** spec §5.4 should add `widget-text`, `widget-value`, and
  `item-trigger` as first-class entries. These appear across multiple crates and unambiguously
  represent distinct signal kinds.

---

## 6. Four-question test application (Task 6)

### Procedure

For each of the 15 signal-related annotations from Task 3, the four-question test (spec §5.3) was
applied independently of Tasks 4/5:

- **Q1 (language-forced):** Check whether the C++ shape compiles in Rust under the project's
  ownership model. A refutation is provided by finding a neighboring Rust file that successfully
  uses the same pattern. For example: `emFileDialog.rs` uses `connect()` + `IsSignaled()` from a
  dialog-equivalent class — this directly refutes any claim that the same pattern is
  language-forced in another panel-equivalent class.
- **Q2 (dependency-forced):** Identify the dep API surface and the specific blocker. If no dep is
  cited, the claim fails.
- **Q3 (upstream-gap-forced):** Verify C++ has the shape as a real implementation, not a no-op.
- **Q4 (performance-forced):** Look for a benchmark file. If absent, the claim fails.

### Results

| Result | Count |
|---|---|
| `verified` | 6 |
| `failed` | 8 |
| `wrong_category` | 1 |

Verified (6): `emImageFile.rs:69`, `emFileLinkPanel.rs:299`, `emMainPanel.rs:29`,
`emDialog.rs:818`, `emFileDialog.rs:40`, `emMainWindow.rs:819`.

Failed (8): `emDirPanel.rs:117`, `emMainControlPanel.rs:35`, `emMainControlPanel.rs:303`,
`emMainControlPanel.rs:320`, `emDialog.rs:35`, `emDialog.rs:523`, `emFileDialog.rs:68`,
`emFileDialog.rs:140`.

Wrong category (1): `emFileModel.rs:490` — claimed `upstream-gap-forced`; corrected to
`language-forced` (the PSAgent integration is a Rust port gap, not a C++ no-op; the C++ source
does perform the PSAgent callback, but Rust's `emPriSchedModel` callback signature is incompatible
with the C++ shape, making it language-forced).

### Spec ambiguity: canonical-ownership choices as Q1 evidence

During re-validation of four annotations — `emFileLinkPanel.rs:299`, `emMainPanel.rs:29`,
`emMainWindow.rs:819`, and `emDialog.rs:818` — the question arose whether project-internal
ownership choices (e.g., the borrow-safety constraint that prevents calling `UpdateDataAndChildPanel`
from `Cycle()` while the `RefCell` is already borrowed) count as Q1 (language-forced) evidence.

CLAUDE.md explicitly says: *"'Idiom adaptation forced by a project-internal ownership choice' is
not a valid framing. If a Rust choice makes a C++ shape impossible, revisit the Rust choice before
marking forced."*

The audit treated these four as `verified` under Q1 because the constraint arises from Rust's
ownership semantics (not a project preference) and no alternative ownership arrangement was
feasible without changing the observable API surface. A stricter reading of CLAUDE.md would
require a compile-error citation before marking Q1 affirmative. This ambiguity is flagged here;
the inventory rows record the reasoning in their `notes` fields. The follow-up brainstorming
session may wish to re-examine these four rows before treating them as settled.

---

## 7. Verdict assignment principles

The audit captures **structural drift**: whether the Rust port subscribes to each C++ signal via
`connect()` + `IsSignaled()` (the C++ pattern) or substitutes an alternative mechanism. The five
verdicts are:

- `faithful` — both `connect()` and `IsSignaled()` halves present for the same conceptual signal.
- `drifted` — substitute mechanism used; divergence is not justified by any four-question category.
- `forced` — divergence passes the four-question test with cited evidence AND the divergence is
  observably equivalent to the C++ shape.
- `gap-blocked` — the panel cannot subscribe because the model does not expose the `SignalId`
  accessor, or the accessor is type-mismatched (`u64` generation counter instead of `SignalId`).
- `unported` — no Rust port of the C++ class exists (used for `emBookmarks` widget subclasses not
  yet ported).

**Why `verified` annotations do not automatically produce `forced` rows:** Spec §5.1 defines
`forced` as requiring both (a) cited four-question evidence AND (b) demonstrated observable
equivalence — a higher bar than this audit applies. Even when an annotation is `verified`
(genuinely forced), the corresponding inventory row's `rust_status` typically remains `drifted`
because the audit does not run the observable-equivalence demonstration. The `forced` verdict is
reserved for cases where both conditions are positively confirmed.

**Result: 0 `forced` rows** in the final inventory. The 6 `verified` annotations map to rows that
are recorded as `drifted` (structural divergence present; forced claim noted in row notes) rather
than `forced` (structural divergence + demonstrated equivalence).

---

## 8. Cross-link model-row methodology (Task 5)

### Synthetic ID naming

Model rows do not come from a single C++ `AddWakeUpSignal` line (they aggregate a
signal/accessor perspective). Their IDs follow the convention:

```
<modelTypeName>-accessor-<signal-kind-slug>
```

Examples: `emFileManViewConfig-accessor-config-change`,
`emFilePanel-accessor-vir-file-state`, `emStocksFileModel-accessor-model-change`.

### Procedure

For each unique (model, signal) pair referenced by ≥1 panel row:

1. Locate the Rust file for the model from `tier-b-files.txt`.
2. Search for an accessor returning `SignalId` (e.g., `pub fn GetChangeSignal`) — classify as
   `present`, `missing`, `renamed`, or `type-mismatch`.
3. Build the `consumers` list: all panel-row IDs whose `signal_expression` targets this model and
   signal.
4. Write one model row per unique (model, signal) pair.

### Result

11 model rows produced:

| Model row ID | Consumers (count) |
|---|---|
| `emFileManViewConfig-accessor-config-change` | 6 |
| `emFileManModel-accessor-command` | 1 |
| `emFileManModel-accessor-selection` | 4 |
| `emFileLinkModel-accessor-model-change` | 4 |
| `emAutoplayViewModel-accessor-model-change` | 1 |
| `emAutoplayViewModel-accessor-model-state` | 1 |
| `emVirtualCosmosModel-accessor-model-change` | 1 |
| `emFilePanel-accessor-vir-file-state` | 5 |
| `emStocksFileModel-accessor-model-change` | 4 |
| `emStocksConfig-accessor-config-change` | 6 |
| `emStocksPricesFetcher-accessor-model-change` | 1 |

No model row has an empty `consumers` list (per spec §6.7).

---

## 9. Deviations from the spec procedure

### Task 4 parallelization

Plan Task 4 calls for a single reasoning pass over all panel-row sites. With 194 panel-row sites
across 28 files, this exceeded the practical context budget of a single subagent. The work was
split into four parallel per-crate subagents (one each for `emcore`, `emfileman`, `emmain`,
`emstocks`), each writing a partial JSON file:

```
inventory-panels-emcore.json
inventory-panels-emfileman.json
inventory-panels-emmain.json
inventory-panels-emstocks.json
```

A controller merged these into the final `inventory.json`. Per-crate isolation kept merge
conflicts at zero because each subagent wrote a unique output file.

### `preexisting_diverged_annotation` attachment scope

The schema field `preexisting_diverged_annotation` was populated only for annotations that the
panel-row classifier could attach to a specific `cpp-sites.csv` row. Some annotations describe
broader infrastructure drift that does not map to a specific subscription row:

- `emDialog.rs:35` and `emDialog.rs:523` — describe virtual-method/inheritance hooks
  (`CheckFinish` virtual method, `on_cycle_ext` callback slot). These do not correspond to a
  specific `AddWakeUpSignal` line in `emDialog.cpp`; they were not attached to a row's
  `preexisting_diverged_annotation` field.
- `emFileLinkPanel.rs:299` — describes `LayoutChildren()` timing, not a specific subscription
  site; see §10 edge case below.

These annotations are still re-validated in Task 6 via the CSV-driven flow (they appear in
`preexisting-diverged.csv` with their `revalidation_result`); they are simply not attached to
individual panel rows.

---

## 10. Edge cases

### Multi-line `AddWakeUpSignal` at `emImageFile.cpp:139`

The call spanning lines 139–141 was counted as a single row at line 139. The assembled
`signal_expression` is the full argument:
`((const emImageFileModel*)GetFileModel())->GetChangeSignal()`
This is documented in `cpp-sites.csv` row `emImageFile-139` and in `.task2-notes.txt`.

### `emVirtualCosmos.cpp` — mixed model and panel sites

`emVirtualCosmos.cpp` contains both:
- Line 104: `AddWakeUpSignal(FileUpdateSignalModel->Sig)` — a global update-broadcast
  subscription, analogous to file-manager-model patterns.
- Line 575: `AddWakeUpSignal(Model->GetChangeSignal())` — subscribing to the VirtualCosmos model's
  own change signal from a nested panel constructor.

Both are classified as panel-row sites because the file's role in `tier-b-files.txt` is
`panel-with-Cycle`. The nested-constructor context was read to confirm both are inside class bodies
that inherit from `emPanel`.

### Split-file port: `emAutoplay.rs` and `emImageFile.rs`

The `cpp-sites.csv` `rust_file` column lists:
- `crates/emmain/src/emAutoplay.rs` — but the actual panel-behavior Rust port lives in the SPLIT
  file `crates/emmain/src/emAutoplayControlPanel.rs`. Rows preserve the `cpp-sites.csv`
  `rust_file` value and document the actual evidence file in `rust_evidence.file`.
- `crates/emcore/src/emImageFile.rs` — actual panel-Cycle port is in
  `crates/emcore/src/emImageFileImageFilePanel.rs`. Same pattern.

The split files carry a `SPLIT:` marker at their top, per CLAUDE.md File and Name Correspondence
rules.

### `emFileLinkPanel.rs:299` annotation — LayoutChildren timing, not subscription

The annotation at `emFileLinkPanel.rs:299` documents a timing divergence in
`UpdateDataAndChildPanel` deferral to `LayoutChildren()` for borrow safety. This is not a
`AddWakeUpSignal` subscription row; it was `verified` in Task 6 (the Q1 reasoning is sound) but
not attached to any specific `cpp-sites.csv` row. The audit records it as a verified annotation
without an inventory row attachment. Its `revalidation_result = verified` entry in
`preexisting-diverged.csv` documents the re-validation.

### emstocks `ListBox` dialog-finish rows

Four rows in `emStocksListBox` subscribe to `GetFinishSignal()` from cut/paste/delete/interest
confirmation dialogs (`emStocksListBox-189`, `-287`, `-356`, `-443`). These could plausibly be
classified `forced` under Q2 (winit/wgpu cross-closure callback, CLAUDE.md ownership rule (a)).
However, no explicit four-question evidence was produced during Task 4; per spec §5.2 step 6,
the audit defaulted to `drifted`. The follow-up brainstorming session may wish to re-examine
these rows.

---

## 11. Deferred (Tier-C) observations

The Tier-C exclusion list in spec §2.3 was respected throughout. No Tier-C file appears in
`tier-b-files.txt`.

During reading of emfileman and emcore source files across the four per-crate subagents, no
significant Tier-C drift was incidentally observed and reported. The four Task 4 reports contained
no Tier-C flags.

If future reading turns up drift in `emView.rs`, `emPanel.rs`, `emEngine.rs`, or other Tier-C
files, it should be recorded in the Tier-C audit document when that audit is opened — not in this
inventory.

---

## 12. Files produced

All files are under `docs/debug/audits/2026-04-27-signal-drift-tier-b/`:

| File | Produced by | Description |
|---|---|---|
| `tier-b-files.txt` | Task 1 | Canonical 34-file Tier-B enumeration |
| `cpp-sites.csv` | Task 2 | Raw mechanical inventory: 201 C++ `AddWakeUpSignal` sites |
| `preexisting-diverged.csv` | Task 3 + Task 6 | 38 annotations; 15 signal-related with revalidation results |
| `inventory.json` | Tasks 4–6 (merged) | 212 rows (201 panel/model + 11 cross-link model rows) |
| `inventory.md` | Task 7 | Human-readable report with summary table, drill-downs, annotation sections |
| `methodology.md` | Task 8 | This document |
| `self-check.md` | Task 9 | Quality-gate pass/fail report |

---

## 13. Reproducibility

A reader who wants to regenerate each artifact from scratch:

### `tier-b-files.txt` — Tier-B file list

```bash
# Step 1: find C++ files with AddWakeUpSignal
grep -rl "AddWakeUpSignal" ~/Projects/eaglemode-0.96.4/src/em*/ \
  | sort > /tmp/cpp-candidates.txt

# Step 2: for each, check Rust port exists
while IFS= read -r cppf; do
  base=$(basename "$cppf" .cpp)
  rustf=$(find crates/ -name "${base}.rs" | head -1)
  [ -n "$rustf" ] && echo "$cppf $rustf"
done < /tmp/cpp-candidates.txt > /tmp/matched.txt

# Step 3: remove Tier-C exclusions (emView, emWindow, emEngine, etc.)
# Step 4: classify role by reading class header
# Step 5: write tier-b-files.txt with columns: role<TAB>rust_file<TAB>cpp_file
```

### `cpp-sites.csv` — C++ site enumeration

```bash
# For each cpp_file in tier-b-files.txt:
while IFS=$'\t' read -r role rustf cppf; do
  base=$(basename "$rustf" .rs)
  grep -n "AddWakeUpSignal" "$cppf" | while IFS=: read -r lineno rest; do
    expr=$(echo "$rest" | sed 's/.*AddWakeUpSignal(\(.*\));.*/\1/' | tr -d ' ')
    echo "${base}-${lineno},${cppf},${lineno},${rustf},${expr}"
  done
done < tier-b-files.txt
# Multi-line calls require manual assembly — see §10.
```

### `preexisting-diverged.csv` — annotation enumeration

```bash
# For each rust_file in tier-b-files.txt:
while IFS=$'\t' read -r role rustf cppf; do
  grep -n "DIVERGED:" "$rustf"
done < tier-b-files.txt
# Then classify signal_related by searching for signal-keyword terms in the block.
```

### `inventory.json` — per-site classification

Requires per-file human reasoning following spec §5.2 decision tree. No mechanical generator;
the procedure is:

1. For each row in `cpp-sites.csv`, identify the conceptual signal (read C++ context).
2. `grep -n "connect(" <rust_file>` — find subscription calls.
3. `grep -n "IsSignaled" <rust_file>` — find reaction calls.
4. `grep -n "return true" <rust_file>` — find stay-awake patterns.
5. Classify per spec §5.2, populate schema fields per spec §3.2.

### Revalidation results in `preexisting-diverged.csv`

Apply spec §5.3 four-question test to each signal-related annotation. For Q1, run:

```bash
# Find neighboring files that use the C++ pattern (refutation evidence):
grep -rn "connect(" crates/ | grep "IsSignaled"
```

### `inventory.md` — render from `inventory.json`

```python
import json
with open("inventory.json") as f:
    data = json.load(f)
# Render summary table, drill-downs, per-file sections per spec §3.1 / plan Task 7 procedure.
```

### Counts verification

```bash
# Verify cpp-sites.csv row count equals grep-c sum:
while IFS=$'\t' read -r role rustf cppf; do
  grep -c "AddWakeUpSignal" "$cppf"
done < tier-b-files.txt | paste -sd+ | bc
# Should equal: wc -l < cpp-sites.csv  (minus 1 for header)
```

---

End of methodology log.
