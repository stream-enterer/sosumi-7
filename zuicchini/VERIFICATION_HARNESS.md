# Reimplementation Verification Harness

> Systematic verification of zuicchini (Rust) against emCore (C++) source.
> Designed to catch the gaps that impressionistic LLM review misses.

---

## Design Principles

This harness composes six agentic patterns into a pipeline that forces
branch-level comparison while resisting confirmation bias.

| Pattern | Role in Harness |
|---|---|
| **Discrete Phase Separation** | Extract C++ behavior in one phase; evaluate Rust in a separate phase with no raw C++ visible |
| **LLM Map-Reduce** | Each C++ file is verified in isolation — contamination between files is impossible |
| **Opponent Processor** | The behavioral extract becomes the adversarial spec the Rust code must satisfy |
| **Structured Output** | All findings use a fixed schema — vague "looks good" is structurally impossible |
| **Immutable Contract** | The feature contract marks intentional divergences; the harness cannot override them |
| **Filesystem State** | Progress persists in `_verify/` across sessions; work is incremental and resumable |

---

## Paths

```
C++ source:    ~/.local/git/eaglemode-0.96.4/src/emCore/
C++ headers:   ~/.local/git/eaglemode-0.96.4/include/emCore/
Rust source:   zuicchini/src/
Contract:      zuicchini/EMCORE_FEATURE_CONTRACT.md
Verify state:  zuicchini/_verify/
```

---

## Subsystem Risk Tiers

Verification agents spend time proportional to risk. Each tier uses a
different depth of analysis to avoid wasting tokens on low-risk code.

| Tier | Depth | Subsystem | Why |
|---|---|---|---|
| **1** | Full 3-phase, per-method | `emPainter*.cpp` → `render/painter.rs` | Rasterization math: off-by-one, blending formula, sub-pixel AA, coordinate transforms |
| **1** | Full 3-phase, per-method | `emScheduler.cpp`, `emEngine.cpp`, `emSignal.cpp` → `scheduler/` | Signal chaining order, priority queues, time-slice fairness, wake-up parity |
| **1** | Full 3-phase, per-method | `emView.cpp`, `emViewAnimator.cpp`, `emViewInputFilter.cpp` → `panel/view.rs`, `panel/animator.rs`, `panel/input_filter.rs` | Navigation math, input propagation order, animator physics |
| **2** | Full 3-phase, per-file | `emPanel.cpp` → `panel/tree.rs`, `panel/ctx.rs` | Panel lifecycle, notice flags, coordinate invariants, focus tracking |
| **2** | Full 3-phase, per-file | `emBorder.cpp` → `widget/border.rs` | Border metrics, content rect calculation, look application |
| **2** | Full 3-phase, per-file | Layout files (`emLinearLayout`, `emRasterLayout`, `emPackLayout`) → `layout/` | Spacing arithmetic, orientation logic, weight distribution |
| **2** | Full 3-phase, per-file | `emTextField.cpp` → `widget/text_field.rs` | Cursor movement, selection, undo/redo, clipboard, scroll |
| **3** | API coverage only | `emColor.cpp`, `emImage.cpp` → `foundation/` | Well-defined types, less algorithmic complexity |
| **3** | API coverage only | Simple widgets (`emLabel`, `emCheckBox`, `emRadioBox`) | Thin wrappers with little logic |
| **3** | API coverage only | `emWindow.cpp`, `emScreen.cpp` → `window/` | Mostly winit delegation, not direct ports |

**Tier depth differences:**
- **Tier 1:** Extract includes `calls`, `state_mutations`. Verify runs per-method.
- **Tier 2:** Extract omits `calls` and `state_mutations`. Verify runs per-file (all methods in one agent).
- **Tier 3:** Skips the 3-phase pipeline entirely. Uses the lightweight API Coverage Check (see below).

---

## The Three Phases

### Phase 1: EXTRACT — Read C++, Produce Behavioral Spec

**One agent per C++ file. Agent sees ONLY the C++ source. Agent does NOT see any Rust code.**

