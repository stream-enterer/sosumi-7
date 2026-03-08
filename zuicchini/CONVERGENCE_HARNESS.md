# Convergence Harness

> Adapted from PARITY_HARNESS.md. Runs the same pipeline with a third input:
> a **convergence plan** file containing human-resolved design decisions.
>
> The plan file is edited between runs. The harness reads it fresh each time
> and processes only items with a non-empty `resolution` field. This allows
> divide-and-conquer: resolve one phase, run the harness, review, repeat.
>
> Self-contained. Hand this document plus both codebases plus the plan file
> to an LLM and it can run the pipeline autonomously.

---

## 1. General Principles

Seven failure modes that recur when LLMs do C++→Rust parity work.
Each names a cognitive failure, explains why it happens, and prescribes
a structural countermeasure that the pipeline enforces.

### 1.1 Asymmetric Competence

**Failure:** LLMs verify against checklists far more reliably than they
detect differences by side-by-side comparison. Showing an LLM two
implementations and asking "are these the same?" triggers pattern-matching
on surface syntax, not behavioral analysis. Renamed variables, reordered
blocks, and idiomatic Rust rewrites all fool the comparison.

**Countermeasure:** Never compare two codebases side-by-side. Extract
behavioral specs from C++ (Phase 1), then verify Rust against specs
(Phase 3). The verifying agent never sees raw C++.

### 1.2 Enumerate, Then Verify

**Failure:** LLMs skip branches when given bulk "compare these files"
tasks. They latch onto the first few branches, declare them correct,
and summarize the rest as "handles remaining cases similarly." Interior
branches of switch statements and ternary short-circuits are the
most common casualties.

**Countermeasure:** Mandatory exhaustive branch enumeration before any
verification. The extract phase must list every `if`/`else`/`switch`/
`case`/`?:` branch. Quality gate: the branch count in the extract must
meet or exceed `grep -c 'if\|else\|switch\|case' source.cpp`.

### 1.3 Context Windows Are Physical Constraints

**Failure:** Large files (painter 3,474 Rust LOC / 3,644+ C++ LOC,
view 1,874 LOC, text_field 2,678 LOC) defeat analysis when crammed
into one context. The model's attention degrades on long inputs and
findings from early in the file get lost by the time the model writes
conclusions.

**Countermeasure:** Hard budget of ~1,500 source lines per analysis
unit. Files exceeding this are chunked by method group. Each chunk
produces its own output file. Progress persists between chunks via
the filesystem, not the context window.

### 1.4 Structural Reshaping Is Not a Free Pass

**Failure:** RESTRUCTURED mappings (where one C++ method maps to
multiple Rust functions or vice versa) get treated leniently. The
model writes "behavior was reorganized" and moves on. Gaps hide
behind the reorganization.

**Countermeasure:** Every branch from the C++ spec must trace to a
specific Rust file:line, regardless of restructuring. RESTRUCTURED
mappings get expanded verification, not reduced: the agent must list
every Rust function that jointly implements the C++ method and verify
each branch across all of them.

### 1.5 Intentional Divergence Requires Citation

**Failure:** LLMs invent plausible reasons for differences. "This was
likely adapted for Rust's ownership model" or "probably intentional
given the wgpu backend" — without any evidence. These invented
justifications mask real bugs.

**Countermeasure:** Only `EMCORE_FEATURE_CONTRACT.md` and items marked
`resolution: KEEP` in the convergence plan can authorize divergence.
Contract divergences are cited by decision number. Plan divergences
are cited by plan item ID.

### 1.6 State Persists in Files, Not Context

**Failure:** Long sessions lose track of findings. The model forgets
earlier discoveries, re-verifies already-checked methods, or loses
the thread of cross-method dependencies. Context compression and
multi-session workflows make this worse.

**Countermeasure:** All progress is filesystem-based with defined
schemas. Each phase reads its inputs from `_verify/` and writes
structured outputs back. Sessions are resumable: scan `_verify/`
for existing state, continue from where the last session left off.

### 1.7 Skepticism Is Default

**Failure:** LLMs say "YES, handles it" without line-number evidence.
The model generates a plausible explanation of how the code "probably"
handles a branch, writes YES, and moves on. When challenged, these
turn out to be fabrications.

**Countermeasure:** Every YES requires file:line evidence — a quoted
code snippet with a line number. Every NO requires a description of
what is missing. When uncertain between CORRECT and BEHAVIORAL,
the tie goes to BEHAVIORAL (the more severe classification). False
negatives (missing a real gap) are worse than false positives
(flagging something that turns out to be fine).

---

## 2. Pipeline Overview

Five phases, each with defined inputs, outputs, and quality gates.
Phases are strictly sequential (outputs of N feed inputs of N+1).

```
Phase 0: SURVEY          Phase 1: EXTRACT         Phase 2: MAP
git log + file sizes     C++ → behavioral spec    spec + Rust sigs →
→ maturity map,          YAML per C++ file        traceability index
  execution order                                 per subsystem

        ↓                       ↓                       ↓

                    Phase 3: VERIFY              Phase 4: FIX
                    spec + Rust source →         verdicts → code
                    verdicts per method/file     changes → re-verify

```

**Key invariant:** The verifying agent (Phase 3) never sees raw C++.
The fixing agent (Phase 4) never judges its own work — a fresh
Phase 3 re-run does the judging.

**Third input:** The convergence plan (`CONVERGENCE_PLAN.yaml`) feeds
into Phase 4. Items with `resolution: PORT` become actionable fix
targets. Items with `resolution: KEEP` expand the intentional
divergence filter for Phase 3. Items with no resolution or
`resolution: DEFER` are skipped entirely.

**Incremental execution.** The plan groups items into implementation
phases. The orchestrator can run a subset of phases by filtering to
items in a specific plan phase. Unresolved items are invisible to
the pipeline.

