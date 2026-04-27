# Signal-Drift Remediation Bookkeeping Strategy — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce the artifact set defined in the strategy spec — `inventory-enriched.json`, `decisions.md`, `buckets/B-###-<slug>.md`, `work-order.md` — by executing Phases 1–4 of the strategy.

**Architecture:** Data-bookkeeping work, not code. Each phase reads upstream audit artifacts + this-phase outputs, produces a new artifact, and runs a validation gate. Validation gates replace passing tests. Phase 5 (reconciliation) is a standing responsibility owned by the working-memory session and is not part of this plan.

**Tech Stack:** Python 3 for JSON manipulation and validation; the working-memory session (Claude) for judgement-heavy enrichment, ADR drafting, and clustering; subagents dispatched in parallel for sketch-file authoring once the schema is fixed.

**Spec:** `docs/superpowers/specs/2026-04-27-signal-drift-remediation-strategy-design.md`
**Source audit:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/`

---

## File Structure

All outputs go under one directory co-located with the source audit:

```
docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/
├── inventory-enriched.json     # Task 2 output
├── decisions.md                # Task 3 output
├── buckets/
│   └── B-###-<slug>.md         # Task 4 outputs (one per bucket)
└── work-order.md               # Task 5 output
```

Plus two intermediate artifacts kept for provenance and re-derivation:

```
docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/
├── pattern-catalog.md          # Task 2 sub-output
└── decision-catalog.md         # Task 2 sub-output
```

The plan itself lives at `docs/superpowers/plans/2026-04-27-signal-drift-remediation-strategy.md`.

---

## Conventions

- **Actionable row** = an `inventory.json` row with `rust_status` ∈ {`drifted`, `gap-blocked`}. Expected count: 178 (162 + 16). Rows with `faithful` or `unported` status are excluded from enrichment.
- **Cleanup item** = an entry in `preexisting-diverged.csv` flagged in audit Task 6 as failed re-validation or wrong-category. Expected count: 9.
- All `D-###` and `B-###` IDs are zero-padded to three digits.
- All commit messages use the `docs(F010):` prefix to match the audit-branch convention.

---

## Task 1: Set up artifact directory and conventions check

**Files:**
- Create: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/`
- Create: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/`

- [ ] **Step 1: Create the artifact directory tree**

```bash
mkdir -p docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets
```

- [ ] **Step 2: Verify actionable row count matches expectation**

Run:
```bash
python3 -c "
import json
d = json.load(open('docs/debug/audits/2026-04-27-signal-drift-tier-b/inventory.json'))
rows = d['rows']
from collections import Counter
verdicts = Counter(r['rust_status'] for r in rows)
print('verdict counts:', dict(verdicts))
actionable = [r for r in rows if r['rust_status'] in ('drifted', 'gap-blocked')]
print('actionable count:', len(actionable))
assert len(actionable) == 178, f'expected 178 actionable rows, got {len(actionable)}'
print('OK')
"
```

Expected: `verdict counts: {'drifted': 162, 'gap-blocked': 16, ...}` and `actionable count: 178` and `OK`.

If count differs, **stop and reconcile** — the spec's row count assumption is wrong, and downstream tasks depend on it.

- [ ] **Step 3: Verify cleanup-item count matches expectation**

Run:
```bash
python3 -c "
import csv
with open('docs/debug/audits/2026-04-27-signal-drift-tier-b/preexisting-diverged.csv') as f:
    rows = list(csv.DictReader(f))
print('csv columns:', rows[0].keys() if rows else None)
print('csv row count:', len(rows))
"
```

Expected: prints column names and total row count. Confirm the count of failed/wrong-category entries against audit observation 980 and 982 (total 9). The CSV may contain additional rows beyond the 9 cleanup items; the relevant subset is identified by the re-validation status column.

If the CSV schema doesn't make the cleanup subset obvious, add a step to identify the filter expression before proceeding.

- [ ] **Step 4: Commit the empty directory tree**