The agent reads the `.cpp` file and its corresponding `.h` header, then
produces a **behavioral extract** — a structured description of what the
code does at the branch level.

**Large files:** Some emCore files exceed what fits in a single context
(notably `emPainter.cpp` at ~8000 lines, `emBorder.cpp` at ~4000 lines,
`emView.cpp` at ~3000 lines). For these, the agent processes the file in
sequential chunks (by method or logical section), appending to the same
extract file. The agent MUST read the header first to get the full method
list, then work through the source systematically.

**Multi-file units:** The painter is split across `emPainter.cpp` and
several `emPainter_ScTl*.cpp` files. These form a single logical unit.
Extract each file separately, but the Match phase (Phase 2) treats them
as one subsystem when linking to `render/painter.rs`.

**Agent prompt skeleton:**

```
You are analyzing C++ source code from Eagle Mode's emCore library.
Your job is to produce a precise behavioral extract. You will NOT see
or evaluate any Rust code. Focus exclusively on what this C++ code does.

Read the following files:
  - Header: {header_path}
  - Source: {source_path}

For each public method and each non-trivial private method, produce
the following YAML structure. A "trivial" method is one whose entire
body is a single return of a member variable with no conditionals.
Everything else must be extracted.

- method: <class>::<method_name>
  signature: <return_type>(<params>)
  lines: <start>-<end>
  behavior: <1-3 sentence description of what it does>
  calls: [<list of same-class methods this method calls>]        # tier 1 only — orchestrator omits for tier 2
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
  state_mutations:                                                # tier 1 only — orchestrator omits for tier 2
    - <what member variables or globals are modified and how>

Include ALL conditional branches — even single-line early returns,
ternary operators, and short-circuit evaluations that guard side
effects. If a branch is guarded by a flag check, record the flag
name and what triggers it.

Do NOT summarize or skip "obvious" code. The purpose of this extract
is to serve as a verification spec. Every branch you omit is a branch
that will go unverified.

Write the output as valid YAML to the file path provided by the
orchestrator.
```

**Output:** One YAML file per C++ source file, saved to `_verify/extracts/{filename}.yaml`

**Quality gate:** The extract must list at least as many branches as
there are `if`/`else`/`switch`/`case`/`?` keywords in the source
(approximate lower bound via grep). This is a sanity check, not an
exact measure — ternaries and short-circuits may push the real count
higher.

---

### Phase 2: MATCH — Link Extracts to Rust Code

**One agent per subsystem maps extracted methods to their Rust counterparts.**
For small subsystems (scheduler, input), one agent handles all files.
For large subsystems (painter, view), split by file to stay within context.

This phase produces the traceability index. It also identifies:
- **Unmapped methods**: C++ methods with no Rust equivalent (potential gaps)
- **Intentional omissions**: Methods excluded by the feature contract
- **Structural reshaping**: Where one C++ method maps to multiple Rust
  functions or vice versa

**Agent prompt skeleton:**

```
You have a behavioral extract from C++ emCore and the Rust zuicchini source.
Your job is to create a mapping between them.

For each method in the extract, find the corresponding Rust function(s).
If there is no corresponding Rust code, classify why:

  - MISSING: Should exist but doesn't (gap in reimplementation)
  - INTENTIONAL: Excluded by feature contract decision (cite which decision)
  - RESTRUCTURED: Behavior exists but is distributed differently in Rust

Output format:

  cpp_method: <class>::<method>
  cpp_file: <path>
  cpp_lines: <start>-<end>
  rust_fn: <module::function> or [list if split across multiple]
  rust_file: <path>
  rust_lines: <start>-<end>
  status: MAPPED | MISSING | INTENTIONAL | RESTRUCTURED
  notes: <if INTENTIONAL, cite the contract decision by number>
         <if RESTRUCTURED, explain how the behavior was redistributed
          and list ALL Rust functions that jointly implement it>
```

**Output:** `_verify/mappings/{subsystem}.yaml`