**Resumption from prior run.** If `_verify/` contains extracts,
mappings, and verdicts from a prior pipeline run, the harness reuses
them. It only runs Phases 0–3 for items that need new extracts
(marked `needs_extract: true` in the plan).

---

## 3. Setup and Paths

### 3.1 Codebase Locations

```
C++ source:    ~/.local/git/eaglemode-0.96.4/src/emCore/
C++ headers:   ~/.local/git/eaglemode-0.96.4/include/emCore/
Rust source:   ~/Development/egopol/zuicchini/src/
Contract:      ~/Development/egopol/zuicchini/EMCORE_FEATURE_CONTRACT.md
Plan:          ~/Development/egopol/zuicchini/CONVERGENCE_PLAN.yaml
Verify state:  ~/Development/egopol/zuicchini/_verify/
Fix state:     ~/Development/egopol/zuicchini/_fix/
```

### 3.2 Directory Structure

```bash
mkdir -p _verify/{extracts,mappings,verdicts,rust_signatures}
mkdir -p _fix/{context,changes,baselines}
```

The `_verify/` and `_fix/` directories are generated state, not source.
They should be in `.gitignore`. Contents are reproducible by re-running
the pipeline.

### 3.3 Pre-computed Inputs

Before launching any agent, generate Rust signature indices:

```bash
for dir in foundation input layout model panel render scheduler widget window; do
  grep -rn 'pub fn\|pub(crate) fn\|fn ' src/$dir/ > _verify/rust_signatures/$dir.txt 2>/dev/null
done
```

These give Phase 2 agents a compact view of Rust functions without
needing to read full source files.

### 3.4 Convergence Plan Format

The convergence plan is a YAML file with this structure:

```yaml
phases:
  - phase: <number>
    name: <human name>
    depends_on: [<phase numbers>]
    items:
      - id: <unique ID, matches design_decisions.yaml or new>
        summary: <one line>
        detail: <context>
        options: [<choice A>, <choice B>, ...]
        resolution:       # ← human fills this in
        status:           # ← harness updates this
        needs_extract: <bool>
        affected_files: [<paths>]
        depends_on: [<item IDs>]  # optional, for intra-phase ordering
```

**Resolution semantics:**

| Resolution | Harness Action |
|---|---|
| *(empty)* | Skip — item is not processed |
| `PORT` | Implement C++ behavior from the extract. Autonomous fix. |
| `PORT: <instructions>` | Implement with specific approach. Instructions passed to fix agent. |
| `KEEP` | Mark as intentional divergence. No code changes. Added to Phase 3 filter. |
| `DEFER` | Skip — equivalent to empty, but signals "revisit later" intent. |

**Status values (set by harness):**

| Status | Meaning |
|---|---|
| `DONE` | Fix applied. Compiles. Passes clippy + tests. |
| `VERIFIED` | Re-verification confirms the fix and no regressions. |
| `FAILED` | Fix attempt failed after 3 tries. See change log. |
| `BLOCKED` | Depends on an unresolved item. Cannot proceed. |

---

## 4. Risk Tiers and Subsystem Map

Three tiers control analysis depth. Higher tiers get more granular
verification to match their risk of subtle bugs.

### 4.1 Tier Definitions

| Tier | Depth | Why |
|---|---|---|
| **1** | Per-method extraction and verification | Algorithmic code where off-by-one, wrong formula, or missing branch = visible bug |
| **2** | Per-file extraction and verification | Complex but less dense — logic errors are structural, not arithmetic |
| **3** | API coverage check only | Thin wrappers, well-defined types, or winit delegation — low risk of subtle bugs |

### 4.2 Subsystem Table

| Tier | C++ Files | Rust Files | C++ LOC | Rust LOC | Chunks | Contract Decisions |
|---|---|---|---|---|---|---|
| **1** | `emPainter.cpp`, `emPainter_ScTl*.cpp` (9 files) | `render/painter.rs`, `render/scanline.rs`, `render/interpolation.rs` | ~12,165 | 4,460 | 4 | Decision #2 (CPU raster + wgpu) |
| **1** | `emScheduler.cpp`, `emEngine.cpp`, `emSignal.cpp`, `emTimer.cpp` | `scheduler/core.rs`, `scheduler/engine.rs`, `scheduler/signal.rs`, `scheduler/timer.rs` | ~543 | 1,099 | 1 | (none — direct port) |
| **1** | `emView.cpp`, `emViewAnimator.cpp`, `emViewInputFilter.cpp` | `panel/view.rs`, `panel/animator.rs`, `panel/input_filter.rs` | ~6,082 | 4,366 | 3 | Decision #4 (arena + handles), winit |
| **2** | `emPanel.cpp` | `panel/tree.rs`, `panel/ctx.rs` | ~2,696 | 2,362 | 2 | Decision #4 (arena + handles) |
| **2** | `emBorder.cpp` | `widget/border.rs` | ~1,970 | 1,600 | 1 | (none — direct port) |
| **2** | `emLinearLayout.cpp`, `emPackLayout.cpp`, `emRasterLayout.cpp` | `layout/linear.rs`, `layout/pack.rs`, `layout/raster.rs` | ~1,663 | 1,418 | 1 | (none — direct port) |
| **2** | `emTextField.cpp` | `widget/text_field.rs` | ~2,274 | 2,678 | 2 | (none — direct port) |
| **3** | `emColor.cpp`, `emImage.cpp` | `foundation/color.rs`, `foundation/image.rs` | ~2,470 | 1,252 | — | (none — direct port) |
| **3** | `emLabel.cpp`, `emCheckBox.cpp`, `emRadioBox.cpp`, `emButton.cpp`, `emCheckButton.cpp`, `emRadioButton.cpp`, `emSplitter.cpp`, `emScalarField.cpp`, `emColorField.cpp`, `emListBox.cpp` | `widget/label.rs`, `widget/check_box.rs`, `widget/radio_box.rs`, `widget/button.rs`, `widget/check_button.rs`, `widget/radio_button.rs`, `widget/splitter.rs`, `widget/scalar_field.rs`, `widget/color_field.rs`, `widget/list_box.rs` | ~3,826 | 1,498 | — | (none — direct port) |
| **3** | `emWindow.cpp`, `emScreen.cpp`, `emWindowStateSaver.cpp` | `window/zui_window.rs`, `window/screen.rs`, `window/state_saver.rs`, `window/app.rs` | ~502 | 1,040 | — | winit |

