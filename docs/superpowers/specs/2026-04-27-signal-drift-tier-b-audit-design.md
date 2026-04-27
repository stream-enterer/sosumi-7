# Signal-Drift Audit (Tier B) — Design Spec

Date: 2026-04-27
Status: ready-for-plan
Branch context: f010-investigation
Predecessor: F010 fix-spec brainstorming session (this design produced after the brainstorm pivoted from F010-fix-only to audit-first)
Successor: a separate brainstorming session that consumes the audit artifact and produces remediation plans (F010 fix + drift fixes)

## 1. Motivation

The F010 investigation located the visible black-panel symptom in `emFileLinkPanel`'s `Cycle`: the panel does not subscribe to its model's change signal, so `ensure_loaded()` is never called after late path resolution and `vfs` stays at `Waiting` forever. (See log entry 0011 at `docs/debug/investigations/F010-investigation/log/0011-h1-fix-falsifies-as-cause-emFileLinkPanel-load-stuck.md`.)

Investigating this revealed that `emFileLinkPanel` is not the only Rust port site that has dropped C++'s universal `AddWakeUpSignal` + `IsSignaled` idiom. The fix-spec brainstorm identified at least eight panel files in `emfileman` and `emcore` that substitute polling-and-stay-awake for what C++ expresses as signal subscription. Some of these substitutes are observably equivalent; some — like `emFileLinkPanel`'s — are not, and produce visible bugs. Some pre-existing `DIVERGED:` annotations in the Rust port claim language-forced or dependency-forced status; these claims have not been independently re-validated against the four-question test in `CLAUDE.md`.

A targeted F010 fix without a complete drift inventory cannot be planned safely:

- The fix to `emFileLinkPanel` may fix one panel while leaving structurally-equivalent bugs elsewhere unaddressed.
- Some drift sites may already be observably correct (forced or accidentally-equivalent), and the inventory must distinguish those from real bugs.
- The remediation plan needs an exhaustive enumeration so phasing decisions can be made on real data, not partial samples.

This spec defines that inventory: an audit of the Rust port's signal-drift surface across the Tier-B scope (panels with reactive `Cycle` overrides + the models they consume). The audit produces a structured artifact that a follow-up brainstorming session will use as input to plan remediation, including the F010 fix.

## 2. Scope

### 2.1 Tier B definition

A file is in scope if **all** of the following hold:

1. Its C++ counterpart at `~/Projects/eaglemode-0.96.4/src/emFoo/emFoo.cpp` (or for `emCore` files, `~/Projects/eaglemode-0.96.4/src/emCore/emFoo.cpp`) contains at least one `AddWakeUpSignal(...)` call.
2. A Rust port file `crates/*/src/emFoo.rs` exists.
3. The C++ class is **either**:
   - **A panel that overrides `Cycle()`** (reactive substitution surface), **or**
   - **A model whose signals are the target of `AddWakeUpSignal` in any in-scope panel** (signal-emitter surface).

Dialog-class files (`emDialog`, `emFileDialog`, `emWindowStateSaver`, `emMiniIpc`) are included as panel-equivalent reactive surfaces — they have private engines that override `Cycle`-equivalent and use the subscription idiom.

### 2.2 Tier B file enumeration

The plan derives the precise file list mechanically (see Plan Task 1). A non-authoritative seed list, for reviewer orientation:

**Panel files (likely in scope):**
emColorField, emCoreConfigPanel, emFilePanel, emFileSelectionBox, emDialog, emFileDialog, emWindowStateSaver, emMiniIpc, emDirEntryAltPanel, emDirEntryPanel, emDirPanel, emDirStatPanel, emFileLinkPanel, emFileManControlPanel, emFileManSelInfoPanel, emMainControlPanel, emMainPanel, emMainWindow, emVirtualCosmos, emStocksControlPanel, emStocksFetchPricesDialog, emStocksFilePanel, emStocksItemChart, emStocksItemPanel, emStocksListBox.

**Model / config / signal-emitter files (likely in scope):**
emConfigModel, emFileModel, emImageFile, emFileManModel, emFileManViewConfig, emStocksFileModel, emStocksPricesFetcher, emFileManThemeNames, emDirModel, emAutoplay, emBookmarks.

The plan's Task 1 produces the canonical list. If Task 1's output disagrees with this seed list, Task 1's output is authoritative.