```bash
git add docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/
git commit --allow-empty -m "$(cat <<'EOF'
docs(F010): scaffold signal-drift remediation artifact directory

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

(`--allow-empty` because git won't track an empty directory; the commit anchors the work in history.)

---

## Task 2: Phase 1 — Enrich inventory with four orthogonal tags

**Files:**
- Read: `docs/debug/audits/2026-04-27-signal-drift-tier-b/inventory.json`
- Read: `docs/debug/audits/2026-04-27-signal-drift-tier-b/cpp-sites.csv`
- Read: `docs/debug/audits/2026-04-27-signal-drift-tier-b/preexisting-diverged.csv`
- Create: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/pattern-catalog.md`
- Create: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/decision-catalog.md`
- Create: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json`

**Approach.** Enrichment is judgement-heavy. The working-memory session does the catalog-derivation passes (Steps 1, 3) directly. Once catalogs are stable, mechanical tag-application (Steps 2, 4, 5, 6) runs as a Python script with a lookup table. `prereq-ids` requires cross-row reasoning and is handled in Step 6 via direct session work, not script.

- [ ] **Step 1: Derive the pattern-id catalog**

Read the actionable rows. Histogram on `(rust_evidence.kind, rust_signal_accessor_status)` to find drift shapes:

```bash
python3 -c "
import json
from collections import Counter
d = json.load(open('docs/debug/audits/2026-04-27-signal-drift-tier-b/inventory.json'))
actionable = [r for r in d['rows'] if r['rust_status'] in ('drifted', 'gap-blocked')]
hist = Counter((r['rust_evidence']['kind'] if r['rust_evidence'] else 'none', r['rust_signal_accessor_status']) for r in actionable)
for (kind, status), n in hist.most_common():
    print(f'{n:4d}  evidence={kind:<20}  accessor={status}')
"
```

Then read 5–10 representative rows for each `(evidence-kind, accessor-status)` cell and synthesize a small set of pattern-ids (target: 5–12 patterns). Each pattern-id has a stable slug like `P-polling-where-subscribe-expected`, `P-u64-typed-signal-accessor`, `P-missing-accessor`.

Write `pattern-catalog.md` with one entry per pattern-id:

```markdown
## P-polling-where-subscribe-expected

**Shape:** Rust path uses field comparison or tree-walk polling instead of subscribing to an emitted signal.
**Evidence-kind:** polling
**Accessor-status:** present
**Sample rows:** emColorField-245, ...
**Mechanical-vs-judgement:** mechanical-heavy
```

Gate: every distinct `(evidence-kind, accessor-status)` cell with non-zero count maps to at least one pattern-id.

- [ ] **Step 2: Derive `scope-key` mapping (mechanical)**

`scope-key` = subsystem name extracted from `rust_file` path. Use this mapping rule, codified as a Python function:

```python
def scope_key(rust_file):
    # crates/<crate>/src/<file>.rs -> use crate name; further split for emmain
    parts = rust_file.split('/')
    if len(parts) >= 2 and parts[0] == 'crates':
        crate = parts[1]
        if crate == 'emmain':
            # emmain spans many panels; use filename stem as scope
            stem = parts[-1].rsplit('.', 1)[0]
            return f'emmain:{stem}'
        return crate  # emcore, emstocks, embookmarks, etc.
    return rust_file  # fallback for unusual paths
```

No separate output; folded into Step 6.

- [ ] **Step 3: Derive the decision-id catalog**

Walk every actionable row. For each, ask: is there a global design choice this fix depends on that, if answered differently, would change the fix? Examples to seed thinking:

- `D-001-fileman-u64-convention` — should `fileman`'s `GetChangeSignal() -> u64` pattern be flipped to `SignalId`, or kept and consumers adapted?
- `D-002-bookmarks-port-strategy` — emBookmarks has 21 unported rows; do we port the C++ subscription model wholesale, or carve a Rust-native subset first?
- `D-003-gap-blocked-fill-vs-stub` — for the 16 gap-blocked rows, do we fill the upstream-gap or stub at the consumer?