### 4.3 Tier Depth Differences

- **Tier 1 extract** includes: `calls`, `state_mutations`, per-branch detail.
  Verify runs **per-method** (one agent invocation per method or small method group).
- **Tier 2 extract** omits `calls` and `state_mutations`.
  Verify runs **per-file** (all methods from one C++ file in one agent).
- **Tier 3** skips the 3-phase pipeline entirely. Uses the lightweight
  API Coverage Check (Section 8.4).

### 4.4 Chunking Strategy

Files exceeding ~1,500 lines per analysis unit must be chunked:

| File | Strategy |
|---|---|
| `emPainter.cpp` + ScTl files (12K+ C++ LOC) | 4 chunks: (1) core primitives, (2) line/arrow/bezier, (3) image/text, (4) ScTl blending variants |
| `emView.cpp` (2,719 C++ LOC) | 2 chunks: (1) lifecycle + coordinates, (2) navigation + focus |
| `emViewAnimator.cpp` (2,060 C++ LOC) | 2 chunks: (1) base animator + smooth zoom, (2) swiping + magnetic |
| `panel/tree.rs` (2,206 Rust LOC) | 2 chunks: (1) tree structure + lifecycle, (2) notices + layout |
| `widget/text_field.rs` (2,678 Rust LOC) | 2 chunks: (1) cursor + selection + undo, (2) painting + scroll |
| `render/painter.rs` (3,474 Rust LOC) | Use same 4-chunk split as C++ painter |

Each chunk reads the header first (for full method list), then processes
its portion of the source. Output appends to the same extract file.

---

## 5. Phase 0 — SURVEY (Scope and Plan Intake)

**Purpose:** Determine what work to do this run. Scan the plan for
resolved items, check existing artifacts, and produce an execution
manifest.

**Input:** Convergence plan, `_verify/` state, `_fix/` state.
**Output:** `_verify/survey.md` (updated), execution manifest on stdout.

### 5.1 Agent Prompt

```
You are surveying the state of a Rust codebase (zuicchini) that is being
converged with a C++ library (emCore from Eagle Mode). A prior pipeline
run has already produced extracts, mappings, and verdicts.

Read:
  - Convergence plan: CONVERGENCE_PLAN.yaml
  - Existing artifacts: ls _verify/ and _fix/

Produce a survey covering:

1. RESOLVED ITEMS: List every plan item with a non-empty resolution.
   For each, note:
   - Does an extract exist for the relevant C++ methods?
   - Does a mapping exist?
   - Does the item need a new extract (needs_extract: true)?
   - Are dependencies satisfied (depends_on items resolved)?

2. EXECUTION ORDER: Sort resolved items by:
   a. Phase number (lower first)
   b. Dependency order within phase
   c. Items needing new extracts before items that don't

3. BLOCKED ITEMS: List resolved items whose depends_on items are
   unresolved. These cannot be processed this run.

4. KEEP ITEMS: List items with resolution: KEEP. These expand the
   intentional divergence filter for Phase 3.

Write output to: _verify/survey.md
```

### 5.2 Expected Output

```markdown
## Resolved Items (this run)

| ID | Phase | Resolution | Needs Extract | Dependencies Met |
|---|---|---|---|---|
| D-LAYOUT-01 | 3 | PORT | no | yes |
| D-LAYOUT-02 | 3 | PORT | no | yes (D-LAYOUT-01 resolved) |
| ...

## Execution Order
1. D-LAYOUT-01 (Phase 3, no extract needed)
2. D-LAYOUT-02 (Phase 3, depends on D-LAYOUT-01)
...

## Blocked Items
- D-PANEL-10 (depends on D-PANEL-09, which is DEFER)

## New Intentional Divergences (KEEP)
- PAINTER-FONT: TTF font system is intentional
```

---

## 6. Phase 1 — EXTRACT (C++ → Behavioral Spec)

**Purpose:** Read C++ source and produce a structured behavioral
description. One agent per C++ file. Agent sees ONLY C++ — no Rust.

**Input:** C++ header + source file.
**Output:** `_verify/extracts/{filename}.yaml`