### 2.3 Out of scope (Tier C, deferred)

The following Rust files have C++ counterparts that use `AddWakeUpSignal` but are framework / scheduler / view infrastructure rather than reactive panels-or-models. They are deferred to a future Tier-C audit:

`emView.rs`, `emWindow.rs`, `emGUIFramework.rs`, `emContext.rs`, `emRenderThreadPool.rs`, `emEngine.rs`, `emPanel.rs`, `emPanelCycleEngine.rs`, `emPanelTree.rs`, `emInputDispatchEngine.rs`, `emPriSchedAgent.rs`, `emRecListener.rs`, `emSubViewPanel.rs`, `emRes.rs`, `emMain.rs`.

Bug-class rationale for deferral: drift in these files manifests as scheduler / surface-creation / framework-level malfunctions, not as panel-reactivity bugs. They are a different bug class with different symptoms and risk; they deserve their own audit when a symptom appears.

### 2.4 Out-of-scope work

This spec describes an **investigation that produces a document**. Specifically out of scope:

- No code changes anywhere in the workspace.
- No remediation plan, prioritization, or phasing of drift fixes.
- No fix-spec writing.
- No F010 fix work — that is deferred to the follow-up brainstorming session that consumes this audit's artifact.
- No application of the four-question forbidden-fix-shape test (that test classifies fixes; this audit produces no fix).

## 3. Output artifact

### 3.1 Path and files

Directory: `docs/debug/audits/2026-04-27-signal-drift-tier-b/`

Files produced:

- `inventory.md` — human-readable Markdown report. Per-file sections, summary tables, aggregate verdicts.
- `inventory.json` — machine-readable companion. One row per C++ `AddWakeUpSignal` site in scope. Used by the follow-up brainstorm to iterate deterministically.
- `methodology.md` — concise procedural log: what the audit did, what tools it used, what reasoning rules it applied. One static document.
- `self-check.md` — completeness checks and counts. Demonstrates that the audit covered everything in scope.
- `tier-b-files.txt` — canonical Tier-B file enumeration (output of Plan Task 1).
- `cpp-sites.csv` — raw mechanical inventory of C++ `AddWakeUpSignal` sites in scope, before classification (output of Plan Task 2).

### 3.2 Output schema (per row in `inventory.json`)

```jsonc
{
  // Mechanical fields (from the cpp-sites.csv pre-pass)
  "id": "emFileLinkPanel-53",                      // stable: "<rust_file_basename>-<cpp_line>"
  "cpp_file": "src/emFileMan/emFileLinkPanel.cpp",
  "cpp_line": 53,
  "signal_expression": "UpdateSignalModel->Sig",   // verbatim from C++

  // Resolution to the conceptual signal
  "signal_kind": "model-update-broadcast",         // see §5.4 for vocabulary
  "rust_file": "crates/emfileman/src/emFileLinkPanel.rs",

  // Verdict (one of: faithful | drifted | forced | gap-blocked | unported)
  "rust_status": "drifted",

  // Evidence for the verdict
  "rust_evidence": {
    "kind": "polling",                             // connect_call | polling | stay_awake
                                                   // | wake_up_panel | rc_cell_shim | absent
    "file": "crates/emfileman/src/emFileLinkPanel.rs",
    "line": 175,
    "snippet": "fn Cycle(...) { self.file_panel.refresh_vir_file_state(); false }"
  },

  // Whether the Rust port exposes a SignalId accessor for this signal
  "rust_signal_accessor": null,                    // e.g. "model.AcquireUpdateSignalModel()"
  "rust_signal_accessor_status": "missing",        // present | missing | renamed | type-mismatch

  // For "forced" verdicts only — must be non-null
  "forced_category": null,                         // language-forced | dependency-forced
                                                   // | upstream-gap-forced | performance-forced
  "forced_evidence": null,                         // concrete test result, see §5.3

  // Pre-existing DIVERGED annotation, if any, in the Rust file referring to this drift
  "preexisting_diverged_annotation": null,         // {file, line, claim, revalidation_result}

  // For model-row entries: list of consumer (subscriber) IDs from this same inventory
  "consumers": null,                               // ["emDirPanel-37", "emDirEntryPanel-55", ...]

  "notes": ""
}
```

### 3.3 Per-file aggregate (also in `inventory.json`)