Write `decision-catalog.md` with one ADR-stub per decision-id (just the question and a 1-sentence sketch of options; full resolution happens in Task 3):

```markdown
## D-001-fileman-u64-convention

**Question:** Should `fileman`'s `u64`-typed signal accessors be flipped to `SignalId`, or preserved with consumer-side adaptation?
**Affects rows:** ~7 fileman rows + ~5 consumer rows.
**Status:** unresolved (resolved in decisions.md)
```

Aim for 3–8 decisions total. More than 8 likely means some are sub-decisions of a larger one; collapse.

Gate: catalog covers every cross-cutting concern that surfaces in the row data.

- [ ] **Step 4: Build pattern-id and decision-id assignment table**

Create a Python script `scripts/enrich_inventory.py` (one-shot, not committed to source tree — write to `/tmp/enrich_inventory.py` or similar) that:

1. Loads `inventory.json`.
2. Filters to actionable rows.
3. Applies `pattern-id` via lookup on `(evidence-kind, accessor-status, signal_kind)` triple, falling back to manual override table for outliers.
4. Applies `decision-id` list via lookup on `scope-key` + `pattern-id` (most decisions are scope-specific).
5. Applies `scope-key` via the function from Step 2.
6. Loads `preexisting-diverged.csv` cleanup items, tags each with the same schema (cleanup items get a synthetic `pattern-id` like `P-stale-annotation`).

Both lookup tables are Python dicts in the script, derived from Steps 1 and 3. Run the script:

```bash
python3 /tmp/enrich_inventory.py > /tmp/enriched-draft.json
```

Inspect a sample of rows from each pattern-id bucket; spot-check for misclassification. Iterate the lookup table until spot-checks pass.

- [ ] **Step 5: Write `inventory-enriched.json` (without prereq-ids)**

Once the script produces a clean draft, write to the final location:

```bash
cp /tmp/enriched-draft.json docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json
```

The schema of each enriched row:

```json
{
  "id": "emColorField-245",
  "...": "all original inventory.json fields preserved",
  "enrichment": {
    "pattern_id": "P-polling-where-subscribe-expected",
    "decision_ids": ["D-003-gap-blocked-fill-vs-stub"],
    "scope_key": "emcore",
    "prereq_ids": []
  }
}
```

Plus a top-level `cleanup_items` array with the 9 cleanup-item entries from `preexisting-diverged.csv`, tagged with the same `enrichment` schema.

- [ ] **Step 6: Add `prereq_ids` (judgement pass)**

Walk the enriched rows grouped by `pattern-id`. For each pattern, identify which rows depend on others (e.g., a consumer row depends on the model row that exposes the accessor; a stocks row depends on the same fix landing in emcore first). Edit the JSON in place to populate `prereq_ids` lists.

Default: empty list. Only add prereqs where the dependency is structural (a row literally cannot be fixed before another lands).

- [ ] **Step 7: Validate `inventory-enriched.json`**

Run:
```bash
python3 -c "
import json
d = json.load(open('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json'))
rows = d['rows']
cleanup = d.get('cleanup_items', [])
print(f'rows: {len(rows)}, cleanup: {len(cleanup)}')
assert len(rows) == 178, f'expected 178 actionable rows, got {len(rows)}'
assert len(cleanup) == 9, f'expected 9 cleanup items, got {len(cleanup)}'
for r in rows + cleanup:
    e = r['enrichment']
    assert e['pattern_id'], f'{r[\"id\"]} missing pattern_id'
    assert isinstance(e['decision_ids'], list), f'{r[\"id\"]} decision_ids not list'
    assert e['scope_key'], f'{r[\"id\"]} missing scope_key'
    assert isinstance(e['prereq_ids'], list), f'{r[\"id\"]} prereq_ids not list'
    # prereq_ids must reference existing rows
    all_ids = {x['id'] for x in rows + cleanup}
    for pid in e['prereq_ids']:
        assert pid in all_ids, f'{r[\"id\"]} prereq {pid} not in inventory'
print('OK')
"
```

