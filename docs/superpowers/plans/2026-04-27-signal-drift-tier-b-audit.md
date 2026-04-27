# Plan: Signal-Drift Audit (Tier B)

Date: 2026-04-27
Spec: `docs/superpowers/specs/2026-04-27-signal-drift-tier-b-audit-design.md`
Branch: f010-investigation
Working directory: `/home/alex/Projects/eaglemode-rs.f010-investigation`
Execution model: subagent-driven-implementation in a separate session (user will invoke after this plan lands).

## Context for the executor

This plan implements the audit defined in the spec. **Read the spec first**, especially §3 (output schema), §5 (per-site classification rules), and §6 (quality gates). The plan tasks reference the spec by section number rather than restating rules.

**No code changes.** This audit produces only documents under `docs/debug/audits/2026-04-27-signal-drift-tier-b/`. Any task that finds itself wanting to edit production code has misread the spec — halt and re-read §2.4 and §7.

**No remediation.** This audit produces no fix-spec, no plan, no priority ordering. The follow-up brainstorming session does that work, taking this audit as input. A task that finds itself wanting to write "Phase 1: fix X" content has misread the spec — halt and re-read §9.

**Authority hierarchy when sources disagree:**
1. C++ source at `~/Projects/eaglemode-0.96.4/src/` — ground truth for what was subscribed.
2. Rust source under `crates/` — ground truth for what is currently implemented.
3. Pre-existing Rust comments / annotations — claims that may be wrong; re-validate against the four-question test before trusting.

## Anti-patterns to harden against

The executor must not:

- **Skip rows.** Every C++ `AddWakeUpSignal` site in scope produces exactly one `inventory.json` row. Counts are verified mechanically (Task 9). If a row "looks obviously faithful," it still gets a row with cited evidence.
- **Classify without evidence.** A `faithful` verdict requires a cited `connect(` line **and** a cited `IsSignaled(` reaction line. A `forced` verdict requires a cited test result for the four-question test. A `drifted` verdict requires a cited substitute-mechanism line. If the executor cannot produce the citation, the verdict is not yet decided.
- **Trust pre-existing `DIVERGED:` annotations.** Re-validate every annotation in Tier-B scope (Task 6). Some claims will fail re-validation. CLAUDE.md is explicit: "Treating Rust convenience as a reason to diverge ... Convenience is never a reason."
- **Hand-curate the file list.** Task 1 derives it mechanically. The seed list in spec §2.2 is non-authoritative.
- **Edit code while auditing.** Documents only. No edits to `crates/`. Anything else is out of scope and a halt condition.
- **Assume the F010 fix is in scope.** It is not. The audit informs the next brainstorm; that brainstorm plans the fix.

## Phase / task structure

Tasks 1–3 are mechanical pre-passes; their outputs feed the reasoning passes in Tasks 4–6. Tasks 7–9 produce the artifact and verify it. Task 10 commits.

Each task has: **inputs**, **procedure**, **output**, **acceptance criteria**. A task is complete only when its acceptance criteria are met and verifiable from artifacts on disk.

---

## Task 1 — Derive Tier-B file enumeration

**Inputs:**
- Spec §2.1 (Tier-B definition) and §2.3 (Tier-C exclusion list).
- C++ source tree at `~/Projects/eaglemode-0.96.4/src/`.
- Rust workspace at `crates/`.

**Procedure:**

1. Enumerate every C++ file under `~/Projects/eaglemode-0.96.4/src/em*/` and `~/Projects/eaglemode-0.96.4/src/SilChess/` containing `AddWakeUpSignal`. Use `grep -l "AddWakeUpSignal" <files>` over each path.
2. For each C++ file, check whether a Rust port file exists with the same basename under `crates/`. Use `find crates/ -name "<basename>.rs"`.
3. Apply Tier-C exclusion list from spec §2.3. Drop matches.
4. For each remaining file, classify the C++ class role:
   - **panel-with-Cycle**: C++ class overrides `Cycle()` AND inherits transitively from `emPanel`. Confirm by reading the class declaration in the corresponding `.h` file (`~/Projects/eaglemode-0.96.4/include/emCore/<basename>.h` or wherever).
   - **dialog-equivalent**: C++ class is a dialog with a private engine that overrides Cycle (e.g., emDialog, emFileDialog, emWindowStateSaver, emMiniIpc).
   - **model-or-config**: C++ class emits signals consumed by AddWakeUpSignal in any in-scope panel; check by the file basename appearing as `XxxSignal` target elsewhere.
   - **other**: drop — only if confidently neither panel-equivalent nor signal-emitter.