```jsonc
{
  "file": "crates/emfileman/src/emFileLinkPanel.rs",
  "cpp_counterpart": "src/emFileMan/emFileLinkPanel.cpp",
  "row_count": 4,
  "verdict_counts": { "faithful": 0, "drifted": 4, "forced": 0, "gap-blocked": 0 },
  "missing_accessors": ["UpdateSignalModel signal accessor on emFileLinkModel"],
  "preexisting_diverged_revalidations": 1,
  "notes": ""
}
```

## 4. Methodology

The audit is a hybrid of mechanical pre-passes and per-site reasoning. Section 5 specifies the reasoning rules. The methodology summary:

1. **Mechanical pre-pass — Tier-B file enumeration** (Plan Task 1). Filter every C++ file with `AddWakeUpSignal` against the existence of a Rust port, and against the panel-or-model role test. Produces `tier-b-files.txt`.

2. **Mechanical pre-pass — C++ site enumeration** (Plan Task 2). For every in-scope C++ file, list every `AddWakeUpSignal(...)` call: file, line, full signal expression. Produces `cpp-sites.csv`.

3. **Mechanical pre-pass — pre-existing DIVERGED annotation enumeration** (Plan Task 3). For every in-scope Rust file, list every `DIVERGED:` annotation block: file, line, claimed forced category, full annotation text. These will be re-validated in Task 6.

4. **Reasoning pass — panel-row classification** (Plan Task 4). For each C++ site whose Rust file is a panel (or panel-equivalent dialog/IPC class), read the Rust `Cycle` method and constructor / setter and classify the row per §5.

5. **Reasoning pass — model-row classification** (Plan Task 5). For each C++ site whose subscription target is a model, locate the Rust accessor for that signal and classify the accessor's status. Cross-link consumers.

6. **Reasoning pass — DIVERGED re-validation** (Plan Task 6). For each annotation collected in Task 3 that pertains to signal handling, run the four-question test independently of Tasks 4 / 5. Record verdict and reasoning. If the original claim fails, the row's verdict is reclassified to `drifted`.

7. **Render artifact** (Plan Tasks 7–8). Produce `inventory.md`, `inventory.json`, `methodology.md`.

8. **Self-check** (Plan Task 9). Counts match. Every row has the required evidence. Every annotation has a re-validation. Produces `self-check.md`.

## 5. Per-site classification rules

### 5.1 Verdicts

The five verdicts and their definitions:

- **faithful** — The Rust file calls `ectx.connect(...)` (or an equivalent pre-show wake-up rail like `add_pre_show_wake_up_signal`) for the same conceptual signal as the C++ `AddWakeUpSignal` call, AND the Rust `Cycle` (or equivalent) uses `IsSignaled(...)` on that signal to react. Both halves required.
- **drifted** — The Rust file uses a substitute mechanism (polling, `return true` stay-awake, `Rc<Cell<bool>>` shim, generation-counter polling) that is **not** justified by any of the four forced-divergence categories. Substitute may be observably correct or observably broken; this verdict captures only the structural divergence.
- **forced** — The Rust file diverges from the C++ shape, but the divergence passes the four-question test for one of the forced categories (§5.3) with concrete evidence cited. Examples: `Config->GetChangeSignal()` returning `u64` instead of `SignalId` is dependency-forced (the Rust API choice in `emFileManViewConfig` blocks the SignalId form); `Rc<Cell<bool>>` in winit closure callbacks may be language-forced (per CLAUDE.md ownership rule (a) for cross-closure references in winit/wgpu callbacks).
- **gap-blocked** — The Rust port lacks the API needed to subscribe. Typical cases: the model does not expose its `change_signal` as a public `SignalId` accessor; a renamed accessor doesn't appear under any obvious name; the signal exists only as a generation counter. Distinct from `forced` because gap-blocked is a fixable plumbing absence, not a justified architectural divergence.
- **unported** — The C++ class has no Rust port file. Rare in Tier B, possible if a C++ helper class without 1:1 port. Records the gap but takes no further action.

### 5.2 Decision tree

For each row:

1. Read the C++ site to identify the conceptual signal (model state? model change? config change? selection? button click? timer? close-signal?).
2. Search the Rust file for `connect(` calls. For each, check whether it targets the same conceptual signal:
   - If yes → continue to step 3.
   - If no `connect(` exists for this signal → likely `drifted` or `gap-blocked`; continue to step 4.