**Input optimization:** Before launching the match agent, the
orchestrator generates a Rust signature index:
```bash
grep -rn 'pub fn\|pub(crate) fn\|fn ' zuicchini/src/{subsystem}/ > _verify/rust_signatures/{subsystem}.txt
```
The match agent receives the extract + this signature listing rather than
full Rust source files. It only reads full Rust source for RESTRUCTURED
cases where it needs to trace how behavior was redistributed.

**This phase also flags unmapped Rust code** — functions in zuicchini
that have no C++ origin. These are new infrastructure (tile_cache,
compositor, font_cache, app.rs, tga.rs) — note them briefly in the
mapping file but don't audit them; they're not ports.

---

### Phase 3: VERIFY — Compare Extract Against Rust (No Raw C++)

**One agent per mapped method (tier 1) or per C++ source file (tier 2).
Tier 3 does not use Phase 3 — see API Coverage Check below.
Agent receives the behavioral extract and the Rust source.
Agent does NOT see the raw C++ code.**

This is the anti-confirmation-bias mechanism. The agent works from the
behavioral description (Phase 1 output), not the C++ syntax. It cannot
pattern-match curly braces or variable names — it must verify whether
the Rust code satisfies each behavioral claim.

**Handling RESTRUCTURED methods:** When a C++ method maps to multiple
Rust functions, the agent receives ALL the Rust functions listed in the
mapping and verifies that their combined behavior satisfies the spec.
The agent must trace how the C++ method's branches are distributed
across the Rust functions.

**Agent prompt skeleton:**

```
You are verifying a Rust reimplementation against a behavioral specification
extracted from C++ source code. You will NOT see the original C++ code.

The specification describes what the code MUST do. Your job is to check
whether the Rust code satisfies every requirement in the spec.

IMPORTANT: Some behaviors were intentionally changed in the Rust
reimplementation. The following are NOT gaps — do not flag them:
{intentional_divergences_relevant_to_this_subsystem}

SPECIFICATION (from C++ behavioral extract):
{extract_for_this_unit}
  (Tier 1: one method's extract. Tier 2: all methods from one file's extract.)

RUST CODE:
{rust_source_for_corresponding_functions}

For each branch in the specification, determine:

  branch: <the condition from the spec>
  rust_handles: YES | NO | PARTIAL
  evidence: <quote the specific Rust line(s) that handle this, or state
             what is missing. "Evidence" means a line number and code
             snippet, not a general statement.>
  severity: CORRECT | COSMETIC | BEHAVIORAL | CRITICAL
    - CORRECT: Rust matches spec
    - COSMETIC: Differs in naming/style but behavior is equivalent
    - BEHAVIORAL: Logic diverges in a way that changes output for some inputs
    - CRITICAL: Branch is entirely missing or inverted

For each constant in the specification:
  constant: <name>
  spec_value: <from extract>
  rust_value: <from Rust code, or MISSING>
  match: YES | NO | APPROXIMATE

For each edge case in the specification:
  edge_case: <description>
  rust_handles: YES | NO
  evidence: <how, or what's missing>

Do NOT assume the Rust code is correct. Your default stance is skeptical.
If you cannot find clear evidence that a branch is handled, mark it NO.
When uncertain between CORRECT and BEHAVIORAL, choose BEHAVIORAL.
False negatives (missing a real gap) are worse than false positives
(flagging a non-gap that turns out to be fine).

Write the output as valid YAML to the file path provided by the
orchestrator.
```

**Output:** `_verify/verdicts/{cpp_filename}.yaml` (one file per C++ source,
methods are sections within it)

**Severity aggregation:**
- Any CRITICAL finding → immediate fix candidate
- BEHAVIORAL findings → review queue (check against intentional divergence filter)
- COSMETIC → log but do not act
- CORRECT → verified, record for progress tracking

**Compact CORRECT shorthand:** When ALL branches, constants, and edge
cases for a method are CORRECT, emit a single line instead of the full
per-branch structure:

```yaml
- method: emTimer::Stop
  all_correct: true
```

Only expand to the full per-branch format when at least one item is
not CORRECT. This saves significant output tokens on well-implemented
methods (which should be the majority).