Expected: counts match and `OK`.

If counts differ, fix the script and re-run from Step 4. If a prereq references a non-existent row, fix the JSON and re-validate.

- [ ] **Step 8: Commit**

```bash
git add docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/pattern-catalog.md \
        docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/decision-catalog.md \
        docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json
git commit -m "$(cat <<'EOF'
docs(F010): Phase 1 — enrich inventory with pattern/decision/scope/prereq tags

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Phase 2 — Resolve global decisions into `decisions.md`

**Files:**
- Read: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/decision-catalog.md`
- Read: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json`
- Create: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/decisions.md`

**Approach.** For each `D-###` from the catalog, write an ADR-style entry at sketch resolution: question, options considered, chosen direction, why. Full design fidelity is not required — Phase 3 sketches need to *cite* a stable answer, not derive it.

- [ ] **Step 1: Enumerate decision-ids referenced in enriched inventory**

Run:
```bash
python3 -c "
import json
from collections import Counter
d = json.load(open('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json'))
all_decisions = []
for r in d['rows'] + d.get('cleanup_items', []):
    all_decisions.extend(r['enrichment']['decision_ids'])
hist = Counter(all_decisions)
for did, n in hist.most_common():
    print(f'{n:4d}  {did}')
"
```

Compare against `decision-catalog.md`. Every `D-###` cited by any row must appear in the catalog. If a row cites an undefined `D-###`, **stop and add it to the catalog**, then re-run Task 2 Step 6 to ensure consistency.

- [ ] **Step 2: Draft `decisions.md` with one ADR per `D-###`**

Template per entry:

```markdown
## D-001-fileman-u64-convention

**Question:** Should `fileman`'s `u64`-typed signal accessors be flipped to `SignalId`, or preserved with consumer-side adaptation?

**Affects:** ~7 fileman rows + ~5 consumer rows. Cited by buckets touching fileman or its consumers.

**Options considered:**
- A. Flip to `SignalId`. Matches C++ contract; touches all consumers but mechanically.
- B. Keep `u64`. Preserves Rust port choice; requires consumer-side adaptation per use site.
- C. Hybrid — flip in fileman but provide a `u64` wrapper for consumers that already adapted.

**Chosen direction:** A. Flip to `SignalId`.

**Why:** Audit Task 6 re-validation found no language-forced justification for the `u64` convention; it was an unannotated divergence. Per Port Ideology, divergence without forced category is fidelity-bug. Sketch resolution — full design lives in the bucket that touches fileman.

**Open questions deferred to bucket design:** consumer migration ordering; whether to gate on a single PR or stage.
```

Resolve every `D-###` from Step 1. Resolution can be terse — the goal is a stable citation target, not a final design.

- [ ] **Step 3: Validate `decisions.md` coverage**

Run:
```bash
python3 -c "
import json, re
d = json.load(open('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json'))
referenced = set()
for r in d['rows'] + d.get('cleanup_items', []):
    referenced.update(r['enrichment']['decision_ids'])
with open('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/decisions.md') as f:
    text = f.read()
defined = set(re.findall(r'^## (D-\d{3}-[\w-]+)', text, re.MULTILINE))
missing = referenced - defined
extra = defined - referenced
print(f'referenced: {len(referenced)}, defined: {len(defined)}')
if missing: print(f'MISSING (referenced but not defined): {missing}')
if extra: print(f'EXTRA (defined but not referenced): {extra}')
assert not missing, 'decisions.md is missing entries'
print('OK')
"
```

Expected: `OK`. Extra entries are tolerated (they may be load-bearing context); missing entries are a hard fail.

- [ ] **Step 4: Commit**

```bash
git add docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/decisions.md
git commit -m "$(cat <<'EOF'
docs(F010): Phase 2 — resolve global signal-drift decisions

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Phase 3 — Cluster, sketch every bucket, then freeze

**Files:**
- Read: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json`
- Read: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/decisions.md`
- Create: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-###-<slug>.md` (one per bucket)