5. Save to `docs/debug/audits/2026-04-27-signal-drift-tier-b/tier-b-files.txt`, one file per line, format:
   ```
   <role>\t<rust_file>\t<cpp_file>
   ```

**Output:** `tier-b-files.txt` (canonical Tier-B enumeration).

**Acceptance criteria:**
- File exists at the path above.
- Every line has the three columns. No empty rust_file or cpp_file.
- Number of distinct rust_file paths is between 20 and 45 (a sanity range based on the seed list in spec §2.2; values outside this range trigger a re-read of methodology before proceeding).
- No file from spec §2.3 Tier-C exclusion list appears.

---

## Task 2 — Enumerate C++ AddWakeUpSignal sites

**Inputs:**
- `tier-b-files.txt` from Task 1.

**Procedure:**

1. For each `cpp_file` in `tier-b-files.txt`, run `grep -n "AddWakeUpSignal" <cpp_file>` and capture every match.
2. For each match, parse:
   - `cpp_line` (integer)
   - `signal_expression` — the verbatim argument to `AddWakeUpSignal(...)`, e.g. `Model->GetChangeSignal()`. Trim whitespace; preserve everything inside the parentheses up to the matching closing paren.
   - Multi-line calls (rare): treat as one row counted at the line of the opening `AddWakeUpSignal(`. Document any multi-line call in `methodology.md` so Task 9's count check accounts for it.
3. Compute `id` = `<rust_file_basename_without_.rs>-<cpp_line>` (stable across re-runs).
4. Save to `cpp-sites.csv` with header columns:
   `id, cpp_file, cpp_line, rust_file, signal_expression`.

**Output:** `cpp-sites.csv` — one row per C++ site, ordered by (rust_file, cpp_line).

**Acceptance criteria:**
- File exists; CSV header present; every row populated.
- Row count equals `sum over Tier-B C++ files of (grep -c "AddWakeUpSignal" <cpp_file>)`. Document this sum in `methodology.md`.
- No `id` collision (distinct rows have distinct ids). If a collision happens (same file, two AddWakeUpSignal on same line), append `-a/-b` suffixes and document in methodology.md.

---

## Task 3 — Enumerate pre-existing Rust DIVERGED annotations in scope

**Inputs:**
- `tier-b-files.txt` from Task 1.

**Procedure:**

1. For each `rust_file` in scope, run `grep -n "DIVERGED:" <rust_file>` and capture every match.
2. For each match, capture the full annotation block (the `DIVERGED:` line plus continuation comment lines, conventionally up to the next non-comment line).
3. Identify the stated forced category if any (e.g., `(language-forced)`, `(dependency-forced)`).
4. Determine whether the annotation pertains to **signal handling** — the criterion: the annotation block mentions any of: `AddWakeUpSignal`, `IsSignaled`, `Signal`, `Cycle`, `subscribe`, `wake`, `Notice`, `signal`. If it pertains to other divergence (e.g., AVL iterator, color rec API), mark as `unrelated` and exclude from re-validation in Task 6.
5. Save to `preexisting-diverged.csv` with columns:
   `rust_file, line, claimed_category, full_block, signal_related (bool), notes`.

**Output:** `preexisting-diverged.csv`.

**Acceptance criteria:**
- File exists; every row populated.
- The `signal_related = true` subset is the input to Task 6.
- `unrelated` rows are preserved but excluded from re-validation.

---

## Task 4 — Classify panel-row sites

**Inputs:**
- `cpp-sites.csv` from Task 2 (subset where `tier-b-files.txt` role is `panel-with-Cycle` or `dialog-equivalent`).
- Spec §5 (classification rules).
- Pre-existing annotations for the file from Task 3.

**Procedure:**

For each panel-row C++ site:

1. **Identify the conceptual signal.** Read the C++ site's surrounding context (the constructor/method enclosing the AddWakeUpSignal call) to understand which model / config / button / timer is being subscribed to. Map to the controlled vocabulary in spec §5.4.
2. **Search the Rust file for `connect(` calls.** Use `grep -n "connect(" <rust_file>`. For each, read the call: what signal does it pass? Is it the same conceptual signal as the C++ site?
   - If yes: continue to step 3.
   - If no `connect(` for this signal: skip to step 4.