---

### Tier 3 Shortcut: API Coverage Check

Tier 3 files skip the full pipeline. A single agent compares the C++
header's public API against the Rust module's public API in one pass.

**Agent prompt skeleton:**

```
Compare the public API of a C++ emCore class against its Rust
reimplementation. You are checking for missing public methods, not
verifying internal logic.

C++ HEADER (public methods only):
{header_contents}

RUST PUBLIC FUNCTIONS (pre-extracted via grep):
{rust_pub_fn_listing}

For each C++ public method, report:
  method: <class>::<method>
  rust_equivalent: <function name> | MISSING | INTENTIONAL
  notes: <brief note if naming differs or if INTENTIONAL, cite decision>

Only flag MISSING methods that represent real functionality gaps.
Do not flag methods covered by the intentional divergence decisions:
{relevant_divergences}
```

**Input optimization:** The Rust side is provided as a pre-extracted
signature listing (`grep -n 'pub fn' file.rs`), not the full source.
This dramatically reduces input tokens for the agent.

**Output:** `_verify/verdicts/{cpp_filename}_api.yaml`

---

## Intentional Divergence Filter

Before acting on any finding, check it against the feature contract.
These are NOT gaps:

| Contract Decision | What It Covers |
|---|---|
| Decision #2: CPU raster + wgpu compositor | No `emViewRenderer`, no `emRenderThreadPool` equivalent |
| Decision #3: Typed singletons + ResourceCache | No `dyn Any` service locator, no `Acquire(ctx, typeid, name)` |
| Decision #4: Arena + PanelId handles | No raw parent/child pointers, no `emCrossPtr` in panel tree |
| Decision #5: `Rc` everywhere, no `Arc` | Single-threaded model domain |
| Decision #6: KDL replaces emRec | No emRec parser, different serialization format |
| winit replaces platform backends | No `emX11WindowPort`, no `emWndsWindowPort` |
| Tile cache is new | No C++ equivalent to verify against |

A finding that flags the absence of `emContext::Acquire()` is not a gap —
it's the typed-singleton decision working as intended.

**Per-subsystem scoping:** The orchestrator injects ONLY the 1-2 relevant
decisions into each verify agent's prompt, not the full table. Mapping:

| Subsystem | Relevant Decisions |
|---|---|
| scheduler | (none — direct port) |
| model, context | Decision #3, Decision #6 |
| panel, view | Decision #4, winit |
| render/painter | Decision #2 |
| render/compositor, tile_cache | Tile cache is new |
| window, screen | winit |
| widgets | (none — direct port) |

---

## State & Progress Tracking

The `_verify/` directory is generated state, not source. Add it to
`.gitignore`. Its contents are reproducible by re-running the harness.

```
_verify/
  extracts/          # Phase 1 output: one .yaml per C++ file
  mappings/          # Phase 2 output: one .yaml per subsystem
  verdicts/          # Phase 3 output: one .yaml per C++ file
  rust_signatures/   # Pre-computed grep output for Phase 2
```

**Progress is implicit.** File existence is the progress tracker:
- `extracts/emScheduler.yaml` exists → Phase 1 done for that file
- `mappings/scheduler.yaml` exists → Phase 2 done for that subsystem
- `verdicts/emScheduler.yaml` exists → Phase 3 done for that file

No separate `progress.yaml`. To find what needs work: check which
tier-1 files lack a verdict. To find actionable findings:
`grep -l 'CRITICAL\|BEHAVIORAL' _verify/verdicts/*.yaml`

---

## Execution Model

The harness runs as a series of Claude Code agent delegations:

1. **Orchestrator** checks which files lack verdicts, selects the next file(s) to verify
2. **Extract agents** (Phase 1) run in parallel, one per C++ file, tier-1 first
3. **Match agent** (Phase 2) runs after extracts complete for a subsystem
4. **Verify agents** (Phase 3) run in parallel, one per method (tier 1) or per file (tier 2)
5. **Orchestrator** spot-checks verdicts for quality, moves to next subsystem