**Approach.** Cluster by `pattern-id` primary, `scope-key` tiebreaker. Sketch *every* bucket file before freezing any — this is the gate that prevents D-style scope-creep. Sketch authoring fans out across subagents in parallel once the bucket list is fixed.

- [ ] **Step 1: Compute candidate bucket list**

Run:
```bash
python3 -c "
import json
from collections import defaultdict
d = json.load(open('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json'))
groups = defaultdict(list)
for r in d['rows'] + d.get('cleanup_items', []):
    e = r['enrichment']
    key = (e['pattern_id'], e['scope_key'])
    groups[key].append(r['id'])
# Print groups sorted by size
for (pat, scope), ids in sorted(groups.items(), key=lambda x: -len(x[1])):
    print(f'{len(ids):4d}  {pat:<40}  {scope:<20}  ({ids[0]} ... {ids[-1]})')
print(f'TOTAL groups: {len(groups)}')
"
```

Review the group list:
- Groups smaller than 3 rows are candidates for merging into a sibling group with the same `pattern-id`.
- Groups larger than 30 rows are candidates for splitting along `scope-key` sub-axis.

Decide the final bucket list — typically 8–20 buckets total. Assign each a `B-###` ID (zero-padded) and a slug derived from `<pattern-short>-<scope-short>` (e.g. `B-001-polling-emcore`, `B-002-u64-fileman`).