3. **Search Rust Cycle (and any `on_cycle_ext` closure or cycle_inner) for `IsSignaled(` referencing this signal.**
   - If found: verdict = `faithful`. Record `rust_evidence.kind = connect_call`, file:line of the connect; record snippet of the IsSignaled site in `notes`. Done with this row.
   - If not found: verdict = `drifted` (connect exists but is dead). Record both as evidence in `notes`.
4. **Read the Rust Cycle to identify the substitute mechanism.** Look for:
   - Polling: a state field is read each Cycle and acted on (e.g., `last_config_gen != cfg_gen`). `rust_evidence.kind = polling`.
   - Stay-awake: `return true;` from Cycle to keep cycling. `rust_evidence.kind = stay_awake`.
   - External wake: `wake_up_panel(id)` is called from a model or other code, sidestepping subscription. `rust_evidence.kind = wake_up_panel`.
   - Rc<Cell<bool>> shim: a flag is set by a closure and observed in Cycle. `rust_evidence.kind = rc_cell_shim`.
   - No reaction at all: `rust_evidence.kind = absent`.
5. **Look for a Rust accessor returning the SignalId for this signal.** Examples: `model.GetFileStateSignal()`, `config.GetChangeSignal()` (note: u64 form is type-mismatch, not present).
   - Present (returns SignalId): `rust_signal_accessor_status = present`.
   - Missing (no accessor exists): `missing`.
   - Renamed (accessor exists under non-1:1 name): `renamed`.
   - Type-mismatch (returns u64 generation or other non-SignalId): `type-mismatch`.
6. **Apply the four-question test for any candidate-`forced` verdict** (typically when accessor status is `type-mismatch`):
   - Q2 dependency-forced is the most common applicable answer (Rust API choice blocks SignalId form).
   - Cite concrete evidence per spec §5.3.
7. **Cross-reference any pre-existing DIVERGED annotation for this file (from Task 3).** If one of the annotations references this drift, copy its block into `preexisting_diverged_annotation.claim`. The `revalidation_result` field is populated by Task 6.
8. **Write the row to `inventory.json`** with the schema in spec §3.2.

**Output:** Panel rows in `inventory.json` (ongoing; final assembly in Task 7).

**Acceptance criteria:**
- Every panel-row site from `cpp-sites.csv` produces a row in inventory.json.
- Every row has a verdict in `{faithful, drifted, forced, gap-blocked, unported}`. No `null`.
- Every `faithful` verdict has both `rust_evidence.kind = connect_call` AND a `notes`-field IsSignaled citation (file:line).
- Every `drifted` verdict has `rust_evidence.kind` set and `rust_evidence.line` populated.
- Every `forced` verdict has `forced_category` and `forced_evidence` populated with a concrete citation.

---

## Task 5 — Classify model-row sites