**Parallelism limits:** Run at most 3 extract agents or 3 verify agents
simultaneously. More than that risks context quality degradation in the
orchestrator when collecting results.

**Agent subjects follow the naming convention:**
```
Extract:    "Extract emScheduler.cpp behavioral spec"
Match:      "Map scheduler subsystem C++ to Rust"
Verify T1:  "Verify EngineScheduler::do_time_slice fidelity"
Verify T2:  "Verify emBorder.cpp fidelity"
API Check:  "API coverage check emColor.cpp"
```

**Orchestrator responsibilities between phases:**
- After Phase 1: Run the quality gate (branch count check) before proceeding
- After Phase 2: Review MISSING items — some may warrant immediate attention
  before spending time on Phase 3
- After Phase 3: Grep verdicts for CRITICAL/BEHAVIORAL, check against
  the Intentional Divergence table before acting

---

## Example Finding

A concrete example of what a Phase 3 verdict entry looks like, so
agents produce consistent output:

```yaml
- method: emScheduler::DoTimeSlice
  branches:
    - condition: "if signal is pending and was fired during this same time slice (instant chaining)"
      rust_handles: PARTIAL
      evidence: |
        scheduler/core.rs:142 processes pending signals in a loop, but
        the loop exits after one pass. The C++ spec says signals fired
        during Cycle() wake the target engine within the SAME time slice,
        which requires re-scanning the pending list after each engine
        Cycle() call. The Rust code only scans once.
      severity: BEHAVIORAL
    - condition: "engine priority ordering within time slice"
      rust_handles: YES
      evidence: "scheduler/core.rs:108-120 iterates wake queues in priority order"
      severity: CORRECT
  constants: []
  edge_cases:
    - edge_case: "Signal fired by engine A wakes engine B in same slice"
      rust_handles: NO
      evidence: "Single-pass signal processing means B wakes next slice, not this one"
```

Note: the second branch shows CORRECT inline. If ALL branches, constants,
and edge cases were CORRECT, the entire method collapses to:
```yaml
- method: emScheduler::DoTimeSlice
  all_correct: true
```

This example shows: specific line references, quoted Rust code location,
a clear description of the divergence, and a severity that is
falsifiable (you could write a test for this).

---

## Quality Gates

| Gate | Mechanism |
|---|---|
| Extract completeness | Branch count in extract >= `grep -c 'if\|else\|switch\|case' source.cpp` |
| Mapping coverage | Every public method in extract has a status (MAPPED/MISSING/INTENTIONAL/RESTRUCTURED) |
| Verdict coverage | Every MAPPED method has a verdict |
| No false INTENTIONAL | Every INTENTIONAL classification cites a specific contract decision |
| Finding actionability | Every CRITICAL/BEHAVIORAL finding includes the spec branch AND the Rust evidence (or absence) |

---

## Running a Verification Pass

To verify a single subsystem (e.g., scheduler):

```
1. Extract: Run Phase 1 on emScheduler.cpp, emEngine.cpp, emSignal.cpp, emTimer.cpp
2. Match:   Run Phase 2 linking extracts to zuicchini/src/scheduler/
3. Verify:  Run Phase 3 on all MAPPED methods
4. Review:  grep -l 'CRITICAL\|BEHAVIORAL' _verify/verdicts/emScheduler.yaml etc.
5. Fix:     Address findings, then re-run Phase 3 on affected methods only
```

To verify the full codebase, iterate subsystems in tier order:
Tier 1 (painter, scheduler, view) → Tier 2 (panel, border, layout, text_field) → Tier 3 (rest).

**After fixes:** When a finding is addressed by a code change, re-run
only Phase 3 on the affected methods. Do NOT re-extract (Phase 1) or
re-match (Phase 2) unless the Rust code was structurally reorganized.
Update the verdict file in place and change the severity to CORRECT
with a note referencing the fix commit.

**When to re-extract:** Only if the behavioral extract itself was wrong
or incomplete. Fix the extract, re-run Phase 3 for that file.