Write the decision down as a top-of-task scratch list (don't commit yet — bucket files come next):

```text
B-001-polling-emcore        pattern=P-polling-where-subscribe-expected scope=emcore  rows=N
B-002-...
```

- [ ] **Step 2: Author every bucket sketch in parallel via subagents**

For each bucket on the list from Step 1, dispatch one subagent in parallel (single message, multiple Agent tool uses). Each subagent receives:
- Its `B-###` ID and slug.
- The rows assigned to it (full row data extracted from `inventory-enriched.json`).
- Pointer to `decisions.md` for cited `D-###`s.
- The sketch file template below.
- Output path: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-###-<slug>.md`.

Sketch file template (the subagent fills in the bracketed parts):

```markdown
# B-### — [Bucket title]

**Pattern:** [pattern-id]
**Scope:** [scope-key]
**Row count:** [N]
**Mechanical-vs-judgement:** [mechanical-heavy | balanced | judgement-heavy]
**Cited decisions:** [list of D-### with one-line summary of how each applies]
**Prereq buckets:** [list of B-### or "none"]

## Pattern description

[2-3 sentences describing the drift shape and the canonical fix.]

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| [row-id] | [cpp_file:cpp_line] | [rust_file:rust_evidence.line] | [accessor_status] | [1-line note] |
| ... |

## C++ reference sites

[Path:line list distilled from rows; the per-bucket brainstorm session reads these to verify the canonical fix shape.]

## Open questions for the bucket-design brainstorm

- [Anything the sketcher noticed that isn't decided by cited D-###s.]
```

Subagent prompt template (for dispatch):

> You are authoring a brainstorm-launcher file for a downstream design session. Read the strategy spec at `docs/superpowers/specs/2026-04-27-signal-drift-remediation-strategy-design.md` (Phase 3 section) and `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/decisions.md`. Write `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-###-<slug>.md` for the bucket described below using the template provided. Do not invent rows; use only the ids listed. Do not modify any other file. Report DONE when the file is written.
>
> Bucket: B-###-<slug>
> Pattern: <pattern-id>
> Scope: <scope-key>
> Row IDs: <list>
> Cited decisions: <list of D-###>

After all subagents return, list the bucket directory:

```bash
ls docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/
```

Expected: one file per bucket from Step 1, no extras.

- [ ] **Step 3: Validate row coverage and uniqueness (the freeze gate)**

Run:
```bash
python3 -c "
import json, re, os, glob
d = json.load(open('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json'))
all_ids = {r['id'] for r in d['rows'] + d.get('cleanup_items', [])}
seen = {}
for path in sorted(glob.glob('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-*.md')):
    bucket = os.path.basename(path).rsplit('.', 1)[0]
    with open(path) as f:
        text = f.read()
    # Extract row IDs from the table — they appear in the first column
    ids_in_bucket = set(re.findall(r'^\| ([a-zA-Z][\w-]+) \|', text, re.MULTILINE))
    for rid in ids_in_bucket:
        if rid in seen:
            print(f'DUPLICATE: {rid} in both {seen[rid]} and {bucket}')
        seen[rid] = bucket
missing = all_ids - set(seen)
extra = set(seen) - all_ids
print(f'expected rows: {len(all_ids)}, covered: {len(seen)}')
if missing: print(f'MISSING (no bucket): {sorted(missing)[:20]}{\"...\" if len(missing)>20 else \"\"}')
if extra: print(f'EXTRA (in bucket but not in inventory): {sorted(extra)[:20]}')
assert not missing and not extra, 'row coverage check failed'
print('FROZEN OK')
"
```

Expected: `FROZEN OK`.

If duplicates: re-dispatch the subagent for one of the affected buckets to remove the duplicate row. If missing: identify which bucket the row should belong to and re-dispatch that bucket's sketcher. **Do not silently edit bucket files** — re-dispatch keeps the sketches consistent with their templates.

This step is the freeze gate. After it passes, bucket assignments are committed.

- [ ] **Step 4: Commit**

```bash
git add docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/
git commit -m "$(cat <<'EOF'
docs(F010): Phase 3 — sketch and freeze remediation buckets

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Phase 4 — Tier and order into `work-order.md`

**Files:**
- Read: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-*.md`
- Read: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json`
- Create: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/work-order.md`

- [ ] **Step 1: Build the bucket-level prereq DAG**

A bucket-level prereq edge `B-X → B-Y` exists iff any row in B-Y has a `prereq_ids` entry that points to a row in B-X.

Run:
```bash
python3 -c "
import json, re, glob, os
from collections import defaultdict
d = json.load(open('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/inventory-enriched.json'))
row_lookup = {r['id']: r for r in d['rows'] + d.get('cleanup_items', [])}
# Map row -> bucket
row_to_bucket = {}
for path in sorted(glob.glob('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-*.md')):
    bucket = os.path.basename(path).rsplit('.', 1)[0]
    with open(path) as f:
        text = f.read()
    for rid in re.findall(r'^\| ([a-zA-Z][\w-]+) \|', text, re.MULTILINE):
        row_to_bucket[rid] = bucket
# Build bucket DAG
edges = defaultdict(set)
for rid, bucket in row_to_bucket.items():
    for prereq_rid in row_lookup[rid]['enrichment']['prereq_ids']:
        prereq_bucket = row_to_bucket.get(prereq_rid)
        if prereq_bucket and prereq_bucket != bucket:
            edges[prereq_bucket].add(bucket)  # prereq_bucket must come before bucket
import json as j
print(j.dumps({k: sorted(v) for k, v in edges.items()}, indent=2))
" > /tmp/bucket-dag.json
cat /tmp/bucket-dag.json
```

- [ ] **Step 2: Topo-sort buckets into layers; tiebreak mechanical-first**

Run:
```bash
python3 << 'PY'
import json, glob, os, re
from collections import defaultdict, deque

with open('/tmp/bucket-dag.json') as f:
    edges = json.load(f)
# All buckets
all_buckets = []
mechanical_rank = {}
for path in sorted(glob.glob('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-*.md')):
    bucket = os.path.basename(path).rsplit('.', 1)[0]
    all_buckets.append(bucket)
    with open(path) as f:
        text = f.read()
    m = re.search(r'\*\*Mechanical-vs-judgement:\*\*\s*(\S+)', text)
    label = m.group(1) if m else 'unknown'
    mechanical_rank[bucket] = {'mechanical-heavy': 0, 'balanced': 1, 'judgement-heavy': 2, 'unknown': 3}.get(label, 3)

# Reverse-build incoming-edge counts
incoming = defaultdict(int)
out = defaultdict(set)
for src, dsts in edges.items():
    for dst in dsts:
        incoming[dst] += 1
        out[src].add(dst)
for b in all_buckets:
    incoming[b] = incoming.get(b, 0)

# Kahn topo sort with mechanical tiebreaker per layer
remaining = set(all_buckets)
layers = []
while remaining:
    layer = sorted(
        [b for b in remaining if incoming[b] == 0],
        key=lambda b: (mechanical_rank[b], b),
    )
    if not layer:
        print('CYCLE in DAG! Remaining:', remaining)
        raise SystemExit(1)
    layers.append(layer)
    for b in layer:
        remaining.remove(b)
        for dst in out[b]:
            incoming[dst] -= 1

for i, layer in enumerate(layers):
    print(f'Layer {i}: {layer}')
PY
```

If a cycle is reported, **stop**: a cycle in bucket prereqs means Phase 1 prereq tagging has a logic error or two buckets need to be merged. Resolve before continuing.

- [ ] **Step 3: Write `work-order.md`**

Use the layer output from Step 2 as the source of truth. Format:

```markdown
# Signal-Drift Remediation — Work Order

**Generated:** 2026-04-27 from Phase 4 of the bookkeeping strategy.
**Total buckets:** N
**Layers:** M

Buckets are ordered by topological layer over the prereq DAG (lower layer = no unmet prereqs). Within a layer, mechanical-heavy buckets come first to validate the pattern cheaply before judgement-heavy work.

## Order

| # | Bucket | Layer | Mechanical-vs-judgement | Rows | Status | Design doc |
|---|---|---|---|---|---|---|
| 1 | B-001-<slug> | 0 | mechanical-heavy | N | pending | — |
| 2 | B-002-<slug> | 0 | balanced | N | pending | — |
| ... |

## Status legend

- `pending` — not yet picked up.
- `in-design` — a fan-out brainstorm session is currently working on this bucket.
- `designed` — design doc returned and reconciled into the spine.
- `merged` — implementation merged to `main`.

## Reconciliation log

(Phase 5 entries will be appended here as fan-out designs return and are reconciled. Each entry: date, bucket, what was reconciled, spine amendments.)
```

- [ ] **Step 4: Validate `work-order.md`**

Run:
```bash
python3 -c "
import re, glob, os
with open('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/work-order.md') as f:
    text = f.read()
listed = re.findall(r'^\|\s*\d+\s*\|\s*(B-\d{3}-[\w-]+)\s*\|', text, re.MULTILINE)
on_disk = sorted(os.path.basename(p).rsplit('.', 1)[0]
                 for p in glob.glob('docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-*.md'))
print(f'listed: {len(listed)}, on-disk: {len(on_disk)}')
assert sorted(listed) == on_disk, f'mismatch:\nonly listed: {set(listed)-set(on_disk)}\nonly on disk: {set(on_disk)-set(listed)}'
assert len(listed) == len(set(listed)), 'duplicate bucket in work-order'
print('OK')
"
```

Expected: `OK`.

- [ ] **Step 5: Commit**

```bash
git add docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/work-order.md
git commit -m "$(cat <<'EOF'
docs(F010): Phase 4 — tier and order remediation buckets

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Out of scope (Phase 5)

Phase 5 reconciliation is a standing responsibility of the working-memory session and is not a planned task. After this plan executes, the session remains open and processes returning per-bucket designs by:

- Updating `work-order.md` status column.
- Appending entries to the `## Reconciliation log` section.
- Editing `decisions.md` when a design surfaces a new global decision (and re-citing affected bucket files).
- Editing affected bucket files when row reassignment is needed.
- Re-running Task 5 Steps 1–4 if the prereq DAG changes.

The strategy spec (`docs/superpowers/specs/2026-04-27-signal-drift-remediation-strategy-design.md`) is the authoritative reference for Phase 5 behavior.