3. Search the Rust `Cycle` (and `cycle_inner` and any `on_cycle_ext` closure installed at construction) for `IsSignaled(<sig>)`:
   - If found → verdict `faithful`. Record `rust_evidence.kind = connect_call` with file:line of the connect; record snippet of the IsSignaled call site in `notes`.
   - If not found → the connect call exists but is dead. Verdict `drifted` (the connect is structural-only, no reaction). Record both as evidence.
4. Read the Rust `Cycle` to identify the substitute mechanism:
   - Polling a state value that the signal would have flagged: `rust_evidence.kind = polling`.
   - `return true` to stay awake unconditionally: `rust_evidence.kind = stay_awake`.
   - Wakeup driven by `wake_up_panel(id)` from outside: `rust_evidence.kind = wake_up_panel`.
   - `Rc<Cell<bool>>` flag set by a closure and observed in Cycle: `rust_evidence.kind = rc_cell_shim`.
   - No reaction at all: `rust_evidence.kind = absent`.
5. Look for a Rust accessor that returns the SignalId for this signal:
   - If present → record in `rust_signal_accessor`; status `present` (the gap is in the panel, not the model).
   - If missing → status `missing`. The verdict is `gap-blocked` if the panel cannot subscribe without first adding the accessor; otherwise `drifted` (panel could subscribe but doesn't).
   - If the signal exists only as a `u64` generation counter or other non-SignalId form → status `type-mismatch`. The verdict is `forced` with category `dependency-forced` if the Rust API choice makes the SignalId form impossible; otherwise it remains a candidate for re-design and is recorded as `gap-blocked`.
6. Apply the four-question test (§5.3) for any candidate-`forced` verdict. If no question's test passes with concrete evidence, downgrade to `drifted`.

### 5.3 Four-question forced-divergence test (per CLAUDE.md)

A divergence is **forced** only if at least one of the four questions has an affirmative answer with cited evidence:

- **Q1 — Language-forced.** Try writing the C++ shape in Rust under the project's canonical ownership model. If it does not compile, language-forced. *Evidence required:* either a concrete compile-error citation, or a citation to a neighboring Rust file that successfully compiles the same shape (which would refute the claim).
- **Q2 — Dependency-forced.** A required dependency (wgpu, winit, etc.) cannot be made to admit the C++ shape through its public API. *Evidence required:* citation of the dep API surface and the specific blocker.
- **Q3 — Upstream-gap-forced.** C++ emCore itself ships the shape as a no-op. *Evidence required:* C++ file:line showing the no-op.
- **Q4 — Performance-forced.** A benchmark demonstrates the C++-mirrored shape crossing a documented degradation threshold. *Evidence required:* path to the benchmark + threshold.

Convenience is **not** a forced category. "Idiom adaptation forced by a project-internal ownership choice" is **not** valid framing. If a Rust choice makes a C++ shape impossible, the audit records this verdict only after revisiting the Rust choice.

### 5.4 `signal_kind` vocabulary (controlled)

A controlled vocabulary for `signal_kind` to enable cross-row queries by the next session:

- `model-state` — emFileModel's `GetFileStateSignal()` family (Waiting → Loaded transitions).
- `model-change` — model's record-level change signal (e.g., `Model->GetChangeSignal()`).
- `model-update-broadcast` — global `UpdateSignalModel->Sig` (emFileModel's mtime-changed broadcast).
- `vir-file-state` — emFilePanel's `GetVirFileStateSignal()` (derived from model-state).
- `config-change` — `Config->GetChangeSignal()` family (per-config-tree change).
- `selection` — `FileMan->GetSelectionSignal()` and similar selection-tracking signals.
- `command` — `FMModel->GetCommandsSignal()` and similar command-broadcast signals.
- `widget-click` / `widget-check` — button click / checkbox check signals.
- `timer` — emTimer-based signals.
- `close` / `finish` — dialog close / finish signals.
- `other` — record a free-form description in `notes`.

If a row's signal does not fit any kind, the auditor adds a kind and notes the addition in `methodology.md`.

## 6. Quality gates

The audit is "complete" only if all of the following pass; `self-check.md` documents each:

1. **Row coverage.** `len(inventory.json.rows)` equals `sum over Tier-B C++ files of (grep -c AddWakeUpSignal)`. Off-by-one tolerated only with a documented exception (e.g., a multi-line `AddWakeUpSignal` call counted as one).
2. **No null verdicts.** Every row has `rust_status ∈ {faithful, drifted, forced, gap-blocked, unported}`. No `null` or `unknown`.
3. **Forced-evidence completeness.** Every `rust_status == "forced"` row has non-null `forced_category` and non-null `forced_evidence` containing a concrete citation.
4. **Drift-evidence completeness.** Every `rust_status == "drifted"` row has non-null `rust_evidence.kind ∈ {polling, stay_awake, wake_up_panel, rc_cell_shim, absent}` and non-null `rust_evidence.line`.
5. **Faithful-evidence completeness.** Every `rust_status == "faithful"` row has `rust_evidence.kind == "connect_call"` with non-null line, AND a notes-field `IsSignaled` citation showing the reaction half.
6. **Pre-existing DIVERGED re-validation.** Every annotation collected in Plan Task 3 that pertains to signal handling has a `revalidation_result` field. Annotations whose claims fail re-validation are listed by file:line in a top-level "reclassified to drifted" section in `inventory.md`.
7. **Consumer cross-links.** Every model row's `consumers` list is non-empty (a model row exists only because at least one panel subscribes to its signal). Every panel row referencing a model signal accessor cross-links to the model row.
8. **No code changes.** `git status` after the audit completes shows no modifications outside `docs/debug/audits/2026-04-27-signal-drift-tier-b/` (and possibly `docs/superpowers/specs/` and `docs/superpowers/plans/` for this spec and plan).

## 7. Non-goals (recap)

- No code changes.
- No remediation plan or ordering.
- No prioritization of drift sites.
- No fix-spec writing.
- No F010 fix.
- No application of the forbidden-fix-shape four-question test (that test classifies fixes; the follow-up brainstorm applies it).
- No Tier-C audit.

## 8. Forbidden-fix-shape test

Not applicable. This spec describes an investigation that produces a document, not a fix. The four-question forbidden-fix-shape test in `docs/debug/investigations/F010-investigation/forbidden-fix-shapes.md` will be applied to each fix proposed in the follow-up brainstorming session.

## 9. Handoff

The follow-up brainstorming session begins by reading `inventory.md` (or `inventory.json` if iterating mechanically). Its goals:

- Plan the F010 fix proper (Phase 1 in that session) — `emFileLinkPanel`'s missing subscription, plus any model-side accessor additions the audit identified as needed.
- Plan drift remediation phases (Phase 2+) over the rows in the audit, using audit-derived priorities (high-risk drifted rows first; gap-blocked rows that need accessor additions next; forced rows merely re-annotated).
- Apply the forbidden-fix-shape test to each phase.
- Decide the order: F010 fix first (independently shippable; closes the visible bug) vs. emcore base fix first (cleaner final state but delays user verification).

This spec does not pre-empt any of those decisions.

## 10. Risks and mitigations

- **Risk: missed drift sites.** Mitigation: Tier-B scope is mechanically derived in Plan Task 1, not hand-curated; the row-coverage gate (§6.1) verifies counts against grep totals.
- **Risk: false-positive faithfulness.** A Rust file may contain a `connect(` call that is dead code or test-only. Mitigation: §5.2 step 3 requires both halves (connect AND IsSignaled) for a faithful verdict.
- **Risk: false-negative faithfulness.** Subscription via inheritance / pre-show rail may not be obvious from a grep. Mitigation: §5.2 step 2 instructs the auditor to consider helper paths; methodology.md documents specific patterns observed.
- **Risk: forced-claim rubber-stamping.** Pre-existing `DIVERGED:` annotations may carry unjustified forced claims. Mitigation: §5.3 evidence requirements + Plan Task 6 independent re-validation.
- **Risk: per-file effort underestimate.** §1 estimated 5–15 min per file. Plan Task 4 / 5 carry no time budget; the auditor reads until they can answer; the gate is verdict-with-evidence, not elapsed time.
- **Risk: scope creep into Tier C.** Mitigation: §2.3 is an explicit deny-list; if the auditor finds drift in a Tier-C file, they note it in `methodology.md` "deferred observations" but do not classify or remediate.

---

End of design spec.