**When to run:** Only for plan items with `needs_extract: true` whose
C++ methods were not already extracted at sufficient depth. If an
extract already exists from the prior run, reuse it (C++ source is
a released version and hasn't changed).

### 6.1 Agent Prompt Template

````
You are analyzing C++ source code from Eagle Mode's emCore library.
Your job is to produce a precise behavioral extract. You will NOT see
or evaluate any Rust code. Focus exclusively on what this C++ code does.

Read the following files:
  - Header: {header_path}
  - Source: {source_path}

{chunk_instruction}

For each public method and each non-trivial private method, produce
the following YAML structure. A "trivial" method is one whose entire
body is a single return of a member variable with no conditionals.
Everything else must be extracted.

```yaml
- method: <class>::<method_name>
  signature: <return_type>(<params>)
  lines: <start>-<end>
  behavior: <1-3 sentence description of what it does>
  {tier1_fields}
  branches:
    - condition: <the if/switch condition, verbatim or summarized>
      effect: <what happens in this branch>
      line: <line number>
  constants:
    - name: <constant or magic number>
      value: <value>
      usage: <what it controls>
      line: <line number>
  edge_cases:
    - <description of boundary condition or guard>
```

Include ALL conditional branches — even single-line early returns,
ternary operators, and short-circuit evaluations that guard side
effects. If a branch is guarded by a flag check, record the flag
name and what triggers it.

Do NOT summarize or skip "obvious" code. The purpose of this extract
is to serve as a verification spec. Every branch you omit is a branch
that will go unverified.

Write the output as valid YAML to: {output_path}
````

### 6.2 Template Variables

| Variable | Tier 1 Value | Tier 2 Value |
|---|---|---|
| `{tier1_fields}` | `calls: [<same-class method calls>]` and `state_mutations: [<member modifications>]` | *(omitted)* |
| `{chunk_instruction}` | `Process methods {start_method} through {end_method} (lines ~{start_line}-{end_line}). Append to the existing extract file if it exists.` | `Process the entire file.` |

### 6.3 Quality Gate

After each extract, verify:

```bash
# Count conditional keywords in C++ source
cpp_branches=$(grep -c 'if\|else\|switch\|case' {source_path})

# Count branches in extract
extract_branches=$(grep -c 'condition:' {output_path})

# Extract must have at least as many branches as keyword count
# (approximate lower bound — ternaries push the real count higher)
echo "C++ keywords: $cpp_branches, Extract branches: $extract_branches"
```

If `extract_branches < cpp_branches * 0.8`, the extract is incomplete.
Re-run with explicit instruction to enumerate all branches.

---

## 7. Phase 2 — MAP (Spec → Traceability Index)

**Purpose:** Link every extracted method to its Rust counterpart(s).
Identify gaps, intentional omissions, and structural reshaping.

**Input:** Extract YAML + Rust signature index.
**Output:** `_verify/mappings/{subsystem}.yaml`

**When to run:** Only for new extracts produced in Phase 1 this run.
Existing mappings from the prior run are reused unless Rust files were
structurally reorganized.

### 7.1 Agent Prompt Template

````
You have a behavioral extract from C++ emCore and the Rust zuicchini
source. Your job is to create a mapping between them.

Read:
  - Extract: {extract_path}
  - Rust signatures: {rust_signatures_path}
  - (For RESTRUCTURED cases, read full Rust source as needed)

For each method in the extract, find the corresponding Rust function(s).
If there is no corresponding Rust code, classify why:

  - MAPPED: Direct Rust equivalent exists
  - MISSING: Should exist but doesn't (gap in reimplementation)
  - INTENTIONAL: Excluded by feature contract (cite decision number)
  - RESTRUCTURED: Behavior exists but distributed differently in Rust

Output format (one entry per C++ method):

```yaml
- cpp_method: <class>::<method>
  cpp_file: <path>
  cpp_lines: <start>-<end>
  rust_fn: <module::function> or [list if split across multiple]
  rust_file: <path>
  rust_lines: <start>-<end>
  status: MAPPED | MISSING | INTENTIONAL | RESTRUCTURED
  notes: >
    If INTENTIONAL: cite the contract decision by number.
    If RESTRUCTURED: list ALL Rust functions that jointly implement it
    and explain how the behavior was redistributed.
```

Also note unmapped Rust code — functions in zuicchini that have no C++
origin (tile_cache, compositor, font_cache, app.rs, tga.rs). List them
briefly but do not audit them; they are new infrastructure, not ports.

Write output to: {output_path}
````

### 7.2 Quality Gate

- Every public method in the extract has a status.
- Every INTENTIONAL classification cites a specific decision number
  from `EMCORE_FEATURE_CONTRACT.md`.
- No status field is empty or "UNKNOWN".

---

## 8. Phase 3 — VERIFY (Spec + Rust → Verdicts)

**Purpose:** Check whether Rust code satisfies every requirement in the
behavioral spec. Agent receives spec and Rust source. Agent does NOT
see raw C++.

**Input:** Extract YAML + Rust source + intentional divergence filter.
**Output:** `_verify/verdicts/{cpp_filename}.yaml`

### 8.1 Intentional Divergence Filter

Before acting on any finding, check against **both** the feature contract
and any `resolution: KEEP` items in the convergence plan. These are NOT gaps:

**Feature contract decisions:**

| Contract Decision | What It Covers |
|---|---|
| Decision #2: CPU raster + wgpu compositor | No `emViewRenderer`, no `emRenderThreadPool` equivalent |
| Decision #3: Typed singletons + ResourceCache | No `dyn Any` service locator, no `Acquire(ctx, typeid, name)` |
| Decision #4: Arena + PanelId handles | No raw parent/child pointers, no `emCrossPtr` in panel tree |
| Decision #5: `Rc` everywhere, no `Arc` | Single-threaded model domain |
| Decision #6: KDL replaces emRec | No emRec parser, different serialization format |
| winit replaces platform backends | No `emX11WindowPort`, no `emWndsWindowPort` |
| Tile cache is new | No C++ equivalent to verify against |

**Convergence plan KEEP items:**

Any item in `CONVERGENCE_PLAN.yaml` with `resolution: KEEP` is an
intentional divergence. Cite by plan item ID (e.g., "KEEP per
PAINTER-FONT in convergence plan").

**Per-subsystem scoping:** Inject ONLY relevant decisions into each agent:

| Subsystem | Relevant Decisions |
|---|---|
| scheduler | (none — direct port) |
| model, context | Decision #3, Decision #6 |
| panel, view | Decision #4, winit, + any KEEP items from plan phases 2 |
| render/painter | Decision #2, + any KEEP items from plan phase 4 |
| layout | (none — direct port) + any KEEP items from plan phase 3 |
| render/compositor, tile_cache | Tile cache is new |
| window, screen | winit |
| widgets | + any KEEP items from plan phase 6 |

### 8.2 Agent Prompt Template (Tier 1: Per-Method)

````
You are verifying a Rust reimplementation against a behavioral
specification extracted from C++ source code. You will NOT see the
original C++ code.

The specification describes what the code MUST do. Your job is to check
whether the Rust code satisfies every requirement in the spec.

IMPORTANT: Some behaviors were intentionally changed in the Rust
reimplementation. The following are NOT gaps — do not flag them:
{intentional_divergences_for_this_subsystem}

SPECIFICATION (from C++ behavioral extract):
{extract_for_this_method}

RUST CODE:
{rust_source_for_corresponding_functions}

For each branch in the specification, determine:

```yaml
- branch: <the condition from the spec>
  rust_handles: YES | NO | PARTIAL
  evidence: >
    Quote the specific Rust line(s) that handle this, with file:line.
    "Evidence" means a line number and code snippet, not a general
    statement like "the code handles this." If you cannot find the
    line, state what is missing.
  severity: CORRECT | COSMETIC | BEHAVIORAL | CRITICAL
```

Severity definitions:
  - CORRECT: Rust matches spec
  - COSMETIC: Differs in naming/style but behavior is equivalent
  - BEHAVIORAL: Logic diverges in a way that changes output for some inputs
  - CRITICAL: Branch is entirely missing or inverted

For each constant in the specification:
```yaml
- constant: <name>
  spec_value: <from extract>
  rust_value: <from Rust code, or MISSING>
  match: YES | NO | APPROXIMATE
```

For each edge case in the specification:
```yaml
- edge_case: <description>
  rust_handles: YES | NO
  evidence: <how, or what is missing>
```

IMPORTANT RULES:
- Do NOT assume the Rust code is correct. Your default stance is skeptical.
- If you cannot find clear evidence that a branch is handled, mark it NO.
- When uncertain between CORRECT and BEHAVIORAL, choose BEHAVIORAL.
- Every YES must include a file:line reference and code snippet.

Compact shorthand: when ALL branches, constants, and edge cases for
a method are CORRECT, emit:
```yaml
- method: <name>
  all_correct: true
```

Write output to: {output_path}
````

### 8.3 Agent Prompt Template (Tier 2: Per-File)

Same as Tier 1 but with this preamble change:

```
SPECIFICATION (from C++ behavioral extract — all methods from {cpp_filename}):
{full_file_extract}

RUST CODE (corresponding file):
{full_rust_file}
```

All methods from one C++ source file are verified in a single agent
invocation. The agent must still produce per-method verdicts.

### 8.4 Agent Prompt Template (Tier 3: API Coverage Check)

Tier 3 skips the full pipeline. One agent compares public APIs.

````
Compare the public API of a C++ emCore class against its Rust
reimplementation. You are checking for missing public methods, not
verifying internal logic.

C++ HEADER (public methods only):
{header_contents}

RUST PUBLIC FUNCTIONS (pre-extracted via grep):
{rust_pub_fn_listing}

For each C++ public method, report:

```yaml
- method: <class>::<method>
  rust_equivalent: <function name> | MISSING | INTENTIONAL
  notes: <brief note if naming differs, or cite decision for INTENTIONAL>
```

Only flag MISSING methods that represent real functionality gaps.
Do not flag methods covered by these intentional divergences:
{relevant_divergences}

Write output to: {output_path}
````

### 8.5 Severity Aggregation

- Any **CRITICAL** → immediate fix candidate
- **BEHAVIORAL** → review queue (check against intentional divergence filter first)
- **COSMETIC** → log but do not act
- **CORRECT** → verified, record for progress tracking

### 8.6 Quality Gate

- Every MAPPED method from Phase 2 has a verdict.
- Every CRITICAL/BEHAVIORAL finding includes the spec branch AND
  Rust evidence (or explicit absence statement).
- No verdict uses phrases like "probably handles" or "likely correct"
  without a file:line reference.

---

## 9. Phase 4 — FIX (Verdicts + Plan → Code Changes → Re-Verify)

**Purpose:** Fix gaps found in Phase 3 and implement resolutions from
the convergence plan, then prove correctness via a fresh Phase 3 re-run.
The agent that writes fixes never judges whether they worked.

### 9.1 Fix Categories

| Category | Description | Autonomy |
|---|---|---|
| **BUG** | Wrong behavior in existing Rust code | Fully autonomous |
| **MISSING_BRANCH** | Method exists but a conditional path is absent | Fully autonomous |
| **MISSING_METHOD** | C++ method with no Rust counterpart | Autonomous (port from extract) |
| **DESIGN** | Behavioral divergence that may be intentional | **Resolved by convergence plan** |

### 9.2 Sub-Phase 4a: TRIAGE

**Input:** All verdict files, all mapping files, **convergence plan**.
**Output:** `_fix/triage.yaml`, `_fix/design_decisions.yaml` (updated)

````
You are triaging verification findings for the zuicchini Rust
reimplementation. Classify every finding and group by Rust module.

Read ALL verdict files: _verify/verdicts/*.yaml
Read ALL mapping files: _verify/mappings/*.yaml
Read convergence plan: CONVERGENCE_PLAN.yaml

CLASSIFICATION WITH PLAN INTEGRATION:

For every CRITICAL or BEHAVIORAL finding, and every MISSING mapping:

1. Is this finding covered by a plan item with `resolution: KEEP`?
   YES → INTENTIONAL (cite plan item ID). Skip.

2. Is this finding covered by a plan item with `resolution: PORT`
   or `resolution: PORT:<instructions>`?
   YES → Use the plan resolution to determine category:
   - Method exists with wrong behavior → BUG
   - Method exists but branch is missing → MISSING_BRANCH
   - Method doesn't exist → MISSING_METHOD
   The `resolution` field provides the authoritative fix direction.

3. Is this finding covered by a plan item with `resolution: DEFER`
   or empty resolution?
   YES → DESIGN (deferred). Do not process.

4. Not covered by any plan item?
   Apply the standard decision tree:
   a. Method exists?
      NO → Does existing Rust code reference or need it?
            YES → MISSING_METHOD
            NO  → DESIGN (skip_reason: "no callers; needs human decision")
   b. Correct behavior unambiguous from the extract alone?
      NO → DESIGN (skip_reason: explain what is ambiguous)
   c. Wrong behavior → BUG. Missing branch → MISSING_BRANCH.

AMBIGUITY SAFEGUARD: Err toward DESIGN. A DESIGN item that turns out
simple wastes one human decision. A BUG item that's actually a design
choice embeds the wrong answer silently.

Output schema:

```yaml
modules:
  - rust_file: <path>
    fixes:
      - id: <module>-<n>
        category: BUG | MISSING_BRANCH | MISSING_METHOD
        source_finding: <verdict file>:<method>:<condition>
        extract_ref: <extract file>:<method>
        plan_item: <plan item ID, if applicable>
        plan_resolution: <resolution text, if applicable>
        dependency: [<ids that must be fixed first>]
        summary: <one line>

design_decisions:
  - id: <module>-D<n>
    source_finding: <verdict or mapping file>:<method>
    skip_reason: <specific explanation>
    plan_status: <"unresolved" | "DEFER" | "not in plan">
    question: <what needs human input>
    options:
      - <option A and consequence>
      - <option B and consequence>

kept_divergences:
  - plan_item: <plan item ID>
    findings_covered:
      - <verdict file>:<method>:<condition>
```

Write triage to: _fix/triage.yaml
Write design decisions to: _fix/design_decisions.yaml

QUALITY GATE: Count CRITICAL + BEHAVIORAL in verdicts. Count entries
in triage fixes + design_decisions + kept_divergences findings_covered.
The counts must match.
````

### 9.3 Sub-Phase 4b: CONTEXT

**Input:** Triage, Rust source, extracts, mappings.
**Output:** `_fix/context/<module>.yaml`
**One agent per Rust module. Read-only — no code changes.**

```
You are analyzing a Rust module to understand its patterns before
any code changes are made. You will NOT modify any files.

Read:
1. Rust source: {rust_file_path}
2. Behavioral extract: {extract_file_path}
3. Mapping file: {mapping_file_path}
4. Triage entries for this module: {triage_entries}

Produce a context document answering:
- Naming conventions?
- Coordinate system / units?
- Ownership pattern (&self, &mut self, owned)?
- State management (struct fields, push/pop)?
- Error handling (Result, Option, panic, silent skip)?
- File organization (method grouping with line ranges)?

For each fix ID:
- Find all call sites (grep for function name across src/)
- Find all methods the function calls
- Note what the fixing agent needs to produce fitting code

For MISSING_METHOD: where should it go? What's the style reference?

Write to: {context_output_path}
```

### 9.4 Sub-Phase 4c: FIX

**Input:** Triage, context, extract, mapping, Rust source, **plan resolutions**.
**Output:** Modified Rust source, `_fix/changes/<module>.yaml`

````
You are fixing verified gaps in the zuicchini Rust reimplementation.

Read:
1. Triage entries: {triage_entries}
2. Context document: {context_file_path}
3. Behavioral extract (your spec): {extract_file_path}
4. Mapping file: {mapping_file_path}
5. Current Rust source: {rust_file_path}

Rules:
- Apply fixes in dependency order from the triage.
- The extract defines correct behavior. Implement it, adapted to
  patterns in the context document.
- If a triage entry has `plan_resolution`, use it as the authoritative
  direction for how to implement the fix. For example:
  - "PORT" → implement the C++ behavior exactly
  - "PORT: use enum not bitfield" → implement C++ behavior but use the
    specified approach
- Do NOT refactor, rename, reformat, add comments, or change code
  outside the scope of listed fixes.
- If a fix depends on an unresolved DESIGN decision, skip it →
  SKIPPED_DESIGN.
- If compilation fails after 3 attempts, revert → BLOCKED.
- If you discover a judgment call during implementation, do NOT
  guess → RECLASSIFIED.

After all fixes: cargo check --workspace
If it fails, fix compilation errors in modified files only.

Change log schema:

```yaml
module: <rust file>
changes:
  - fix_id: <from triage>
    plan_item: <plan item ID, if applicable>
    action: MODIFIED | ADDED | REPLACED | SKIPPED_DESIGN | BLOCKED | RECLASSIFIED
    lines_before: <start>-<end>
    lines_after: <start>-<end>
    description: <what changed and why>
    extract_branches_addressed:
      - <branch conditions this fix satisfies>
```

Write change log to: {changes_output_path}
````

### 9.5 Sub-Phase 4d: RE-VERIFY

**A fresh Phase 3 agent judges the fixes. It does NOT see the change
log, fix descriptions, or any Phase 4 output.**

Scope (determined by orchestrator, not the agent):
1. Every method modified or added in 4c
2. Every method in the same file that was previously CORRECT (regression)
3. Every method that calls or is called by a changed method (integration)

Before re-verifying, snapshot previous verdicts:
```bash
cp _verify/verdicts/{file}.yaml _fix/baselines/{file}.round{N}.yaml
```

**Regression gate:** After re-verification, compare against baseline.
CRITICAL count must not increase. BEHAVIORAL count must not increase.
If either increases, the offending fix must be reverted.

### 9.6 Sub-Phase 4e: STATUS UPDATE

After re-verification, update the convergence plan:

```
For each plan item that was processed this run:
- If re-verification shows all addressed branches are now CORRECT
  and no regressions: set status: VERIFIED
- If the fix was applied but re-verification found issues:
  set status: FAILED
- If the fix was blocked or skipped: set status: BLOCKED

Write the updated CONVERGENCE_PLAN.yaml.
```

This closes the loop: the user can see at a glance which items
succeeded and which need attention.

### 9.7 Sub-Phase 4f: HUMAN REVIEW

Present all items requiring human attention as a single batch:
- RECLASSIFIED fixes from 4c (implementation revealed a judgment call)
- BLOCKED fixes from 4c that are NOT blocked by plan dependencies
  (compilation failures, unexpected interactions)
- DESIGN items from triage step 4 that are not covered by any plan item
  (candidates for new plan items in the next convergence cycle)

Human annotates each. RECLASSIFIED items become new plan items.
DESIGN items may be added to the plan with a resolution for the next run.
BLOCKED items are investigated and either fixed manually or deferred.

---

## 10. Session Management

### 10.1 Incremental Execution

The convergence plan enables divide-and-conquer. The orchestrator
follows this protocol:

1. **Read the plan.** Find all items with non-empty resolution.
2. **Filter by phase.** If the user has only resolved items in one
   plan phase, process only those items.
3. **Check dependencies.** If a resolved item depends on an unresolved
   item (via `depends_on`), mark it BLOCKED and skip.
4. **Run only necessary pipeline phases:**
   - Items with `needs_extract: true` → Phase 1, then Phase 2, then Phase 4
   - Items with existing extracts → Phase 4 only
   - Items with `resolution: KEEP` → Update Phase 3 filter only (no code changes).
     Annotate existing verdict entries for affected methods with
     `superseded_by: <plan item ID>` so they are excluded from
     BEHAVIORAL/CRITICAL counts. Do not delete the old verdicts —
     they document what was accepted.
5. **Update the plan.** Set `status` on each processed item.

### 10.2 Single-Session Workflow

For small batches (a few items from one plan phase), the full
pipeline can run in one session:

```
Survey → (Extract if needed) → (Map if needed) → Triage → Context → Fix → Re-verify → Status Update → Human Review
```

### 10.3 Multi-Session Workflow

For large batches (e.g., all of painter convergence), split:

**Session 1:** Phase 0 (survey) + Phase 1 (extract new C++ methods)
**Session 2:** Phase 2 (map) + Phase 4a-b (triage + context)
**Session 3:** Phase 4c (fix)
**Session 4:** Phase 4d-f (re-verify + status update + human review)

### 10.4 Resumption Protocol

At the start of any session, scan for existing state:

```
1. Read CONVERGENCE_PLAN.yaml → which items are resolved, which have status
2. List _verify/extracts/*.yaml → which C++ files have been extracted
3. List _verify/mappings/*.yaml → which subsystems are mapped
4. List _verify/verdicts/*.yaml → which files have verdicts
5. Check _fix/triage.yaml → whether triage has been done
6. Check _fix/changes/*.yaml → which modules have been fixed
```

Resume from the earliest incomplete phase. Never re-extract unless
the extract itself was wrong. Never re-map unless Rust files were
structurally reorganized. Skip items that already have `status: VERIFIED`.

### 10.5 Context Window Budgets

| Phase | Input Size Budget | Strategy |
|---|---|---|
| Phase 0 (Survey) | ~200 lines plan + file list | Fits easily |
| Phase 1 (Extract) | ~1,500 LOC C++ per chunk | Chunk large files |
| Phase 2 (Map) | Extract YAML + signature listing | Compact inputs |
| Phase 3 (Verify T1) | Method extract + ~500 LOC Rust | Per-method keeps it small |
| Phase 3 (Verify T2) | File extract + full Rust file | Budget ~3,000 lines total |
| Phase 4 (Fix) | Triage + context + extract + source | May need chunking for painter |

### 10.6 Parallelism

- Phase 1: Up to 3 extract agents in parallel (one per C++ file)
- Phase 2: Sequential (needs prior extracts)
- Phase 3: Up to 3 verify agents in parallel
- Phase 4c: Parallel for independent modules, sequential for cross-deps

---

## 11. Anti-Pattern Catalogue

Specific things the LLM must not do, with examples of each.

### 11.1 "Looks Similar" Verdicts

**Wrong:**
```yaml
- branch: "if alpha == 0, skip blending"
  rust_handles: YES
  evidence: "The blending code appears to handle the zero-alpha case"
  severity: CORRECT
```

**Right:**
```yaml
- branch: "if alpha == 0, skip blending"
  rust_handles: YES
  evidence: "painter.rs:847 `if alpha == 0 { return; }` — early return before blend loop"
  severity: CORRECT
```

The difference: specific file:line reference vs. vague "appears to."

### 11.2 Invented Justifications

**Wrong:**
```yaml
- status: INTENTIONAL
  notes: "This was likely removed because Rust's type system makes it unnecessary"
```

**Right:**
```yaml
- status: INTENTIONAL
  notes: "Decision #4 (Arena + PanelId handles) — raw parent pointers replaced by PanelId lookup"
```

Or, for plan-based divergences:

```yaml
- status: INTENTIONAL
  notes: "KEEP per PAINTER-FONT in convergence plan — TTF font system is intentional"
```

The difference: cites a specific contract decision or plan item vs. invents a reason.

### 11.3 Scope Creep During Fixes

**Wrong:** "While fixing the missing clamp, I also refactored the
surrounding function to use more idiomatic Rust patterns and added
documentation."

**Right:** Add the clamp. Touch nothing else.

### 11.4 Bulk "All Correct" Without Evidence

**Wrong:**
```yaml
- method: emPainter::PaintRect
  all_correct: true
```
(for a method with 12 branches, 4 constants, and 3 edge cases)

**Right:** `all_correct: true` is only valid after explicitly checking
every branch. For complex methods, expand the full format and show
evidence for at least the non-trivial branches.

### 11.5 Re-verifying Your Own Fixes

**Wrong:** The agent that wrote a fix also writes the verdict for
that fix, concluding "the fix is correct."

**Right:** A separate Phase 3 agent that never saw the change log
produces a fresh verdict based only on spec + current code.

### 11.6 Treating RESTRUCTURED as CORRECT

**Wrong:**
```yaml
- status: RESTRUCTURED
  notes: "Behavior was reorganized across multiple Rust functions"
```
(with no further verification)

**Right:** RESTRUCTURED triggers expanded verification. List every
Rust function involved. Trace every C++ branch across all of them.
A branch that exists nowhere in the Rust functions is MISSING, not
"reorganized away."

### 11.7 Ignoring Chunking Needs

**Wrong:** Cramming all 3,474 lines of `painter.rs` + a 500-line
extract into one context and producing shallow analysis.

**Right:** Split into 4 chunks per the chunking strategy (Section 4.4).
Each chunk gets focused analysis within the context budget.

### 11.8 Conflating Missing Callers with Intentional Omission

**Wrong:** "No Rust code calls this method, so it must be intentionally
omitted." → Marked INTENTIONAL with no contract citation.

**Right:** Mark as DESIGN with `skip_reason: "no callers in current
Rust codebase; needs human decision on whether to port."` Only mark
INTENTIONAL if a specific contract decision or plan KEEP covers it.

### 11.9 Ignoring Plan Resolutions

**Wrong:** A plan item has `resolution: PORT` but the fix agent
decides "the Rust approach is better" and skips it.

**Right:** The plan resolution is authoritative. If the human said
PORT, implement the C++ behavior. The fix agent does not second-guess
resolved design decisions. If implementation reveals a genuine
impossibility, mark RECLASSIFIED with an explanation — do not
silently keep the old behavior.

---

## 12. Example Walkthrough

A concrete example showing one plan item flowing through the pipeline.

### Plan Item (from CONVERGENCE_PLAN.yaml)

```yaml
- id: D-LAYOUT-05
  summary: "Pack split search simplified (single midpoint vs multi-point)"
  resolution: "PORT"
```

### Phase 0 Survey

```
D-LAYOUT-05: resolved PORT, extract exists (emPackLayout.yaml),
  mapping exists (layout.yaml), no dependencies. → Ready for Phase 4.
```

### Phase 4a Triage

```yaml
- id: LAYOUT-PACK-SPLIT
  category: BUG
  source_finding: emPackLayout.yaml:split search
  plan_item: D-LAYOUT-05
  plan_resolution: PORT
  summary: Single midpoint search instead of multi-point for 8-20 items
```

### Phase 4c Fix (change log)

```yaml
- fix_id: LAYOUT-PACK-SPLIT
  plan_item: D-LAYOUT-05
  action: MODIFIED
  lines_before: 120-135
  lines_after: 120-155
  description: >
    Added multi-point split search for 8-20 items. Tests 11 points
    for 8 items, linearly decreasing to 2 for 20 items, using
    alternating pattern around midpoint.
  extract_branches_addressed:
    - "8-20 items: test multiple split points around midpoint"
```

### Phase 4d Re-verify

Fresh agent verifies pack layout against extract. Confirms multi-point
search is now present.

### Phase 4e Status Update

```yaml
- id: D-LAYOUT-05
  resolution: "PORT"
  status: VERIFIED
```

---

## 13. Quick Reference

### Commands

```bash
# Setup
mkdir -p _verify/{extracts,mappings,verdicts,rust_signatures}
mkdir -p _fix/{context,changes,baselines}

# Generate signature indices
for dir in foundation input layout model panel render scheduler widget window; do
  grep -rn 'pub fn\|pub(crate) fn\|fn ' src/$dir/ > _verify/rust_signatures/$dir.txt 2>/dev/null
done

# Quality gate: extract completeness
grep -c 'if\|else\|switch\|case' {cpp_source} # → lower bound
grep -c 'condition:' _verify/extracts/{file}.yaml # → must meet bound

# Find actionable findings
grep -rl 'CRITICAL\|BEHAVIORAL' _verify/verdicts/

# Count findings for triage gate
grep -c 'severity: CRITICAL\|severity: BEHAVIORAL' _verify/verdicts/*.yaml

# Plan: show resolved items
grep -A1 'resolution:' CONVERGENCE_PLAN.yaml | grep -v '^--$'

# Plan: show completed items
grep 'status: VERIFIED' CONVERGENCE_PLAN.yaml
```

### Agent Naming Convention

```
Survey:     "Survey convergence plan state"
Extract:    "Extract emListBox.cpp behavioral spec"
Match:      "Map widget subsystem C++ to Rust"
Verify T1:  "Verify PaintImage fidelity"
Verify T2:  "Verify emBorder.cpp fidelity"
API Check:  "API coverage check emColor.cpp"
Triage:     "Triage findings with plan resolutions"
Context:    "Context for render/painter.rs"
Fix:        "Fix render/painter.rs (D-RENDER-07, D-RENDER-06)"
Re-verify:  "Re-verify emPainter.cpp verdicts"
Status:     "Update convergence plan status"
```

### Execution Order (Recommended)

```
For each plan phase with resolved items:

  1. Survey: scan plan, check artifacts, determine scope
  2. Extract: only for needs_extract items (parallel, up to 3)
  3. Map: only for new extracts (sequential)
  4. Triage: classify resolved items as BUG/MISSING_BRANCH/MISSING_METHOD
  5. Context: one agent per affected Rust module (parallel)
  6. Fix: apply changes (parallel for independent modules)
  7. Re-verify: fresh Phase 3 agent per affected verdict file
  8. Status update: write VERIFIED/FAILED/BLOCKED back to plan
  9. Human review: surface RECLASSIFIED, BLOCKED, and uncovered DESIGN items
```
