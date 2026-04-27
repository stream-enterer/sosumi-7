# Signal-Drift Tier-B Audit — Self-Check

Date: 2026-04-27
Spec: `docs/superpowers/specs/2026-04-27-signal-drift-tier-b-audit-design.md` §6

All gates computed mechanically from `inventory.json` and `preexisting-diverged.csv`.
Git status verified via `git status --porcelain`.

---

## Gate 1: Row coverage

Result: PASS

Computation:

- C++ sites in scope (rows in `cpp-sites.csv`, excluding header): 201
- Inventory rows with `consumers == null` (panel rows + model-internal rows, 1:1 with C++ sites): 201
- Inventory rows with `consumers != null` (cross-link model rows, no C++ site of their own): 11
- Total inventory rows: 212

The 201 panel-or-model-internal rows map exactly to the 201 grep total.
The 11 cross-link rows are aggregate consumer records — they are not C++ `AddWakeUpSignal` sites.
Arithmetic: 201 + 11 = 212 = `len(inventory.json.rows)`. Exact match.

Known multi-line exception documented in methodology: `src/emCore/emImageFile.cpp:139` — a
two-call `AddWakeUpSignal` block that produces two inventory rows from two separately-counted
grep hits. No net off-by-one.

---

## Gate 2: No null verdicts

Result: PASS

Computation:

- Rows scanned: 212
- Rows with null or empty `rust_status`: 0
- `rust_status` distribution: `drifted`=162, `faithful`=8, `gap-blocked`=16, `unported`=26, `forced`=0

All 212 rows carry a valid verdict from the controlled vocabulary
`{faithful, drifted, forced, gap-blocked, unported}`.

---

## Gate 3: Forced-evidence completeness

Result: PASS

Computation:

- Rows with `rust_status == "forced"`: 0
- Gate trivially passes — no forced rows require `forced_category` / `forced_evidence`.

---

## Gate 4: Drift-evidence completeness

Result: PASS

Computation:

- Rows with `rust_status == "drifted"`: 162
- Of these, rows missing `rust_evidence.kind`: 0
- Of these, rows missing `rust_evidence.line`: 0

All 162 drifted rows carry both a `kind` value from the controlled vocabulary
`{polling, stay_awake, wake_up_panel, rc_cell_shim, absent}` and a non-null `line`.

---

## Gate 5: Faithful-evidence completeness

Result: PASS

Computation:

- Rows with `rust_status == "faithful"`: 8
- Of these, rows with `rust_evidence.kind != "connect_call"`: 0
- Of these, rows with null `rust_evidence.line`: 0
- Of these, rows with no `IsSignaled` citation in `notes`: 0

All 8 faithful rows carry both halves of the faithful evidence requirement:
a `connect_call` evidence record (with `kind`, `file`, `line`, `snippet`) and
an `IsSignaled` citation in `notes`. Row list:

| id | connect line |
|----|-------------|
| emDialog-38 | 671 |
| emFileDialog-42 | 110 |
| emMiniIpc-807 | 366 |
| emWindowStateSaver-39 | 1207 |
| emWindowStateSaver-40 | 1209 |
| emWindowStateSaver-41 | 1208 |
| emMainWindow-72 | 1113 |
| emMainWindow-378 | 1114 |

---

## Gate 6: Pre-existing DIVERGED re-validation

Result: PASS

Computation:

- Total rows in `preexisting-diverged.csv`: 38 (37 data + 1 header)
- Rows marked `signal_related = true`: 15
- Of these 15, rows missing `revalidation_result`: 0
- Non-signal-related rows (23): not required by spec §6 to carry `revalidation_result`

All 15 signal-related pre-existing annotations carry a `revalidation_result` value
(`verified` or `failed`). Annotations whose `revalidation_result == "failed"` are listed
in `inventory.md` under the "reclassified to drifted" section per spec §6.6.

---

## Gate 7: Consumer cross-links

Result: PASS

Computation:

- Model rows (rows with `consumers != null`): 11
- Of these, rows with empty `consumers` list: 0

All 11 cross-link model rows carry at least one consumer reference. Sample:

- `emFileManViewConfig-accessor-config-change` → 6 consumers
- `emFileManModel-accessor-command` → 1 consumer
- `emFileManModel-accessor-selection` → 4 consumers

---

## Gate 8: No code changes

Result: PASS

Computation:

`git status --porcelain` output:

```
?? docs/debug/audits/
```

The only untracked path is `docs/debug/audits/` — the audit artifact directory itself.
No modifications to any tracked file.
No changes anywhere under `crates/`, `src/`, `tests/`, or `scripts/`.
All audit work is confined to `docs/debug/audits/2026-04-27-signal-drift-tier-b/`.

---

## Summary

| Gate | Name | Result |
|------|------|--------|
| 1 | Row coverage | PASS |
| 2 | No null verdicts | PASS |
| 3 | Forced-evidence completeness | PASS |
| 4 | Drift-evidence completeness | PASS |
| 5 | Faithful-evidence completeness | PASS |
| 6 | Pre-existing DIVERGED re-validation | PASS |
| 7 | Consumer cross-links | PASS |
| 8 | No code changes | PASS |

All eight gates pass. The audit artifact is complete and consistent.
