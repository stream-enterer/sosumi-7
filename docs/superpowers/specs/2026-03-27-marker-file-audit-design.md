# Marker File Correspondence Audit

**Date:** 2026-03-27
**Goal:** Advanced evidence gathering on all marker files in `src/emCore/` to document what each C++ type does, where equivalent functionality lives in Rust (or doesn't), and surface open questions for human review.

## Scope

20 marker files in `src/emCore/`:

- **15 `no_rust_equivalent`:** emAnything, emArray, emAvlTree, emAvlTreeMap, emAvlTreeSet, emCrossPtr, emFileStream, emList, emOwnPtr, emOwnPtrArray, emRef, emString, emThread, emTmpFile, emToolkit
- **5 `rust_only`:** emPainterDrawList, fixed, rect, toolkit_images, widget_utils

## Core Principle: Evidence, Not Interpretation

Agents produce **factual evidence** for human review. They do not classify, judge, or recommend. Specifically:

- **No classification labels.** No "stdlib-replacement", "necessary-rust-specific", "premature abstraction", etc.
- **No action recommendations.** No "should be deleted", "should be ported", "could be folded into X".
- **Factual evidence only.** What the C++ type does, where equivalent functionality appears in Rust, what's covered, what's not, ambiguities.
- **Open questions surfaced explicitly.** When an agent can't determine something with confidence, it writes `OPEN QUESTION: ...` rather than guessing.

The output is a research dossier, not a decision document.

## Architecture

6 parallel agents, each running in a git worktree for isolation. Each agent analyzes a group of related marker files.

### Agent Groups

| Agent | Marker Files | Rationale |
|-------|-------------|-----------|
| **stdlib-containers** | emArray, emList, emString | Standard container/string types — likely map to Rust stdlib |
| **ownership-memory** | emRef, emOwnPtr, emOwnPtrArray, emCrossPtr, emAnything | Smart pointers and type-erasure — relate to Rust ownership model |
| **system-primitives** | emThread, emFileStream, emTmpFile | OS/system abstractions |
| **framework-glue** | emAvlTree, emAvlTreeMap, emAvlTreeSet, emToolkit | Data structures and framework initialization |
| **rust-only-infra** | fixed, rect, widget_utils, emPainterDrawList | Rust-only infrastructure files |
| **rust-only-toolkit** | toolkit_images | Toolkit image data — potentially related to emToolkit |

### Agent Instructions (no_rust_equivalent files)

For each assigned marker file, the agent must:

1. **Read the C++ header** from `~/git/eaglemode-0.96.4/include/emCore/{name}.h`. Record:
   - Every public class/struct/typedef declared
   - Key public methods and their signatures
   - What the type fundamentally does (container? smart pointer? system wrapper?)

2. **Read the C++ implementation** from `~/git/eaglemode-0.96.4/src/emCore/{name}.cpp` if it exists. Note any significant implementation details not visible from the header.

3. **Trace C++ usage.** Grep the C++ codebase (`include/emCore/` and `src/emCore/`) for uses of the type. Record:
   - Which other C++ files use this type
   - How it's typically used (construction patterns, common method calls)
   - Any non-obvious usage patterns

4. **Search the Rust codebase.** For each C++ type/method found:
   - Grep `src/emCore/` for likely Rust equivalents (stdlib types, custom types, trait implementations)
   - Record specific file paths and line numbers where equivalent functionality exists
   - Note any C++ methods/behaviors that have no obvious Rust counterpart

5. **Write findings into the marker file.** Update the empty marker file with factual documentation (see format below).

### Agent Instructions (rust_only files)

For each assigned marker file, the agent must:

1. **Read the current Rust file** (`src/emCore/{name}.rs`). Record:
   - What types/functions it defines
   - What it exports
   - What other Rust files import from it

2. **Check git history.** Run `git log --follow --diff-filter=A -- src/emCore/{name}.rs` to find the creation commit. Then run `git log --follow --stat -- src/emCore/{name}.rs` to find major changes. Record:
   - Creation commit hash, date, and full commit message
   - Any commits that significantly changed the file's scope or purpose
   - Any renames or moves

3. **Trace C++ relationship.** Based on what the Rust file contains:
   - Identify which C++ files contain equivalent or related functionality
   - Grep C++ source for the same concepts/algorithms
   - Record where the C++ code lives and how it differs structurally

4. **Search for dependents.** Grep the Rust codebase for imports/uses of types from this file. Record which files depend on it and how.

5. **Write findings into the marker file.** Update the empty marker file with factual documentation (see format below).

### Marker File Output Format

#### no_rust_equivalent:

```
C++ header: include/emCore/emFoo.h
C++ implementation: src/emCore/emFoo.cpp (or "header-only")

C++ public API:
  - class emFoo : public emBar
  - emFoo(args) — constructor
  - MethodA(args) -> return — description
  - MethodB(args) -> return — description
  [... all public methods]

C++ usage in emCore:
  - Used in emBaz.h (line N): as member variable
  - Used in emQux.cpp (line N): constructed in method X
  [... all usages with file:line]

Rust equivalents found:
  - emFoo broadly maps to std::SomeType
  - MethodA: see src/emCore/emBar.rs:42
  - MethodB: no equivalent found in Rust codebase
  [... per-method mapping with file:line or "not found"]

Coverage gaps:
  - MethodB has no Rust equivalent. In C++ it is called from [locations].
  - Feature X (described in header comment) not found in Rust.

OPEN QUESTIONS:
  - Is the lack of MethodB intentional or an oversight?
  - emFoo::Feature X may be handled differently in Rust — see [location] — but unclear.
```

#### rust_only:

```
Rust file: src/emCore/foo.rs

Defines:
  - struct Foo — description
  - fn bar() — description
  [... all public items]

Used by:
  - src/emCore/emBaz.rs:42 — uses Foo for X
  - src/emCore/emQux.rs:99 — calls bar() in Y
  [... all dependents with file:line]

Git history:
  - Created: [hash] [date] — "[commit message]"
  - Major change: [hash] [date] — "[commit message]" — [what changed]

Related C++ code:
  - The functionality in this file corresponds to code in:
    - include/emCore/emBar.h lines N-M (inline method X)
    - src/emCore/emBaz.cpp lines N-M (function Y)
  - Differences: [factual description of structural differences]

OPEN QUESTIONS:
  - [any ambiguities about why this exists separately]
```

### Synthesis Step

After all 6 agents complete, a synthesis agent:

1. Reads all updated marker files
2. Validates no marker files are still empty
3. Collects all OPEN QUESTIONS into a single summary
4. Writes `docs/marker-audit-summary.md` containing:
   - A table of all 20 marker files with one-line summaries of findings
   - All OPEN QUESTIONS grouped by theme
   - Any cross-cutting observations (e.g., "3 agents independently noted that emToolkit functionality is spread across multiple Rust files")
5. Does NOT add classifications, recommendations, or action items

### Output Artifacts

- 20 updated marker files in `src/emCore/` (no longer empty)
- `docs/marker-audit-summary.md` — consolidated findings and open questions
- `target/marker-audit/` — working directory for agent intermediates (gitignored)

## What This Does NOT Do

- Modify any `.rs` source files
- Create new Rust implementations
- Touch tests or golden data
- Alter SPLIT files (Rust files that are legitimate splits of C++ headers)
- Make architectural decisions
- Classify or judge any marker file's reason for existence