**Inputs:**
- `cpp-sites.csv` from Task 2 (subset where the C++ subscription target is a model in scope — i.e., the signal's emitter is a model file in `tier-b-files.txt`).
- Panel rows from Task 4 (for cross-linking).

**Procedure:**

For each unique model/signal pair appearing as a target across the panel rows:

1. Identify the Rust file for the model (from `tier-b-files.txt`).
2. Search the Rust model file for an accessor method returning the SignalId. Examples: `pub fn GetFileStateSignal(&self) -> SignalId`, `pub fn AcquireUpdateSignalModel(&self) -> SignalId`.
3. Classify:
   - Accessor exists and returns `SignalId` → `present`.
   - Accessor exists but returns a non-SignalId form (e.g., `u64` generation) → `type-mismatch`.
   - Accessor does not exist (model has the signal internally but doesn't expose it) → `missing`.
   - Accessor exists under a non-1:1 name (renamed without DIVERGED annotation) → `renamed` (record both names).
4. Build a `consumers` list: every panel-row id whose `signal_expression` targets this model+signal.
5. Write a model-row entry to `inventory.json` for each unique (model, signal) pair. The model-row uses the same schema as panel-rows but `rust_status` typically reflects the accessor status:
   - `present` → no dedicated model row needed unless the consumer panels are gap-blocked elsewhere; document the accessor for cross-reference in panel rows' `rust_signal_accessor` instead.
   - `missing` / `type-mismatch` / `renamed` → a model row with `rust_status = "gap-blocked"` and `rust_evidence.kind = "absent"`, listing consumers.
6. Update each consumer panel row's `rust_signal_accessor` and `rust_signal_accessor_status` from this analysis. (If Task 4 already populated, verify consistency.)

**Output:** Model rows added to `inventory.json`; panel rows' accessor fields updated.

**Acceptance criteria:**
- Every model-signal pair referenced by ≥1 panel row has either:
  - At least one panel row with `rust_signal_accessor_status = present` referencing the accessor, OR
  - A model row in inventory.json with `rust_status = gap-blocked` and a non-empty `consumers` list.
- No model row's `consumers` list is empty.

---

## Task 6 — Re-validate pre-existing DIVERGED annotations (signal-related subset)

**Inputs:**
- `preexisting-diverged.csv` from Task 3 (subset `signal_related = true`).
- Spec §5.3 (four-question test).

**Procedure:**

For each annotation in the signal-related subset:

1. Identify the claimed forced category (parsed from the annotation, e.g. `(language-forced)`).
2. Apply the four-question test independently:
   - **Q1 (language-forced):** Try writing the C++ shape in Rust under the project's ownership model. Does it compile? Look for a neighboring Rust file that *successfully* compiles the same shape — if found, the claim fails. (For example: emFileDialog uses `connect()` + `IsSignaled()` from a panel-equivalent class — that refutes any "language-forced" claim against using the same pattern in another panel.) Cite the refutation.
   - **Q2 (dependency-forced):** Cite the dep API surface that blocks the C++ shape. If no specific dep is cited and no investigation shows one, the claim fails.
   - **Q3 (upstream-gap-forced):** Check the C++ source for the would-be no-op. If C++ has the shape, the claim fails.
   - **Q4 (performance-forced):** Look for a benchmark file. If absent, the claim fails by definition.
3. Record the `revalidation_result` per annotation:
   - `verified` — the claim passes its category's test with cited evidence.
   - `failed` — the claim does not pass; classify the divergence as `drifted`.
   - `wrong_category` — the claim cites the wrong category but a different category passes; record the corrected category.
4. For any `failed` re-validation, **update the corresponding inventory.json row's `rust_status` to `drifted`** (overriding any forced verdict Task 4 may have given based on the annotation alone).
5. Save the per-annotation re-validation results back to `preexisting-diverged.csv` as a new column `revalidation_result` (and `corrected_category` where applicable).

**Output:**
- Updated `preexisting-diverged.csv` with revalidation columns.
- `inventory.json` rows updated where Task 4's verdict was based on a now-failed claim.
- Section in inventory.md (rendered in Task 7) titled "Annotations reclassified to drifted."

**Acceptance criteria:**
- Every annotation in the signal-related subset has a non-null `revalidation_result`.
- Every `failed` annotation produces a row update in inventory.json or a documented note explaining why the corresponding row's verdict is unchanged.

---

## Task 7 — Render `inventory.md`

**Inputs:** `inventory.json` (from Tasks 4–6), `preexisting-diverged.csv`.

**Procedure:**

Render a Markdown report with these sections:

1. **Summary table.** Columns: `file`, `total_rows`, `faithful`, `drifted`, `forced`, `gap-blocked`, `unported`. One row per Tier-B file. Sortable by drift count.
2. **Drifted-row drill-down.** For each `drifted` row in inventory.json: file, cpp_site, signal, rust_evidence kind+line, notes.
3. **Gap-blocked rows.** For each `gap-blocked` row: file, signal, accessor status, notes — the missing-accessor list the next session will use.
4. **Forced rows.** For each `forced` row: file, signal, forced_category, forced_evidence — the audit's accepted divergences.
5. **Annotations reclassified to drifted.** Output of Task 6 `failed` revalidations.
6. **Annotations verified.** Output of Task 6 `verified` revalidations (kept for transparency).
7. **Per-file deep-dive.** One section per file with row-by-row detail for any reader who wants the underlying rows.

**Output:** `inventory.md`.

**Acceptance criteria:**
- Every section listed above is present and non-empty (or labeled "(none)" with explanation).
- Numerical counts in the summary table match `inventory.json` aggregations.

---

## Task 8 — Write `methodology.md`

**Inputs:** All preceding tasks.

**Procedure:**

Write a static procedural log:

- What greps were used and at what paths.
- What reasoning rules were applied (link to spec §5).
- Any deviations from the spec procedure with rationale.
- Edge cases encountered (multi-line `AddWakeUpSignal`, signal_kind additions, annotation parsing exceptions).
- Observations *outside scope* worth recording for a future Tier-C audit (e.g., drift spotted in `emView.rs` while reading neighboring code) — note but do not classify.

**Output:** `methodology.md`.

**Acceptance criteria:**
- Document is self-contained: a reader who has not seen this plan can still reproduce the audit from `methodology.md` alone.

---

## Task 9 — Self-check

**Inputs:** All produced artifacts.

**Procedure:**

For each of the eight quality gates in spec §6, produce a check + result:

1. **Row coverage.** Compute `len(inventory.json.rows)` vs `sum over Tier-B C++ files of grep -c AddWakeUpSignal`. Difference must be zero (or documented multi-line exception).
2. **No null verdicts.** Scan `inventory.json` for any row with null/empty `rust_status`. Must be empty.
3. **Forced-evidence completeness.** For each `forced` row, check non-null `forced_category` and `forced_evidence`. List any failures.
4. **Drift-evidence completeness.** For each `drifted` row, check `rust_evidence.kind` and `rust_evidence.line`. List failures.
5. **Faithful-evidence completeness.** For each `faithful` row, check `rust_evidence.kind = connect_call` and notes-field IsSignaled citation.
6. **Pre-existing DIVERGED re-validation.** For each signal-related annotation in `preexisting-diverged.csv`, check non-null `revalidation_result`.
7. **Consumer cross-links.** For each model row, check non-empty `consumers`.
8. **No code changes.** Run `git status`. Output must show no modifications outside `docs/debug/audits/2026-04-27-signal-drift-tier-b/`, `docs/superpowers/specs/`, `docs/superpowers/plans/`.

Save results to `self-check.md` with one section per gate.

**Output:** `self-check.md`.

**Acceptance criteria:**
- Every gate marked `pass` or `fail` with explicit numbers / citations.
- Any `fail` halts the audit: do not commit. Report failures and stop.

---

## Task 10 — Commit

**Inputs:** All artifacts; passing self-check.

**Procedure:**

1. Verify `self-check.md` shows all gates passing.
2. `git add docs/debug/audits/2026-04-27-signal-drift-tier-b/`.
3. (Spec and plan are already committed in a prior commit; no need to re-add.)
4. Commit with message:
   ```
   docs(F010): signal-drift Tier-B audit artifact

   Produces inventory.md/json + methodology.md + self-check.md under
   docs/debug/audits/2026-04-27-signal-drift-tier-b/. No code changes.
   Feeds the next brainstorming session that will plan F010 fix +
   drift remediation.
   ```
5. `git status` — verify clean.

**Output:** A single commit on `f010-investigation`.

**Acceptance criteria:**
- One new commit; working tree clean afterward.
- Pre-commit hook passes (this is a docs-only commit; clippy and nextest should be unaffected, but if any test fails it must be a pre-existing failure unrelated to this work).
- No production-code changes.

---

## Halt conditions (stop and report; do not proceed)

The executor halts and reports back to the user — does **not** improvise — in any of these cases:

- **Task 1 file count outside [20, 45].** May indicate a bad filter or hidden Tier-C inclusion. Report file list and ask.
- **Task 2 row count differs from grep totals by more than the documented multi-line exceptions.** Report numbers and ask.
- **Task 4 / 5 verdict cannot be reached without code changes.** Spec is investigation-only; do not patch.
- **Task 6 finds an annotation that is neither verified nor failed under any of the four questions.** Should not happen — record the analysis, mark as `failed` (the claim is unjustified), but flag in methodology.md.
- **Task 9 gate fails.** Do not commit. Report the gate and the rows / numbers that failed.
- **Task encounters a Tier-C drift observation that demands attention.** Note in methodology.md "deferred observations"; do not classify; do not extend scope.
- **Any task wants to edit production code.** Halt; spec is documents-only.

## Definition of done

- All ten tasks have completed acceptance criteria met.
- One commit exists on `f010-investigation` adding the audit artifacts under `docs/debug/audits/2026-04-27-signal-drift-tier-b/`.
- `self-check.md` reports all gates passing.
- No production code modified anywhere in `crates/`.
- The follow-up brainstorming session has everything it needs to plan remediation: a complete inventory, a methodology log, an annotation re-validation report, and explicit gap-blocked / drifted / forced classifications.

---

End of plan.
