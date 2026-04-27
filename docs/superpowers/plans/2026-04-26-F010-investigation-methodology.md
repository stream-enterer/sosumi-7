# F010 Investigation Methodology Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run the F010 X+Z investigation per `docs/superpowers/specs/2026-04-26-F010-investigation-methodology-design.md`. Produce converged evidence (one confirmed root cause + mechanical+manual confirmation) or "all hypotheses falsified, escalate," to feed a separate fix-spec cycle.

**Architecture:** Five phases. Phase 0 sets up directories. Phase 1 drafts and locks the pre-registration table (26 hypothesis entries). Phase 2 is split into per-cluster sub-phases, ordered cheapest-cluster-first; each phase-2 sub-phase builds only the harness components needed by that cluster. Phase 3 is split into matching per-cluster sub-phases that run the falsifications and resolve each cluster (or escalate). Phase 4 satisfies the termination gate (mechanical + manual) and writes the handoff. Append-only log under `docs/debug/investigations/F010-investigation/log/` records every observation, decision, confirmation, falsification, revision, and escalation.

**Tech Stack:** Rust (cargo, nextest), markdown frontmatter for log/registration entries, JSON op-stream artifacts, `scripts/diff_draw_ops.py` for C++ comparison.

**Critical references:**
- Spec: `docs/superpowers/specs/2026-04-26-F010-investigation-methodology-design.md`
- Hypothesis-category source: `docs/debug/investigations/F010-hypothesis-enumeration/synthesis-v2.md`
- Project port ideology: `CLAUDE.md`

**Phase boundaries are gated.** A phase advances only when its log entries satisfy the spec's lock rules. Mid-phase abandonment requires an `escalate` log entry.

---

## Phase 0 — Setup

### Task 0.1: Initialize investigation directory structure

**Files:**
- Create: `docs/debug/investigations/F010-investigation/README.md`
- Create: `docs/debug/investigations/F010-investigation/hypotheses/.gitkeep`
- Create: `docs/debug/investigations/F010-investigation/log/.gitkeep`
- Create: `docs/debug/investigations/F010-investigation/harness/.gitkeep`
- Create: `docs/debug/investigations/F010-investigation/artifacts/.gitkeep`

- [ ] **Step 1: Create directories**

```bash
mkdir -p docs/debug/investigations/F010-investigation/{hypotheses,log,harness,artifacts}
touch docs/debug/investigations/F010-investigation/{hypotheses,log,harness,artifacts}/.gitkeep
```

- [ ] **Step 2: Write README**

Create `docs/debug/investigations/F010-investigation/README.md` with:

```markdown
# F010 X+Z investigation

Per spec `docs/superpowers/specs/2026-04-26-F010-investigation-methodology-design.md` and plan `docs/superpowers/plans/2026-04-26-F010-investigation-methodology.md`.

## Layout

- `hypotheses/` — pre-registration entries, one YAML per hypothesis. ID-named (H1.yaml, P1.yaml, B1.yaml, etc.). Locked at end of phase 1.
- `log/` — append-only investigation log, one markdown per entry, named `NNNN-<short>.md` with monotonic 4-digit counter. Never edit prior entries; corrections are new entries with `supersedes:`.
- `harness/` — Rust test fixtures specific to F010 falsification experiments. Built per-cluster in phase 2. Each cluster's components are locked at start of that cluster's phase-3 execution.
- `artifacts/` — experiment outputs (op-stream JSON, image diffs, instrumentation traces). Cited from `observe` log entries via the `artifacts:` frontmatter field.

## Phase status

Update this section as phases complete.

- [ ] Phase 0 — setup
- [ ] Phase 1 — pre-registration drafting and lock
- [ ] Phase 2 — harness construction (per-cluster, ordered cheapest-first)
- [ ] Phase 3 — cluster-first execution
- [ ] Phase 4 — termination gate and handoff
```

- [ ] **Step 3: Commit**

```bash
git add docs/debug/investigations/F010-investigation/
git commit -m "$(cat <<'EOF'
docs(F010): initialize investigation directory structure

Layout per methodology spec: hypotheses/, log/, harness/, artifacts/.
README documents the lock rules and references the spec and plan.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 1 — Pre-registration drafting and lock

### Drafting protocol (read once before Task 1.1)

Each hypothesis becomes one YAML file in `docs/debug/investigations/F010-investigation/hypotheses/<id>.yaml` with the schema from spec Section 3:

```yaml
id: <ID>                     # H1, H2, P1, B1, etc.
short_name: "<short name from synthesis-v2>"
hypothesis_statement: |
  <precise mechanism that, if true, explains X+Z-fail-but-Y-works>
falsification_criterion: |
  <a single observation that, if seen, kills this hypothesis. Popperian
  falsification only — "evidence consistent with X" is forbidden>
experimental_design: |
  <the experiment that produces the observation. Names production code path>
evidence_shape: |
  <artifact the experiment produces — op-stream JSON, image diff, etc.>
falsification_action: |
  <what to do if falsified: which cluster mate to discriminate, or
  "no successor — cluster boundary reached">
cluster_membership: [<cluster IDs from spec Section 2>]
```

**Cross-falsification rule.** Each `falsification_criterion` must be a single observation that fires only on that hypothesis. Phrasing must be specific enough that observing the criterion does NOT incidentally falsify a cluster mate. Task 1.10 audits this property before lock.

### Task 1.1: Draft H1 pre-registration entry (worked example)

**Files:**
- Create: `docs/debug/investigations/F010-investigation/hypotheses/H1.yaml`

- [ ] **Step 1: Write H1.yaml**

```yaml
id: H1
short_name: "Recording-mode dispatch hole — Clear silently dropped"
hypothesis_statement: |
  emPainter::Clear uses require_direct() without a paired DrawOp::Clear
  variant. When the painter target is PaintTarget::DrawList (recording
  mode used by the parallel tile compositor at emViewRenderer.rs:74),
  Clear early-returns silently. The tile pre-fill (view.background_color)
  shows through at the panel interior because emDirPanel::Paint's
  Clear(DirContentColor) call records nothing.
falsification_criterion: |
  Construct a recording painter targeting an empty DrawList. Call
  painter.Clear(emColor::RED). After the call, inspect the DrawList ops
  vec. If the ops vec contains any op (regardless of variant) that was
  added by the Clear call, H1 is falsified.
experimental_design: |
  Unit test in crates/emcore/tests/f010_h1_clear_recording.rs.
  - Create emPainter::new_recording.
  - Call painter.Clear(emColor::rgba(255, 0, 0, 255)).
  - Assert the underlying DrawList ops vec is empty (or contains zero
    ops contributed by Clear). Compare against PaintRect baseline (which
    DOES record).
  Production code path: same emPainter::Clear function called by
  emDirPanel::Paint at emfileman/src/emDirPanel.rs:459.
evidence_shape: |
  Test pass/fail. JSON serialization of the DrawList ops vec, written to
  artifacts/H1-ops.json. Pass = test pass + ops vec empty after Clear.
falsification_action: |
  If H1 falsified, advance to P8 (cluster mate, same-observable-with-H1)
  for cluster discrimination. P8 examines whether the rect emDirPanel
  passes to Clear is degenerate (zero-area) at the symptomatic zoom.
cluster_membership: ["same-observable-with-H1"]
```

- [ ] **Step 2: Write phase-1 log entry decide-0001**

Create `docs/debug/investigations/F010-investigation/log/0001-phase1-start.md`:

```markdown
---
id: 0001
type: decide
timestamp: 2026-04-26T10:00:00Z
hypothesis_ids: []
supersedes: null
artifacts: []
---

# Phase 1 begins

Pre-registration drafting initiated. Hypothesis category checklist comprises 19 hypotheses (H1, H2, H3-H6, H7-H11, P1, P2, P3, P4, P5, P7, P8) and 8 blind spots (B1-B8) per spec Section 2 and synthesis-v2.md Tier 1/2/3.

Cluster memberships:
- same-observable-with-H1: H1, P8
- invalidation-cluster: P2, B1, B8
- dispatch-cluster: P3, B2, B3
- order-config-cluster: P4, B5, H11, P5

H1 drafted as worked example (Task 1.1). Remaining entries follow the same schema.
```

- [ ] **Step 3: Commit**

```bash
git add docs/debug/investigations/F010-investigation/hypotheses/H1.yaml \
        docs/debug/investigations/F010-investigation/log/0001-phase1-start.md
git commit -m "$(cat <<'EOF'
docs(F010): draft H1 pre-registration entry; phase 1 begins

H1 is the first pre-registration entry, used as a worked example for the
remaining 26. Falsification criterion is Popperian: assert no recorded
op is added when emPainter::Clear is called on a DrawList target.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.2: Draft P8 (cluster mate of H1)

**Files:**
- Create: `docs/debug/investigations/F010-investigation/hypotheses/P8.yaml`

- [ ] **Step 1: Write P8.yaml**

```yaml
id: P8
short_name: "Coordinate-rounding to zero-area degenerate rects"
hypothesis_statement: |
  At the symptomatic zoom, the f64→i32 rect computation in either
  emDirPanel::Paint's Clear-target rect or emPainter::Clear's clip rect
  rounds to a zero-area i32 pixel rect (w==0 or h==0). The rect is
  computed correctly but quantizes to nothing visible. Same observable as
  H1 (Clear is a no-op) but mechanism is geometric, not dispatch.
falsification_criterion: |
  Compute the i32 pixel rect that emDirPanel::Paint would clear at the
  symptomatic zoom (the user's reproducing scenario — Card-Blue, zoomed
  into a directory). Specifically: read state.clip from the painter and
  apply the same x1/y1/x2/y2 rounding emPainter::Clear uses
  (emPainter.rs:5779-5782). If the resulting (w, h) has w > 0 AND h > 0,
  P8 is falsified — the rect is non-degenerate.
experimental_design: |
  Unit test in crates/emcore/tests/f010_p8_zero_area.rs.
  - Build a panel state matching the symptomatic zoom (use real_stack
    from existing emfileman tests as the source for plausible state values).
  - Construct a direct painter (PaintTarget::emImage), drive
    emDirPanel::Paint into it, capture the painter's state.clip at the
    moment Clear is called.
  - Apply the round formula manually and emit (w, h) as JSON.
  Production code path: emDirPanel.rs:454-464 + emPainter.rs:5775-5784.
evidence_shape: |
  Test pass/fail. JSON file at artifacts/P8-rect.json containing the
  computed (w, h) and the source state.clip values. Pass criterion:
  (w > 0 AND h > 0).
falsification_action: |
  If P8 falsified (rect non-degenerate), the cluster's discrimination
  succeeds (H1 confirmed by Task 1.1's experiment, P8 falsified here →
  cluster resolved with H1 as cause). If P8 confirmed (rect zero-area)
  AND H1 also confirmed (Clear records nothing), the cluster cannot be
  discriminated by these experiments — escalate.
cluster_membership: ["same-observable-with-H1"]
```

- [ ] **Step 2: Commit**

```bash
git add docs/debug/investigations/F010-investigation/hypotheses/P8.yaml
git commit -m "$(cat <<'EOF'
docs(F010): draft P8 pre-registration — cluster mate of H1

P8 hypothesizes coordinate-rounding to zero-area as the mechanism behind
the same observable H1 produces. Falsification: compute the i32 rect
emDirPanel::Paint would clear at symptomatic zoom; if non-degenerate
(w > 0 AND h > 0), P8 is killed.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.3: Draft H2 (singleton — tile pre-fill)

**Files:**
- Create: `docs/debug/investigations/F010-investigation/hypotheses/H2.yaml`

- [ ] **Step 1: Write H2.yaml**

```yaml
id: H2
short_name: "Tile pre-fill / view.background_color contract"
hypothesis_statement: |
  view.background_color is captured per-frame at emWindow.rs:620-621 and
  used to pre-fill tiles at emWindow.rs:640,683 and the compositor
  pre-fills at emViewRendererCompositor.rs:48,99,111. If
  view.background_color resolves to BLACK (or TRANSPARENT, which the
  wgpu LoadOp::Clear then composites against another BLACK), the
  pre-fill is what shows whenever a panel paint operation does not
  write to those pixels. This is downstream-of-H1 in causal terms but
  testable independently: confirm that view.background_color is BLACK
  when the symptom reproduces.
falsification_criterion: |
  Query view.background_color at the moment emView::Paint runs in the
  symptom-reproducing scenario. If the value is anything other than
  BLACK or TRANSPARENT (e.g. light grey matching DirContentColor), H2
  is falsified — the visible black is not the pre-fill.
experimental_design: |
  Add an instrumentation print at emWindow.rs near line 620 that emits
  view.background_color (as RGBA hex). Build the GUI binary with this
  instrumentation; reproduce the symptom; capture the emitted value.
  Same scenario can also be exercised in a unit test that constructs an
  emView with the production initial config and reads
  view.background_color.
evidence_shape: |
  Stdout capture from the instrumented GUI run AND the unit-test
  output, both saved to artifacts/H2-bgcolor.txt. Pass criterion: value
  is BLACK (0x000000FF) OR TRANSPARENT (0x00000000).
falsification_action: |
  If H2 falsified (background_color is not black), then the visible
  black has another source — most likely a missing paint op that should
  have written DirContentColor. Continue to next cluster (the
  same-observable-with-H1 cluster, if not already run).
cluster_membership: []
```

- [ ] **Step 2: Commit**

```bash
git add docs/debug/investigations/F010-investigation/hypotheses/H2.yaml
git commit -m "$(cat <<'EOF'
docs(F010): draft H2 pre-registration — tile pre-fill / background_color

H2 examines whether the visible black is the tile pre-fill showing
through. Cheap to falsify: instrument emWindow at the per-frame
background_color capture site.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.4: Draft dispatch-cluster entries (P3, B2, B3)

**Files:**
- Create: `docs/debug/investigations/F010-investigation/hypotheses/P3.yaml`
- Create: `docs/debug/investigations/F010-investigation/hypotheses/B2.yaml`
- Create: `docs/debug/investigations/F010-investigation/hypotheses/B3.yaml`

- [ ] **Step 1: Write P3.yaml**

```yaml
id: P3
short_name: "Virtual-method / trait-dispatch override missing in Rust port"
hypothesis_statement: |
  emDirPanel's Paint override is not actually dispatched in Rust at the
  trait-impl level. Either the trait method name in
  emfileman/src/emDirPanel.rs is shadowed by a base trait default, or
  the impl is not registered for the panel-behavior trait object,
  causing the parent's default Paint (a no-op or a different color) to
  run. Frame B presumes Clear IS called and just gets dropped at the
  painter layer; P3 says Clear is never called from emDirPanel because
  the override never dispatches.
falsification_criterion: |
  Add an instrumentation print at the very first line of
  <emDirPanel as PanelBehavior>::Paint (or the trait-method equivalent)
  in emfileman/src/emDirPanel.rs. Reproduce the symptom. If the print
  fires for the symptomatic panel, P3 is falsified — the override IS
  dispatched.
experimental_design: |
  Modify emfileman/src/emDirPanel.rs to add `eprintln!("emDirPanel::Paint
  invoked panel_id={:?}", state.panel_id);` at the entry of the Paint
  trait method. Rebuild GUI; reproduce; capture stderr.
evidence_shape: |
  stderr capture from the GUI run, saved to artifacts/P3-paint-trace.txt.
  Pass criterion: at least one line containing "emDirPanel::Paint
  invoked" with a panel_id matching the symptomatic panel.
falsification_action: |
  If P3 falsified, dispatch reaches emDirPanel — advance to B2
  (state-machine arm) or B3 (paint-not-reached, but for the parent
  panel). If P3 confirmed (Paint never invoked), the cause is upstream
  of paint primitives entirely; halt cluster and report.
cluster_membership: ["dispatch-cluster"]
```

- [ ] **Step 2: Write B2.yaml**

```yaml
id: B2
short_name: "Panel state machine reaches VFS_LOADED in production"
hypothesis_statement: |
  emDirPanel::Paint's Clear-emitting arm fires only when GetVirFileState
  returns VFS_LOADED or VFS_NO_FILE_MODEL (emDirPanel.rs:454-460). If
  production state never reaches Loaded for the symptomatic panel — or
  reaches it briefly before transitioning to a state where Clear isn't
  emitted — Paint runs the default arm (delegating to
  file_panel.Paint), which uses a different paint shape. Headless tests
  reach Loaded; production may not.
falsification_criterion: |
  Add an instrumentation print at emDirPanel.rs:454 emitting the value
  returned by self.file_panel.GetVirFileState() at every Paint
  invocation for the symptomatic panel. If any invocation observes
  VFS_LOADED or VFS_NO_FILE_MODEL while the symptom is visible, B2 is
  falsified — the gated arm IS reached.
experimental_design: |
  Modify emDirPanel.rs Paint method body: add `eprintln!("emDirPanel
  Paint vfs_state={:?}", state)` at line 454 (right before the match).
  Rebuild GUI; reproduce; capture stderr.
evidence_shape: |
  stderr capture saved to artifacts/B2-vfs-state.txt. Pass criterion:
  at least one line shows VFS_LOADED or VFS_NO_FILE_MODEL during the
  symptom-reproducing window.
falsification_action: |
  If B2 falsified, advance to B3 (paint-not-reached at all by parent
  cull) or close cluster with H1/P8 reactivation. If B2 confirmed
  (state never reaches Loaded), the cause is in the state-machine
  pipeline upstream of paint.
cluster_membership: ["dispatch-cluster"]
```

- [ ] **Step 3: Write B3.yaml**

```yaml
id: B3
short_name: "Paint-not-reached for symptomatic panel (parent cull / viewport clip)"
hypothesis_statement: |
  A parent panel's IsOpaque cascade or viewport-clip culls the
  emDirPanel before Paint dispatch reaches it. The dispatch never
  arrives at the trait method (P3 examines whether the trait method is
  invoked AT ALL; B3 examines whether dispatch even attempts to reach
  it). Adjacent to P3 but distinct: P3 = wrong method invoked; B3 =
  no method invocation attempted.
falsification_criterion: |
  Add an instrumentation print at emView.rs:4796 (paint_one_panel entry,
  per architectural-grounding.md Layer 5) emitting the panel id about
  to be painted. If any invocation has a panel_id matching the
  symptomatic emDirPanel, B3 is falsified — the panel is reached.
experimental_design: |
  Modify emView.rs paint_one_panel: add `eprintln!("paint_one_panel
  panel_id={:?}", panel_id)` at the first line of the function.
  Rebuild GUI; reproduce; capture stderr.
evidence_shape: |
  stderr capture saved to artifacts/B3-paint-dispatch.txt. Pass
  criterion: stderr contains a line with the symptomatic panel's id
  during the symptom-reproducing window.
falsification_action: |
  If B3 falsified, paint dispatch reaches the panel — fall back to
  P3/B2 within cluster. If B3 confirmed (panel never reached), upstream
  cull is the cause; halt cluster and report.
cluster_membership: ["dispatch-cluster"]
```

- [ ] **Step 4: Commit**

```bash
git add docs/debug/investigations/F010-investigation/hypotheses/P3.yaml \
        docs/debug/investigations/F010-investigation/hypotheses/B2.yaml \
        docs/debug/investigations/F010-investigation/hypotheses/B3.yaml
git commit -m "$(cat <<'EOF'
docs(F010): draft dispatch-cluster pre-registration (P3, B2, B3)

P3, B2, B3 examine whether emDirPanel::Paint is dispatched at all and
in which state. All three falsification experiments use eprintln-style
instrumentation at distinct dispatch sites; one rebuild-and-reproduce
cycle exercises all three.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.5: Draft invalidation-cluster entries (P2, B1, B8)

**Files:**
- Create: `docs/debug/investigations/F010-investigation/hypotheses/P2.yaml`
- Create: `docs/debug/investigations/F010-investigation/hypotheses/B1.yaml`
- Create: `docs/debug/investigations/F010-investigation/hypotheses/B8.yaml`

- [ ] **Step 1: Write P2.yaml**

```yaml
id: P2
short_name: "State-transition idempotency / hot-reload cache invalidation"
hypothesis_statement: |
  A theme/VFS state-transition handler runs an init-once recompute on
  first invocation, then is silently skipped on re-entry due to a
  flag-cleared-without-recompute idempotency bug. After the first
  paint, subsequent state transitions don't refresh derived values like
  DirContentColor — they remain at default/zero/black. Y is unaffected
  because borders are pre-loaded at theme-init.
falsification_criterion: |
  Add an instrumentation print at every theme-reload and VFS state
  transition site: log "transition fired" + the post-transition value
  of theme.DirContentColor (and any other derived values relevant to
  emDirPanel::Paint). Reproduce. If every transition's
  post-transition DirContentColor value is light-grey, P2 is falsified.
experimental_design: |
  Locate theme-reload site in
  crates/emfileman/src/emFileManViewConfig.rs and the VFS state-machine
  transitions in crates/emfileman/src/emDirModel.rs (Cycle and
  related). Add eprintln at each transition emitting the transition
  type + theme.DirContentColor (RGBA hex). Rebuild; reproduce; capture.
evidence_shape: |
  stderr capture saved to artifacts/P2-transitions.txt. Pass criterion:
  every emitted DirContentColor value is light-grey (specifically:
  alpha == 255 AND not (R==G==B==0)).
falsification_action: |
  If P2 falsified, advance to B1 (initial parse correctness, distinct
  from drift) or B8 (compositor invalidation, downstream). If P2
  confirmed, the invalidation bug is in this handler; report and
  cluster resolves.
cluster_membership: ["invalidation-cluster"]
```

- [ ] **Step 2: Write B1.yaml**

```yaml
id: B1
short_name: "Theme/runtime-data correctness — DirContentColor parses to expected light-grey"
hypothesis_statement: |
  The DirContentColor value at runtime is wrong — the theme parser
  produces black or transparent rather than the expected light-grey,
  even on initial load. P2 is "parses-right-then-drifts"; B1 is
  "parses-wrong-from-the-start." Distinct because the falsification
  evidence differs: B1 is killed by reading the parsed value once at
  theme-init; P2 requires running multiple state transitions.
falsification_criterion: |
  Read theme.DirContentColor immediately after emFileManViewConfig::Acquire
  returns (i.e. after theme-init completes, before any transitions). If
  the value's R, G, or B channel is greater than zero AND alpha == 255,
  B1 is falsified — initial parse is correct.
experimental_design: |
  Unit test in crates/emfileman/tests/f010_b1_theme_parse.rs that
  constructs an emFileManViewConfig with the production theme name
  ("Glass1" or whatever Card-Blue resolves to), reads
  theme.DirContentColor, and asserts the value matches the expected
  light-grey (any of: pre-known expected RGB values from the .emTmp
  source, OR a sanity check that R==G==B and 0x80 <= R <= 0xFF and
  alpha == 255).
evidence_shape: |
  Test pass/fail. JSON dump of the parsed value at
  artifacts/B1-theme-parse.json. Pass criterion as above.
falsification_action: |
  If B1 falsified, theme parse is correct — advance to P2 (drift) or
  B8 (compositor). If B1 confirmed, theme parse is wrong; report and
  cluster resolves.
cluster_membership: ["invalidation-cluster"]
```

- [ ] **Step 3: Write B8.yaml**

```yaml
id: B8
short_name: "Compositor dirty-tile invalidation timing across state transitions"
hypothesis_statement: |
  When the panel transitions VFS_WAITING → VFS_LOADED, the compositor's
  dirty-tile flag for the panel's tile is not set. The compositor
  presents the prior frame's tile (painted in WAITING state, when Clear
  wasn't fired or fired differently) instead of re-rendering. Y is
  unaffected because borders are painted on a different tile or the
  border-image painting shares no transition-conditional code path.
falsification_criterion: |
  Add an instrumentation print at the compositor dirty-tile flag-set
  site (emViewRendererCompositor.rs / emViewRendererTileCache.rs). At
  every VFS_WAITING → VFS_LOADED transition, log whether the
  emDirPanel's tile is set dirty. Reproduce. If at least one such
  transition shows dirty=true for the symptomatic tile, B8 is
  falsified.
experimental_design: |
  Locate dirty-tile flag-set in emViewRendererCompositor.rs and/or
  emViewRendererTileCache.rs. Add eprintln at the flag-set site
  emitting the panel id and dirty value. Cross-reference with the VFS
  transition log (P2's instrumentation) to identify the
  WAITING→LOADED transition for the symptomatic panel.
evidence_shape: |
  stderr capture saved to artifacts/B8-dirty-tile.txt, cross-referenced
  with P2's transition log. Pass criterion: at least one
  WAITING→LOADED transition has the symptomatic tile flagged dirty.
falsification_action: |
  If B8 falsified, compositor invalidation is correct — fall back to
  P2/B1 within cluster. If B8 confirmed, compositor invalidation is
  the bug; report.
cluster_membership: ["invalidation-cluster"]
```

- [ ] **Step 4: Commit**

```bash
git add docs/debug/investigations/F010-investigation/hypotheses/P2.yaml \
        docs/debug/investigations/F010-investigation/hypotheses/B1.yaml \
        docs/debug/investigations/F010-investigation/hypotheses/B8.yaml
git commit -m "$(cat <<'EOF'
docs(F010): draft invalidation-cluster pre-registration (P2, B1, B8)

P2 = parse-right-then-drifts (state-transition handler bug). B1 =
parse-wrong-from-start (theme parser bug). B8 = compositor dirty-tile
flag missing (presentation-time bug). Distinct falsification
artifacts: P2 needs transition log; B1 needs single value at init;
B8 needs flag-set site instrumentation.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.6: Draft order-config-cluster entries (P4, B5, H11, P5)

**Files:**
- Create: `docs/debug/investigations/F010-investigation/hypotheses/P4.yaml`
- Create: `docs/debug/investigations/F010-investigation/hypotheses/B5.yaml`
- Create: `docs/debug/investigations/F010-investigation/hypotheses/H11.yaml`
- Create: `docs/debug/investigations/F010-investigation/hypotheses/P5.yaml`

- [ ] **Step 1: Write P4.yaml**

```yaml
id: P4
short_name: "Non-deterministic async-task ordering between paint-prep stages"
hypothesis_statement: |
  Paint-prep involves multiple async stages (VFS query, theme
  resolution, font-cache prepopulation). If their completion order is
  non-deterministic, the symptomatic launch sees X+Z's prep stages
  complete after the paint frame consumes them; Y's border-image is
  always ready at theme-init. Bug is sometimes-reproducing.
falsification_criterion: |
  Pin all async paint-prep tasks to a deterministic order (force a
  single-threaded runtime, or add explicit serialization at task-spawn
  sites). Reproduce. If the symptom persists with pinned ordering, P4
  is falsified — order is not the cause.
experimental_design: |
  Set RAYON_NUM_THREADS=1 (or analogous serialization env var) and
  rebuild/run GUI; reproduce. Also: identify async-spawn sites in
  emfileman async paths (emDirModel, theme-resolution) and replace
  with synchronous equivalents in a feature-gated branch
  --features serialize_paint_prep. Reproduce in this branch.
evidence_shape: |
  Manual GUI verification under each ordering condition + screenshots
  at artifacts/P4-pinned-ordering-screenshot.png. Pass criterion:
  symptom is gone in one of the pinned-ordering runs.
falsification_action: |
  If P4 falsified (symptom persists with pinning), advance to B5 (font
  cache specifically) or H11 (debug_assert) or P5 (cfg-gating). If P4
  confirmed (symptom vanishes), order-dependence is the cause.
cluster_membership: ["order-config-cluster"]
```

- [ ] **Step 2: Write B5.yaml**

```yaml
id: B5
short_name: "Font cache initialization order vs first paint"
hypothesis_statement: |
  The global font cache becomes populated lazily — on first character
  request. If paint runs before the cache is populated, glyphs render
  as empty rectangles or transparent, which composite to whatever the
  tile pre-fill is (often black). Y has no font dependency.
falsification_criterion: |
  Force-populate the font cache before first paint (call atlas() or
  GetChar on every character that the symptomatic info pane will
  render). Reproduce. If the symptom persists with cache pre-populated,
  B5 is falsified.
experimental_design: |
  Add a pre-paint hook (in emGUIFramework.rs init) that calls
  emFontCache::atlas() and GetChar for ASCII printable + glyphs the
  symptomatic info pane uses (Type label, Permissions label, etc.).
  Rebuild; reproduce.
evidence_shape: |
  Manual GUI verification + screenshot at
  artifacts/B5-prepopulated-cache-screenshot.png. Pass criterion:
  symptom gone in pre-populated run.
falsification_action: |
  If B5 falsified, font init is not the cause — fall back to P4 / H11.
  If B5 confirmed, font cache populate-on-first-paint is the bug.
cluster_membership: ["order-config-cluster"]
```

- [ ] **Step 3: Write H11.yaml**

```yaml
id: H11
short_name: "debug_assert! compiled out in release / push-pop pairing"
hypothesis_statement: |
  emView.rs:4714 has `debug_assert!(scale=1)` and other invariants
  enforced only in debug builds. In release, the assertion is compiled
  out and the actual code may run with scale != 1, producing
  mis-transformed paints that go to invisible coordinates (e.g.
  zero-area rect, off-screen). Push/pop pairing assumptions may also
  silently break.
falsification_criterion: |
  Build with `--release` AND with `RUSTFLAGS=-C debug_assertions=true`
  (forcing assertions on in release). Reproduce in both. If the symptom
  persists in the assertions-on-release build, H11 is falsified —
  invariants are not the cause.
experimental_design: |
  cargo build --release; capture symptom (or non-symptom).
  RUSTFLAGS="-C debug-assertions" cargo build --release; capture
  symptom. If the second build does NOT panic AND symptom persists,
  H11 falsified.
evidence_shape: |
  Manual GUI verification + screenshots. Logs of any panics at
  artifacts/H11-assertions-on.txt. Pass criterion: assertions-on build
  doesn't panic AND symptom persists.
falsification_action: |
  If H11 falsified, advance to P5 (cfg-gating beyond debug_assert). If
  H11 confirmed (assertion fires in assertions-on release, or symptom
  vanishes), invariant violation is the cause.
cluster_membership: ["order-config-cluster"]
```

- [ ] **Step 4: Write P5.yaml**

```yaml
id: P5
short_name: "Build-config-conditional code path (general cfg-gating, not just debug_assert)"
hypothesis_statement: |
  A code path conditionally compiled in via cfg(feature=...) or
  cfg(target_*) routes panel-interior paint differently in release vs
  debug, or with vs without a feature flag. Symptom is build-config-
  specific.
falsification_criterion: |
  Enumerate every `cfg(...)` directive in
  crates/{emcore,emfileman,eaglemode}/src/**/*.rs that touches paint or
  panel code. Build and reproduce under each cfg combination relevant
  to F010 (debug, release, all default features, no default features).
  If symptom is invariant across all combinations, P5 is falsified.
experimental_design: |
  rg 'cfg\(' crates/{emcore,emfileman,eaglemode}/src/ to enumerate. For
  each cfg, build the corresponding profile/feature combination,
  reproduce, observe.
evidence_shape: |
  Matrix table at artifacts/P5-cfg-matrix.md listing each
  build-config combination + symptom-present/absent. Pass criterion:
  every combination shows symptom-present.
falsification_action: |
  If P5 falsified (every config reproduces), config is not the cause —
  cluster exhausted, advance to next cluster. If P5 confirmed (some
  config doesn't reproduce), the diverging config identifies the bug
  surface.
cluster_membership: ["order-config-cluster"]
```

- [ ] **Step 5: Commit**

```bash
git add docs/debug/investigations/F010-investigation/hypotheses/P4.yaml \
        docs/debug/investigations/F010-investigation/hypotheses/B5.yaml \
        docs/debug/investigations/F010-investigation/hypotheses/H11.yaml \
        docs/debug/investigations/F010-investigation/hypotheses/P5.yaml
git commit -m "$(cat <<'EOF'
docs(F010): draft order-config-cluster pre-registration (P4, B5, H11, P5)

Cluster examines order-, env-, and build-config-dependent reproductions.
Heaviest cluster — requires multiple build profiles and pinned-runtime
reruns. Scheduled last in execution order per spec Section 5.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.7: Draft Tier-2 standalone hypotheses (H3, H4, H5, H6, P1)

**Files:**
- Create: `docs/debug/investigations/F010-investigation/hypotheses/{H3,H4,H5,H6,P1}.yaml`

- [ ] **Step 1: Write H3.yaml**

```yaml
id: H3
short_name: "Render-strategy split — bug fires only in display-list branch"
hypothesis_statement: |
  emWindow::render has three branches (viewport-buffer at lines
  636-660, parallel display-list at 661-676, per-tile direct at
  677-696). Only the display-list branch uses recording mode where
  H1's no-op fires. The other two use direct mode. The bug fires only
  when display-list is selected (when render_pool.GetThreadCount() > 1
  AND dirty_count > 1).
falsification_criterion: |
  Force the per-tile direct branch by setting
  render_pool.GetThreadCount() = 1 (or kill the dirty_count > 1
  condition at emWindow.rs:661). Reproduce. If the symptom persists in
  the per-tile direct branch, H3 is falsified — branch-conditional is
  not the cause.
experimental_design: |
  Set EM_RENDER_THREADS=1 env var (or analogous; if not present, add
  one as instrumentation), or inline-edit emWindow.rs:661 to force
  per-tile branch. Rebuild; reproduce.
evidence_shape: |
  Manual GUI verification + screenshot at
  artifacts/H3-pertile-screenshot.png. Pass criterion: symptom gone
  when per-tile branch is forced.
falsification_action: |
  If H3 falsified, branch-condition is not the issue — but per-tile
  branch could still hit zero-area or another mechanism. Advance to
  Tier-3 singletons. If H3 confirmed, the display-list branch
  specifically has the bug.
cluster_membership: []
```

- [ ] **Step 2: Write H4.yaml**

```yaml
id: H4
short_name: "Texture-sampling at replay (font-atlas specific)"
hypothesis_statement: |
  PaintText records DrawOp::PaintText but at replay the font cache
  atlas reference is wrong (uninitialized in worker thread, stale
  pointer, or atlas image was evicted). Glyphs render to wrong pixels
  or transparent. Y is unaffected because border-image uses caller-
  supplied emImage, not the font atlas.
falsification_criterion: |
  Construct a recording painter, record a PaintText op for one
  character. Replay into an emImage. Compare the replayed pixels to a
  direct-mode rendering of the same PaintText call. If the pixel
  outputs match (within a tight tolerance), H4 is falsified.
experimental_design: |
  Unit test in crates/emcore/tests/f010_h4_text_replay.rs:
  - Setup font cache with one glyph populated.
  - Record PaintText("A", ...) into a DrawList.
  - Replay into emImage A.
  - Direct-render PaintText("A", ...) into emImage B.
  - Diff A and B; assert pixels equal (or within ε).
evidence_shape: |
  Test pass/fail; PPM diff at artifacts/H4-text-replay-diff.ppm. Pass
  criterion: pixel diff is zero (or within tolerance).
falsification_action: |
  If H4 falsified, advance to other Tier-3 candidates. If H4 confirmed,
  the font-atlas at replay is buggy.
cluster_membership: []
```

- [ ] **Step 3: Write H5.yaml**

```yaml
id: H5
short_name: "Tile composite alpha re-blend at compositor"
hypothesis_statement: |
  emViewRendererCompositor.rs:99 uses BlendState::ALPHA_BLENDING in the
  composite pass. Tiles containing semi-transparent pixels (e.g.
  anti-aliased text on a transparent canvas) get re-blended against
  the framebuffer at composite time, darkening or shifting their
  effective color from what direct rasterization produced.
falsification_criterion: |
  Render the symptomatic scene through both paths: (a) full pipeline
  (record → replay → composite), (b) direct mode bypassing composite.
  If the per-pixel output of (a) and (b) match within tolerance for
  the symptomatic panel area, H5 is falsified.
experimental_design: |
  Unit test in crates/emcore/tests/f010_h5_composite.rs:
  - Drive emDirPanel::Paint scenario through both pipelines into
    matching-size emImages.
  - Diff the panel-interior pixel range.
  - Assert max channel diff <= tolerance (e.g. 4).
evidence_shape: |
  Test pass/fail; PPM diff at artifacts/H5-composite-diff.ppm. Pass
  criterion: max diff within tolerance.
falsification_action: |
  If H5 falsified, composite re-blend is not the cause. If H5
  confirmed, composite blend mode is wrong.
cluster_membership: []
```

- [ ] **Step 4: Write H6.yaml**

```yaml
id: H6
short_name: "DrawList replay state-snapshot equivalence"
hypothesis_statement: |
  RecordedState (emPainter.rs:646-658) is captured per op but not
  consulted at replay time (emPainterDrawList.rs:438+). Replay relies
  on the painter's *running* state via push/pop/SetClipping DrawOps. If
  any state mutation bypasses record_state OR a DrawOp omits a state
  change, replay desyncs from recording, producing wrong clips, wrong
  transforms, or wrong canvas colors.
falsification_criterion: |
  Compare each RecordedState snapshot in a recorded DrawList against
  the replay-time state computed by stepping through the same ops in
  the painter. If every op's snapshot matches the replay-time state at
  that op, H6 is falsified.
experimental_design: |
  Unit test in crates/emcore/tests/f010_h6_state_equivalence.rs that
  records a representative panel scenario, then replays op-by-op while
  comparing painter state to RecordedState at each step.
evidence_shape: |
  Test pass/fail; mismatch report at artifacts/H6-state-mismatches.json
  if any. Pass criterion: zero mismatches.
falsification_action: |
  If H6 falsified, state equivalence holds. If H6 confirmed, state
  desync identifies the failure.
cluster_membership: []
```

- [ ] **Step 5: Write P1.yaml**

```yaml
id: P1
short_name: "GPU/atlas resource lifecycle (eviction during record→replay window)"
hypothesis_statement: |
  Between recording and replay, a GPU resource handle (font atlas
  texture, image binding) is evicted/recycled, leaving the DrawOp's
  stored pointer/handle pointing at stale or empty memory. Replay
  samples garbage or transparent. Y is unaffected because border-image
  textures are pinned (long-lived, low eviction priority); glyph atlas
  pages are eviction-eligible.
falsification_criterion: |
  Instrument every resource-handle alloc/free/recycle site from record
  start to replay end. If no eviction or recycle event occurs during
  the record→replay window for any resource referenced by the
  recorded DrawList, P1 is falsified.
experimental_design: |
  Add eprintln/log calls at GPU resource alloc/free in
  emViewRendererCompositor.rs and font-cache atlas eviction in
  emFontCache. Record DrawList; pause; before replay, log all
  outstanding resource handles. Compare against post-replay logs.
evidence_shape: |
  Log capture at artifacts/P1-resource-lifecycle.log. Pass criterion:
  no eviction/recycle events occur for any resource the DrawList
  references during the record→replay window.
falsification_action: |
  If P1 falsified, lifecycle is sound. If P1 confirmed, eviction
  during replay window is the cause.
cluster_membership: []
```

- [ ] **Step 6: Commit**

```bash
git add docs/debug/investigations/F010-investigation/hypotheses/{H3,H4,H5,H6,P1}.yaml
git commit -m "$(cat <<'EOF'
docs(F010): draft Tier-2 standalone pre-registrations

H3 (render-strategy split), H4 (font-atlas at replay), H5 (composite
alpha re-blend), H6 (replay state snapshot equivalence), P1 (GPU/atlas
resource lifecycle). All standalone; no cluster mates.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.8: Draft Tier-3 standalone hypotheses (H7, H8, H9, H10, P7)

**Files:**
- Create: `docs/debug/investigations/F010-investigation/hypotheses/{H7,H8,H9,H10,P7}.yaml`

- [ ] **Step 1: Write H7.yaml**

```yaml
id: H7
short_name: "Send-Sync soundness for DrawOp *const emImage variants"
hypothesis_statement: |
  emPainterDrawList.rs:381,415 has unsafe impl Send/Sync for DrawOp
  variants holding *const emImage. If a panel mutates an emImage
  between record and replay (or the parallel-replay workers race on
  it), replay reads stale or invalid pixels.
falsification_criterion: |
  Audit every code path that mutates a referenced emImage. If every
  such mutation occurs strictly outside any record→replay window
  (e.g. only at theme-init, never during paint), H7 is falsified.
experimental_design: |
  rg pattern '&mut .*emImage' in emfileman/, emcore/. Trace each
  mutation site to its caller; verify caller is not invoked during
  paint. Static analysis; no runtime test.
evidence_shape: |
  Audit report at artifacts/H7-mutation-audit.md listing every
  mutation site + lifetime classification. Pass criterion: every site
  is paint-orthogonal.
falsification_action: |
  If H7 falsified, soundness holds. If H7 confirmed, race or aliasing
  bug.
cluster_membership: []
```

- [ ] **Step 2: Write H8.yaml**

```yaml
id: H8
short_name: "GPU pipeline (sRGB / surface clear / present)"
hypothesis_statement: |
  emViewRendererCompositor.rs:174 uses Rgba8UnormSrgb for the surface
  texture format. Bytes written through the linear sampler are read
  back through gamma, causing channel value drift. Could darken text
  or shift solid colors uniformly. Y working argues against a global
  GPU bug, but partial drift could still affect specific channels.
falsification_criterion: |
  Construct an emImage with known pixel values (e.g. RGBA(128,128,128,255)
  for grey). Upload via the same write_texture path the compositor uses.
  Read back via render-to-texture. If readback values match the source
  within ε, H8 is falsified.
experimental_design: |
  Unit test against the wgpu pipeline using a synthetic image and the
  same sRGB texture format. Compare upload-then-readback.
evidence_shape: |
  Test pass/fail; readback dump at artifacts/H8-srgb-roundtrip.json.
  Pass criterion: roundtrip pixel values within ε.
falsification_action: |
  If H8 falsified, GPU pipeline is sound. If H8 confirmed, format
  mismatch.
cluster_membership: []
```

- [ ] **Step 3: Write H9.yaml**

```yaml
id: H9
short_name: "SVP-boundary IsOpaque correctness"
hypothesis_statement: |
  emView.rs:4760-4772 conditionally clears at the SVP boundary if
  tree.IsOpaque(svp_id) returns false. If a directory panel is the
  SVP and its IsOpaque returns true incorrectly, the SVP-boundary
  clear is skipped and pre-fill shows. Compare against C++
  emFilePanel::IsOpaque (cpp:187-198).
falsification_criterion: |
  For the symptomatic panel, query tree.IsOpaque(panel_id). If the
  return value matches what C++ returns for the same VFS state
  (read from emFilePanel.cpp), H9 is falsified.
experimental_design: |
  Add eprintln at emView.rs:4760 emitting the panel id + IsOpaque
  return value at every SVP-boundary check. Capture for the
  symptomatic frame.
evidence_shape: |
  stderr capture at artifacts/H9-isopaque-trace.txt + reference
  values from emFilePanel.cpp inline in the entry. Pass criterion:
  Rust IsOpaque matches C++ for VFS_LOAD_ERROR / VFS_SAVE_ERROR /
  VFS_CUSTOM_ERROR (per cpp:187-198).
falsification_action: |
  If H9 falsified, IsOpaque correct. If H9 confirmed, IsOpaque mirror
  is wrong.
cluster_membership: []
```

- [ ] **Step 4: Write H10.yaml**

```yaml
id: H10
short_name: "canvas_color snapshot/op-arg disagreement"
hypothesis_statement: |
  emPainter::PaintRect (line 980-989) mutates state.canvas_color *after*
  try_record. The snapshot captured at record time has the pre-mutation
  canvas_color while the op carries the post-mutation arg. Replay may
  apply snapshot then call painter.PaintRect with the carried arg —
  but if any subsequent op consults the (now-stale) canvas_color
  snapshot, output diverges.
falsification_criterion: |
  Audit emPainter.rs for any code path where state.canvas_color is
  mutated AFTER try_record AND any subsequent paint op consults the
  snapshot's canvas_color. If no such path exists, H10 is falsified.
experimental_design: |
  Static analysis of emPainter.rs. Search for try_record sites; check
  if state.canvas_color mutation appears after try_record but before
  the next try_record/require_direct.
evidence_shape: |
  Audit report at artifacts/H10-canvas-color-audit.md. Pass criterion:
  no problematic ordering found.
falsification_action: |
  If H10 falsified, audit clean. If H10 confirmed, canvas_color
  snapshot is stale at consumption time.
cluster_membership: []
```

- [ ] **Step 5: Write P7.yaml**

```yaml
id: P7
short_name: "Multi-target paint composition (intra-frame scratch target)"
hypothesis_statement: |
  Some paint goes to a scratch offscreen target (for clipping or
  shadow effects) that is then composited under a different blend mode
  than borders use. X+Z would composite wrong while Y (which goes
  direct) is unaffected.
falsification_criterion: |
  Enumerate all wgpu encoder/render-pass instantiations during a
  frame. If only one render-pass writes the final framebuffer (no
  intermediate offscreen target), P7 is falsified.
experimental_design: |
  Add instrumentation at every wgpu Encoder::begin_render_pass site
  emitting target/blend mode. Reproduce; capture.
evidence_shape: |
  Log at artifacts/P7-render-passes.log. Pass criterion: one
  render-pass per frame.
falsification_action: |
  If P7 falsified, single-pass paint. If P7 confirmed, multi-target
  composition is the bug surface.
cluster_membership: []
```

- [ ] **Step 6: Commit**

```bash
git add docs/debug/investigations/F010-investigation/hypotheses/{H7,H8,H9,H10,P7}.yaml
git commit -m "$(cat <<'EOF'
docs(F010): draft Tier-3 standalone pre-registrations

H7 (Send-Sync soundness audit), H8 (sRGB roundtrip), H9 (IsOpaque
correctness vs C++), H10 (canvas_color snapshot/arg audit), P7
(multi-render-pass enumeration). All standalone.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.9: Draft remaining blind spots (B4, B6, B7)

**Files:**
- Create: `docs/debug/investigations/F010-investigation/hypotheses/{B4,B6,B7}.yaml`

- [ ] **Step 1: Write B4.yaml**

```yaml
id: B4
short_name: "Stale tile from prior frame surviving cache invalidation"
hypothesis_statement: |
  The visible black is a tile painted during a prior frame (when state
  was VFS_WAITING) that was cached and is being re-presented even after
  state advanced. Cache invalidation criterion misses the
  WAITING→LOADED transition.
falsification_criterion: |
  Force tile-cache full invalidation each frame (e.g. clear cache at
  emWindow.rs render entry). Reproduce. If symptom persists with
  per-frame full-invalidation, B4 is falsified.
experimental_design: |
  Add a feature-gated "flush every frame" branch at the tile-cache
  invalidation site. Build with the flag; reproduce.
evidence_shape: |
  Manual GUI verification + screenshot at
  artifacts/B4-flushed-cache-screenshot.png. Pass criterion: symptom
  gone.
falsification_action: |
  If B4 falsified, cache is correctly invalidated. If B4 confirmed,
  invalidation is missing the relevant transition.
cluster_membership: []
```

- [ ] **Step 2: Write B6.yaml**

```yaml
id: B6
short_name: "Build-config / GPU-vendor / DPR / environment-only repros"
hypothesis_statement: |
  The symptom is environment-conditional. Specifically: only on certain
  GPU vendors, only at certain DPRs, only on Wayland fractional-scale,
  only with certain locale settings. Reduces to a runtime environment
  variable that the test harness doesn't reproduce.
falsification_criterion: |
  Reproduce on at least two distinct hardware/driver/DPR combinations.
  If the symptom appears on every reasonable combination, B6 is
  falsified — environment is not the discriminator.
experimental_design: |
  User-driven: reproduce on the original developer machine + a second
  machine with different GPU vendor/DPR/Wayland config. Document
  outcomes.
evidence_shape: |
  Matrix table at artifacts/B6-env-matrix.md. Pass criterion: every
  tested combination shows symptom-present.
falsification_action: |
  Manual; cannot proceed without user-driven multi-machine testing.
  If B6 confirmed (symptom is env-conditional), the diverging env
  identifies the bug surface.
cluster_membership: []
```

- [ ] **Step 3: Write B7.yaml**

```yaml
id: B7
short_name: "Recursive paint invocation safety"
hypothesis_statement: |
  A panel's Paint method can recursively trigger sub-panel Paint. If
  the recording-painter's internal state (PaintTarget, op vec, depth)
  is not preserved across recursive entries, sub-panel paints land on
  the wrong target or get appended to a parent's op list with bad
  state.
falsification_criterion: |
  At every panel Paint method entry, log the painter's PaintTarget
  identity (memory address) and op vec length. If every recursive
  entry observes the SAME PaintTarget and the op vec length grows
  monotonically without resets, B7 is falsified.
experimental_design: |
  Add eprintln at emView.rs:4796 (paint_one_panel entry) emitting the
  painter target identity and op vec length. Reproduce.
evidence_shape: |
  stderr capture at artifacts/B7-recursive-paint.txt. Pass criterion:
  same target throughout, monotonic op-vec growth.
falsification_action: |
  If B7 falsified, recursion is sound. If B7 confirmed, target switch
  or vec reset is the bug.
cluster_membership: []
```

- [ ] **Step 4: Commit**

```bash
git add docs/debug/investigations/F010-investigation/hypotheses/{B4,B6,B7}.yaml
git commit -m "$(cat <<'EOF'
docs(F010): draft remaining blind-spot pre-registrations (B4, B6, B7)

B4 (stale tile cache), B6 (env-only repros — needs user-driven multi-
machine), B7 (recursive paint safety).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.10: Cross-falsification audit + cluster ordering decide entry

**Files:**
- Modify: nothing yet
- Create: `docs/debug/investigations/F010-investigation/log/0002-cross-falsification-audit.md`
- Create: `docs/debug/investigations/F010-investigation/log/0003-cluster-ordering.md`

- [ ] **Step 1: Audit cross-falsification**

For each cluster, manually walk every pair of falsification criteria within the cluster and verify that observing one cluster mate's criterion does NOT incidentally satisfy another mate's criterion. Specifically:

- `same-observable-with-H1` (H1, P8): H1 inspects ops vec on Clear; P8 inspects rect dimensions. Distinct artifacts. ✓
- `dispatch-cluster` (P3, B2, B3): P3 inspects emDirPanel::Paint trait-method invocation; B2 inspects VFS state at that invocation; B3 inspects upstream paint_one_panel dispatch. Distinct sites. ✓
- `invalidation-cluster` (P2, B1, B8): P2 inspects post-transition recompute firing; B1 inspects single-shot theme-parse value; B8 inspects compositor dirty-tile flag. Distinct artifacts. ✓
- `order-config-cluster` (P4, B5, H11, P5): P4 inspects symptom under pinned ordering; B5 inspects symptom under pre-populated font cache; H11 inspects symptom under assertions-on release; P5 inspects symptom under cfg matrix. Each is a different intervention; same-vs-different observable disambiguates. ✓

- [ ] **Step 2: Write audit log entry**

Create `docs/debug/investigations/F010-investigation/log/0002-cross-falsification-audit.md`:

```markdown
---
id: 0002
type: decide
timestamp: 2026-04-26T11:00:00Z
hypothesis_ids: [H1, P8, P3, B2, B3, P2, B1, B8, P4, B5, H11, P5]
supersedes: null
artifacts: []
---

# Cross-falsification audit

For each cluster, every pair of falsification criteria was checked. No pair shares an artifact such that observing one criterion incidentally satisfies another. Specifically:

- `same-observable-with-H1`: H1 = ops-vec inspection; P8 = rect-dimension log. Distinct.
- `dispatch-cluster`: P3 = trait-method invocation; B2 = VFS state; B3 = upstream dispatch. Distinct sites.
- `invalidation-cluster`: P2 = post-transition recompute log; B1 = init-time theme value; B8 = compositor dirty-flag. Distinct.
- `order-config-cluster`: P4 = pinned-ordering symptom check; B5 = pre-populated cache; H11 = assertions-on release; P5 = cfg matrix. Each intervention is independent.

Cross-falsification rule (spec Section 3, Section 5) is satisfied at this point. Any future revision to a falsification_criterion must rerun this audit.
```

- [ ] **Step 3: Write cluster ordering decide entry**

Create `docs/debug/investigations/F010-investigation/log/0003-cluster-ordering.md`:

```markdown
---
id: 0003
type: decide
timestamp: 2026-04-26T11:30:00Z
hypothesis_ids: []
supersedes: null
artifacts: []
---

# Cluster ordering for phase 3 execution

Cheapest expected experiment first per spec Section 5:

1. `same-observable-with-H1` (H1, P8) — pure unit tests, no GUI rebuild required.
2. H2 (singleton, tile pre-fill) — small instrumentation + unit test.
3. `dispatch-cluster` (P3, B2, B3) — single instrumentation pass + GUI rebuild + reproduce.
4. `invalidation-cluster` (P2, B1, B8) — multiple instrumentation sites + GUI rebuild.
5. Tier-2 standalones (H3, H4, H5, H6, P1) — varied costs but unit-testable.
6. Tier-3 standalones (H7, H8, H9, H10, P7) — mostly static analysis or single tests.
7. Remaining blind spots (B4, B6, B7) — multi-machine for B6, otherwise modest.
8. `order-config-cluster` (P4, B5, H11, P5) — multi-build-config; heaviest. Last.

Order is fixed at end of phase 1 (this entry locks it). Deviations during phase 3 require new `decide` entries.
```

- [ ] **Step 4: Commit**

```bash
git add docs/debug/investigations/F010-investigation/log/0002-cross-falsification-audit.md \
        docs/debug/investigations/F010-investigation/log/0003-cluster-ordering.md
git commit -m "$(cat <<'EOF'
docs(F010): cross-falsification audit and cluster ordering

All 26 pre-registration entries pass cross-falsification audit per spec
Section 3 (no two falsification criteria share an artifact such that
observing one incidentally satisfies another). Cluster ordering for
phase 3 fixed: same-observable-with-H1 first, order-config-cluster
last.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.11: Phase 1 lock and forbidden-fix-shapes checklist

**Files:**
- Create: `docs/debug/investigations/F010-investigation/forbidden-fix-shapes.md`
- Create: `docs/debug/investigations/F010-investigation/log/0004-phase1-lock.md`

- [ ] **Step 1: Write forbidden-fix-shapes checklist**

Create `docs/debug/investigations/F010-investigation/forbidden-fix-shapes.md`:

```markdown
# F010 — forbidden fix-shapes (handoff to fix-spec phase)

Per spec Section 8 / M1. The fix-spec phase MUST run this checklist on its proposed fix; any "yes" answer means the fix is an avoidance fix and is forbidden.

## Four-question avoidance test

For the proposed fix:

1. **Does it introduce a feature flag, environment variable, or build-config gate around the broken code path?** (Yes = avoidance.)

2. **Does it change dispatch (which path is taken at runtime) without changing the path itself?** (Yes = avoidance.)

3. **Does it add a workaround at a higher layer that prevents calls from reaching the broken layer?** (Yes = avoidance.)

4. **Does it deprecate or disable the broken path without removing it?** (Yes = avoidance.)

## Concrete F010-specific avoidance shapes (forbidden)

- Forcing `render_pool.GetThreadCount() = 1` at startup to bypass the display-list branch.
- Setting `dirty_count` ceiling to force the per-tile direct branch.
- Disabling `emPainter::new_recording` and routing all painters through `emPainter::new`.
- Adding a `cfg(feature = "f010_workaround")` arm that skips the broken site.
- Replacing `painter.Clear(color)` calls in panels with `painter.PaintRect` to dodge the recording-mode hole — without fixing the recording-mode hole.

## Permitted fix-shapes

- Adding a `DrawOp::Clear { color }` variant + record path + replay handler at the painter layer (fixes the broken path itself).
- Migrating panels from `painter.Clear(color)` (single-arg) to `painter.ClearWithCanvas(color, canvas_color)` (two-arg) **only if** ClearWithCanvas's record-path semantics match C++'s two-arg Clear contract — that's a structural change that adds canvas-color discipline rather than dodging.
- Fixing the panel state-machine if a state-machine bug is the converged cause.
- Fixing the theme parser if theme parse is the converged cause.

The fix-spec must explicitly answer the four-question test in its own document and demonstrate which permitted shape applies.
```

- [ ] **Step 2: Write phase-1 lock log entry**

Create `docs/debug/investigations/F010-investigation/log/0004-phase1-lock.md`:

```markdown
---
id: 0004
type: decide
timestamp: 2026-04-26T12:00:00Z
hypothesis_ids: []
supersedes: null
artifacts: [docs/debug/investigations/F010-investigation/forbidden-fix-shapes.md]
---

# Phase 1 lock

Pre-registration table is locked. 26 entries in
`docs/debug/investigations/F010-investigation/hypotheses/`:

- 18 hypotheses: H1-H11, P1-P5, P7, P8 (P6 was extracted to methodology constraint M1 per synthesis-v2.md)
- 8 blind spots: B1-B8

Cross-falsification audit (entry 0002) passed. Cluster ordering (entry 0003) fixed.

Forbidden-fix-shapes handoff document committed.

Phase 2 may begin. Per spec Section 4, harness construction is per-cluster, ordered cheapest-first. Pre-registration entries may be revised in phase 2 only via new `revise` entries with `supersedes:` references; existing entries are immutable.
```

- [ ] **Step 3: Commit**

```bash
git add docs/debug/investigations/F010-investigation/forbidden-fix-shapes.md \
        docs/debug/investigations/F010-investigation/log/0004-phase1-lock.md
git commit -m "$(cat <<'EOF'
docs(F010): phase 1 lock — pre-registration table immutable

26 pre-registration entries (18 hypotheses + 8 blind spots) drafted,
cross-falsification audited, cluster-ordered for phase 3. Forbidden-
fix-shapes checklist (M1 operationalized as four-question avoidance
test + concrete forbidden shapes) committed for fix-spec handoff.

Phase 2 may begin: per-cluster harness construction in cheapest-first
order.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 2 — Harness construction (per-cluster, ordered)

### Task 2.1: Build harness for cluster `same-observable-with-H1` (H1 + P8)

**Files:**
- Create: `crates/emcore/tests/f010_h1_clear_recording.rs`
- Create: `crates/emcore/tests/f010_p8_zero_area.rs`

- [ ] **Step 1: Write H1 falsification test**

Create `crates/emcore/tests/f010_h1_clear_recording.rs`:

```rust
//! F010 H1 falsification: emPainter::Clear silently dropped in recording mode.
//!
//! Per `docs/debug/investigations/F010-investigation/hypotheses/H1.yaml`.
//!
//! Falsification criterion: if recording painter's ops vec contains any op
//! contributed by a Clear call, H1 is falsified.

use emcore::emColor::emColor;
use emcore::emPainter::emPainter;

#[test]
fn f010_h1_clear_records_no_op() {
    let mut ops = Vec::new();
    let mut painter = emPainter::new_recording(800, 600, &mut ops);

    // Capture op-vec length before Clear; the spec's falsification rule says any
    // op contributed by the Clear is enough to falsify.
    let len_before = painter.draw_list_ops_len();
    painter.Clear(emColor::rgba(255, 0, 0, 255));
    drop(painter);
    let len_after = ops.len();

    // Hypothesis predicts: ops.len() unchanged (Clear silently dropped).
    // Falsification criterion: ops.len() > len_before.
    let observation_artifact = serde_json::json!({
        "test": "f010_h1_clear_records_no_op",
        "len_before": len_before,
        "len_after": len_after,
        "ops_added_by_clear": len_after - len_before,
    });
    let path = "docs/debug/investigations/F010-investigation/artifacts/H1-ops.json";
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    std::fs::write(path, observation_artifact.to_string()).unwrap();

    // Test PASSES under the hypothesis (ops_added_by_clear == 0). Test FAILS
    // (and H1 is falsified) if any op is added.
    assert_eq!(
        len_after - len_before,
        0,
        "H1 hypothesis predicts Clear records nothing; observed {} ops added",
        len_after - len_before
    );
}
```

- [ ] **Step 2: Verify the test compiles and supports the experiment**

Run: `cargo check -p emcore --tests`

If compilation fails because `draw_list_ops_len` does not exist on `emPainter`, that's a harness-construction issue: the test needs a way to inspect the recording painter's ops vec. **Do not** modify production code to add the method; instead, change the test to inspect the externally-passed `&mut Vec<RecordedOp>` directly. Replace `painter.draw_list_ops_len()` with `ops.len()` (which requires careful borrow management — see fixed version below).

If `emPainter::new_recording`'s signature does not accept `&mut Vec<RecordedOp>`, look at `crates/eaglemode/tests/golden/draw_op_dump.rs` for the actual recording-painter API and adapt.

**Fixed version (if API differs):** read existing recording-painter usage in `tests/golden/draw_op_dump.rs:install_direct_op_logger` and the `crates/eaglemode/tests/golden/painter.rs:setup_op_log` helper. Mirror their pattern.

- [ ] **Step 3: Run the test (it should PASS if H1 holds)**

Run: `cargo nextest run -p emcore --test f010_h1_clear_recording`

Expected: PASS (Clear records nothing → `ops_added_by_clear == 0` → assertion holds).

If the test FAILS, that's the H1 falsification observation — record it as `observe` log entry in phase 3 (not now). For now, the test must compile and run; pass-or-fail outcome is interpretation, not harness completeness.

- [ ] **Step 4: Write P8 falsification test**

Create `crates/emcore/tests/f010_p8_zero_area.rs`:

```rust
//! F010 P8 falsification: coordinate-rounding to zero-area degenerate rects.
//!
//! Per `docs/debug/investigations/F010-investigation/hypotheses/P8.yaml`.
//!
//! Falsification criterion: if the i32 rect emDirPanel::Paint computes for
//! its Clear call has w > 0 AND h > 0, P8 is falsified.

use emcore::emPainter::emPainter;
use emcore::emImage::emImage;

#[test]
fn f010_p8_clear_rect_non_degenerate() {
    // The symptomatic zoom is approximated by a 800x600 viewport with a panel
    // covering a typical sub-region. emDirPanel::Paint's Clear targets the
    // painter's current state.clip rectangle (per emPainter.rs:5779-5782),
    // which mirrors the panel's clip after layout.
    let mut img = emImage::new(800, 600, 4);
    let mut painter = emPainter::new(&mut img);

    // Set a clip representing where emDirPanel's interior would be at the
    // symptomatic zoom. Pull realistic values from the existing real_stack
    // tests in crates/emfileman/src/emDirPanel.rs (see
    // `real_stack_dir_panel_children_created_with_nonzero_rects_after_load`,
    // which uses theme-derived rect sizes).
    painter.SetClipping(50.0, 40.0, 700.0, 500.0);

    // Mirror the rounding emPainter::Clear uses (lines 5779-5782):
    let clip = painter.state_clip();
    let x = clip.x1 as i32;
    let y = clip.y1 as i32;
    let w = clip.x2.ceil() as i32 - x;
    let h = clip.y2.ceil() as i32 - y;

    let observation_artifact = serde_json::json!({
        "test": "f010_p8_clear_rect_non_degenerate",
        "clip": {"x1": clip.x1, "y1": clip.y1, "x2": clip.x2, "y2": clip.y2},
        "rect": {"x": x, "y": y, "w": w, "h": h},
    });
    let path = "docs/debug/investigations/F010-investigation/artifacts/P8-rect.json";
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    std::fs::write(path, observation_artifact.to_string()).unwrap();

    // P8 hypothesis predicts: w == 0 OR h == 0 at symptomatic zoom.
    // Falsification: w > 0 AND h > 0 (rect is non-degenerate).
    let non_degenerate = w > 0 && h > 0;
    assert!(
        non_degenerate,
        "P8 hypothesis predicts degenerate rect at symptomatic zoom; observed w={}, h={}",
        w, h
    );
}
```

- [ ] **Step 5: Verify P8 test compiles and runs**

Run: `cargo check -p emcore --tests`

If `state_clip()` is not a public method on `emPainter`, replace with the appropriate accessor. Look at `tests/golden/painter.rs` for example accessors used in golden tests.

If `painter.SetClipping(50.0, 40.0, 700.0, 500.0)` signature differs, consult `emPainter.rs:755-783` for the actual API (the spec note at architectural-grounding Layer 10 cites that line range).

Run: `cargo nextest run -p emcore --test f010_p8_zero_area`

Expected: PASS if P8 is falsified (rect non-degenerate at symptomatic zoom). If FAIL, that's the P8 confirmation observation — record in phase 3.

- [ ] **Step 6: Lock cluster harness; commit**

Create `docs/debug/investigations/F010-investigation/log/0005-cluster1-harness-lock.md`:

```markdown
---
id: 0005
type: decide
timestamp: 2026-04-26T13:00:00Z
hypothesis_ids: [H1, P8]
supersedes: null
artifacts: [crates/emcore/tests/f010_h1_clear_recording.rs, crates/emcore/tests/f010_p8_zero_area.rs]
---

# Cluster `same-observable-with-H1` harness lock

Two tests created. Both compile and run. H1's test inspects the recording
painter's ops vec after Clear; P8's test inspects the i32 rect computed
from the painter's current clip at the symptomatic zoom.

Phase 3 cluster execution may begin.
```

```bash
git add crates/emcore/tests/f010_h1_clear_recording.rs \
        crates/emcore/tests/f010_p8_zero_area.rs \
        docs/debug/investigations/F010-investigation/log/0005-cluster1-harness-lock.md
git commit -m "$(cat <<'EOF'
test(F010): harness for same-observable-with-H1 cluster (H1, P8)

Two unit tests in crates/emcore/tests/. H1 inspects recording-mode ops
vec after Clear. P8 inspects i32 rect for degeneracy at symptomatic
zoom. Both produce JSON artifacts under
docs/debug/investigations/F010-investigation/artifacts/.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 3 — Cluster-first execution (continues per cluster)

### Task 3.1: Execute cluster `same-observable-with-H1`

- [ ] **Step 1: Pick representative**

H1 has the cheapest experiment (a unit test inspecting an internal Vec, no rebuild needed). H1 is the representative.

- [ ] **Step 2: Run H1 falsification**

```bash
cargo nextest run -p emcore --test f010_h1_clear_recording
```

Two outcomes:
- **PASS** → ops_added_by_clear == 0 → H1 hypothesis upheld → H1 is *not* falsified by this experiment, i.e. **confirmed for the cluster purpose**. Read `docs/debug/investigations/F010-investigation/artifacts/H1-ops.json` and capture in observe entry.
- **FAIL** → ops_added_by_clear > 0 → H1 *falsified*. Record observe entry; mark H1 falsify; advance to P8.

- [ ] **Step 3: Write H1 observe entry (templated for each outcome)**

Outcome-conditional template; pick whichever fits. Create `docs/debug/investigations/F010-investigation/log/0006-h1-experiment.md`:

```markdown
---
id: 0006
type: observe
timestamp: <FILL: timestamp from `date -u +%Y-%m-%dT%H:%M:%SZ`>
hypothesis_ids: [H1]
supersedes: null
artifacts: [docs/debug/investigations/F010-investigation/artifacts/H1-ops.json]
---

# H1 falsification experiment result

Test: `cargo nextest run -p emcore --test f010_h1_clear_recording`

Outcome: <PASS / FAIL>

Evidence (from artifacts/H1-ops.json):
- len_before: <value>
- len_after: <value>
- ops_added_by_clear: <value>

Interpretation:
- If ops_added_by_clear == 0 → H1 hypothesis upheld (Clear records nothing).
- If ops_added_by_clear > 0 → H1 falsified (Clear records ops despite the require_direct path).
```

- [ ] **Step 4: Run P8 falsification**

```bash
cargo nextest run -p emcore --test f010_p8_zero_area
```

- [ ] **Step 5: Write P8 observe entry**

Create `docs/debug/investigations/F010-investigation/log/0007-p8-experiment.md` mirroring the H1 entry's structure with P8's artifact.

- [ ] **Step 6: Cluster resolution**

Four possible joint outcomes:

| H1 result | P8 result | Cluster resolution |
|---|---|---|
| H1 confirmed (test PASS, ops==0) | P8 falsified (test PASS, rect non-degenerate) | **H1 confirmed; cluster resolved.** Write `confirm` entry citing H1's observe; cite P8's `falsify` observe. Advance to next cluster. |
| H1 falsified (test FAIL) | P8 confirmed (test FAIL) | Both confirmed → cluster cannot be discriminated. Write `escalate` entry. Investigation suspends. |
| H1 falsified | P8 falsified | Both fail → no surviving cluster member. Write `falsify` entries for both. Advance to next cluster (the cluster's hypothesis space is exhausted but the global investigation continues). |
| H1 confirmed | P8 confirmed | H1 confirmed; P8 also confirmed but distinguished by mechanism. **Cluster cannot be discriminated** even if both could be the cause. Write `escalate` entry for clarification, then either accept H1 as cause or design a new discrimination experiment. |

Write the appropriate entry as `0008-cluster1-resolution.md`:

```markdown
---
id: 0008
type: <confirm | falsify | escalate>
timestamp: <FILL>
hypothesis_ids: [H1, P8]
supersedes: null
artifacts: [log/0006-h1-experiment.md, log/0007-p8-experiment.md]
---

# Cluster `same-observable-with-H1` resolution

Outcome: <confirm | suspend>

H1: <confirmed | falsified> per entry 0006.
P8: <confirmed | falsified> per entry 0007.

<rationale per the four-row table in plan Task 3.1 step 6>
```

- [ ] **Step 7: Commit**

```bash
git add docs/debug/investigations/F010-investigation/log/000{6,7,8}-*.md \
        docs/debug/investigations/F010-investigation/artifacts/{H1-ops,P8-rect}.json
git commit -m "$(cat <<'EOF'
docs(F010): cluster same-observable-with-H1 resolution

Ran H1 and P8 falsification experiments. Outcomes recorded in append-
only log entries 0006 (H1), 0007 (P8), 0008 (cluster resolution).
Artifacts at docs/debug/investigations/F010-investigation/artifacts/.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 8: Advance to next cluster regardless of cluster 1 outcome**

Even if cluster 1 resolves with H1 confirmed and P8 falsified, the methodology proceeds to Task 3.2 and runs every remaining cluster. **Defense-in-depth is the default.** Rationale:

- A "confirmed" hypothesis from a single cluster is one piece of evidence, not the full picture. F010 has 18 hypotheses across 5 clusters precisely because the symptom is observationally consistent with multiple mechanisms.
- The F018 acceptance miss (where a non-user-visible signal was accepted as confirmation, masking the user-visible failure) is the cautionary tale baked into spec Section 7. The same risk applies to single-cluster confirmation: one cluster's evidence may be "correct in isolation" while a second cluster's evidence reveals an additional or alternative mechanism.
- Running remaining clusters costs hours; accepting a false-positive root cause wastes a full fix-spec → fix-plan → fix-implementation cycle plus the user's manual-verification time. The conservative trade favors completeness.
- Cluster 1's outcome is recorded in the log; it does NOT need to be re-run later. Subsequent clusters either falsify (strengthening H1 by elimination) or also confirm (revealing a multi-cause situation that requires explicit handling in fix-spec).

If cluster 1 resolved with H1 confirmed: continue to Task 3.2 expecting the remaining clusters to falsify. Any cluster that ALSO confirms triggers an `escalate` entry — multi-cause situations are out of the methodology's confirm-one-hypothesis assumption and must surface to the user before fix-spec.

If cluster 1 did NOT resolve (cluster suspended via escalate, or both H1 and P8 falsified leaving the cluster empty): also continue to Task 3.2. The investigation's hypothesis space extends across all clusters; cluster 1's outcome does not gate cluster 2's execution.

---

### Task 3.2: Execute cluster `H2 singleton`

If cluster 1 resolved via fast path, **skip this task**. Otherwise:

- [ ] **Step 1: Build H2 harness component**

Modify `crates/emcore/src/emWindow.rs` near line 620 (the per-frame `view.background_color` capture site identified in spec Section 2 / synthesis-v2 Layer 2): add `eprintln!("F010-H2 view.background_color={:08x}", color.AsU32());` immediately after the value is read. Build the GUI binary (`cargo build -p eaglemode`).

- [ ] **Step 2: Reproduce and capture**

Run the GUI:

```bash
cargo run -p eaglemode 2>&1 | tee docs/debug/investigations/F010-investigation/artifacts/H2-bgcolor.txt
```

Reproduce the symptom (zoom into Card-Blue dir). Grep stderr for `F010-H2`.

- [ ] **Step 3: Write H2 observe entry**

`log/00NN-h2-experiment.md` (use next available log id) with the captured value and pass/fail per the falsification criterion.

- [ ] **Step 4: Cluster resolution + commit**

Single-hypothesis cluster, so resolution = falsification or confirmation directly. Write `confirm` or `falsify` entry. Revert the eprintln modification (do NOT leave instrumentation in production source — but DO commit the rebuild artifact and remove instrumentation in a separate commit, so the experiment is reproducible from git).

Commit revert:

```bash
git add crates/emcore/src/emWindow.rs
git commit -m "$(cat <<'EOF'
revert(F010): remove H2 instrumentation eprintln

H2 experiment ran (entry 00NN). Instrumentation removed from production source. Result captured in artifacts/H2-bgcolor.txt and observe log entry.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3.3: Execute cluster `dispatch-cluster` (P3, B2, B3)

Same pattern as Task 3.2 but with three eprintln sites (per the YAMLs). Build once with all three instrumentations; reproduce; collect stderr; demultiplex per-hypothesis criteria into three observe entries; one cluster-resolution entry.

- [ ] **Step 1: Add three eprintln instrumentations**

Modify:
- `emfileman/src/emDirPanel.rs` Paint trait method entry: `eprintln!("F010-P3 emDirPanel::Paint panel_id={:?}", state.panel_id);`
- `emfileman/src/emDirPanel.rs` line 454 (before the match): `eprintln!("F010-B2 vfs_state={:?} panel_id={:?}", self.file_panel.GetVirFileState(), state.panel_id);`
- `emcore/src/emView.rs` paint_one_panel entry (around line 4796): `eprintln!("F010-B3 paint_one_panel panel_id={:?}", panel_id);`

- [ ] **Step 2: Build, reproduce, capture**

```bash
cargo build -p eaglemode
cargo run -p eaglemode 2>&1 | tee docs/debug/investigations/F010-investigation/artifacts/dispatch-cluster.txt
```

Reproduce; grep `F010-P3`, `F010-B2`, `F010-B3` separately.

- [ ] **Step 3: Write three observe entries**

One per hypothesis. Each entry quotes the relevant grepped lines.

- [ ] **Step 4: Write cluster resolution entry + commit + revert instrumentations**

Same shape as Task 3.2 step 4.

---

### Task 3.4: Execute cluster `invalidation-cluster` (P2, B1, B8)

Same pattern as Task 3.3, with these instrumentation sites:

- P2: theme-reload site in `crates/emfileman/src/emFileManViewConfig.rs` + VFS transitions in `crates/emfileman/src/emDirModel.rs`
- B1: a unit test (no instrumentation needed) reading `theme.DirContentColor` after `emFileManViewConfig::Acquire`
- B8: dirty-tile flag site in `emViewRendererCompositor.rs` / `emViewRendererTileCache.rs`

B1 is a unit test, separable from the other two. Run it first (cheap), then if not falsified, build instrumented GUI and reproduce for P2 + B8.

Steps mirror Task 3.3 with these specifics. Three observe entries, one resolution entry, revert instrumentation, commit.

---

### Task 3.5: Execute Tier-2 standalones (H3, H4, H5, H6, P1)

Each is independent. Run cheapest first:

1. **H3** (render-strategy split): set `RAYON_NUM_THREADS=1` (or analogous) and rebuild GUI; reproduce. If symptom gone, H3 confirmed.
2. **H4** (font-atlas at replay): unit test in `crates/emcore/tests/f010_h4_text_replay.rs` per H4.yaml's experimental_design.
3. **H5** (composite re-blend): unit test in `crates/emcore/tests/f010_h5_composite.rs` per H5.yaml.
4. **H6** (state-snapshot equivalence): unit test in `crates/emcore/tests/f010_h6_state_equivalence.rs` per H6.yaml.
5. **P1** (resource lifecycle): instrumented GUI per P1.yaml.

Each: build harness, run, write observe entry, write singleton-cluster resolution. Commit per hypothesis.

---

### Task 3.6: Execute Tier-3 standalones (H7, H8, H9, H10, P7)

Same pattern:

1. **H7** (Send-Sync soundness): static analysis report at `artifacts/H7-mutation-audit.md`.
2. **H8** (sRGB roundtrip): unit test against wgpu pipeline.
3. **H9** (IsOpaque correctness): instrumented GUI + cross-reference C++.
4. **H10** (canvas_color audit): static analysis report.
5. **P7** (multi-render-pass): instrumented GUI + render-pass count log.

---

### Task 3.7: Execute remaining blind spots (B4, B6, B7)

1. **B4** (stale tile cache): feature-gated cache flush + reproduce.
2. **B6** (env-only repros): user-driven multi-machine matrix. **This task requires user action — cannot run autonomously.**
3. **B7** (recursive paint safety): instrumented GUI + recursion log.

For B6, write a `decide` entry naming the user-driven nature and the matrix to fill, then suspend the cluster pending user input. This is an explicit phase boundary.

---

### Task 3.8: Execute cluster `order-config-cluster` (P4, B5, H11, P5)

Heaviest cluster. Run only if all prior clusters exhausted without confirmation.

For each hypothesis: build under the relevant config/env condition; reproduce; capture; observe entry. Cluster resolution entry combines four results. Commit.

---

## Phase 4 — Termination gate and handoff

### Task 4.1: Mechanical channel verification

If a hypothesis is confirmed, the mechanical channel test should already exist (it's the test added in phase 2). Verify it still passes against the unmodified production source (i.e., no instrumentations remain).

- [ ] **Step 1: Confirm clean working tree**

```bash
git status
```

Expected: clean (all instrumentation reverts committed).

- [ ] **Step 2: Run full mechanical test suite**

```bash
cargo nextest ntr
```

Expected: green. The hypothesis-confirming test passes against unmodified source.

- [ ] **Step 3: Add the confirming test to CI if not already**

Verify CI (`.github/workflows/` or equivalent) includes `cargo nextest ntr` so the new tests run on every push. If they do not, add them.

### Task 4.2: Manual channel — user GUI verification

Manual user interaction required.

- [ ] **Step 1: Hand off to user**

Write a manual-channel-prep observe entry summarizing what the user should do:

```markdown
---
id: 00NN
type: decide
timestamp: <FILL>
hypothesis_ids: [<confirmed hypothesis>]
supersedes: null
artifacts: []
---

# Termination gate — manual channel handoff

Mechanical channel: green (cargo nextest ntr passes).

Manual channel pending: user must launch eaglemode-rs, navigate to the
symptom-reproducing scenario (Card-Blue, zoom into a directory), and
confirm whether the symptom (panel interior renders solid black + info
pane invisible) is now gone.

User action: report yes/no/partial. If no/partial, this `decide` entry
is followed by a `revise` entry reverting the cluster resolution and
either expanding to remaining clusters or escalating.
```

- [ ] **Step 2: Wait for user verification result**

- [ ] **Step 3: Write terminating confirm entry**

If user confirms positive:

```markdown
---
id: 00NN
type: confirm
timestamp: <FILL>
hypothesis_ids: [<confirmed hypothesis>]
supersedes: null
artifacts: [...]
---

# Termination — investigation complete

Both channels positive:
- Mechanical: cargo nextest ntr green; <confirming test name> passes.
- Manual: user verified symptom gone in live GUI on <date> in <scenario>.

Investigation methodology terminates with success. Hand off to fix-spec
phase per spec Section 9 mode 1.

Forbidden-fix-shapes checklist at
`docs/debug/investigations/F010-investigation/forbidden-fix-shapes.md`
is the M1-operationalization input to fix-spec.
```

If user confirms negative (symptom persists):

Write `revise` entry that supersedes the cluster resolution; escalate to next cluster or to all-falsified mode per spec Section 9 mode 2 or 3.

### Task 4.3: Handoff to fix-spec phase

- [ ] **Step 1: Update ISSUES.json**

Set F010 status to `needs-fix-spec` with `fix_note` summarizing converged evidence and pointing at:
- `docs/debug/investigations/F010-investigation/log/<terminating entry>.md` (confirmation)
- `docs/debug/investigations/F010-investigation/forbidden-fix-shapes.md` (M1 handoff)
- `docs/debug/investigations/F010-investigation/hypotheses/<confirmed>.yaml` (the registered hypothesis)

- [ ] **Step 2: Commit handoff**

```bash
git add docs/debug/ISSUES.json
git commit -m "$(cat <<'EOF'
docs(F010): investigation methodology terminates — fix-spec handoff

Investigation per docs/superpowers/specs/2026-04-26-F010-investigation-methodology-design.md
terminates with success. Mechanical and manual channels both positive.
Confirmed hypothesis: <ID> (<short name>).

Handoff inputs to fix-spec phase:
- Confirmed hypothesis YAML
- Append-only investigation log
- Forbidden-fix-shapes checklist (M1)

ISSUES.json F010 advanced to needs-fix-spec.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 3: Brainstorm fix-spec**

Invoke `superpowers:brainstorming` with the converged evidence as input. The fix-spec is a *new* spec → plan → implementation cycle, distinct from this methodology cycle. It is out of scope for this plan.

---

## Self-review

(Performed by plan author after writing.)

**Spec coverage check.** Walked spec sections 1-9 against plan tasks:
- §1 scope/goal → Task 0.1 README + plan goal statement.
- §2 hypothesis category checklist → Tasks 1.1-1.9 (all 26 entries).
- §3 pre-registration template → Tasks 1.1-1.9 use the schema; Task 1.11 lock.
- §4 harness rules → Tasks 2.1+ build per-cluster harnesses.
- §5 cluster-first execution → Tasks 3.1-3.8 follow ordering from Task 1.10.
- §6 evidence-recording → log entries throughout, frontmatter per spec, file-per-entry.
- §7 termination gate → Tasks 4.1, 4.2.
- §8 forbidden fix-shapes → Task 1.11 produces `forbidden-fix-shapes.md`; Task 4.3 hands off.
- §9 stop conditions → Task 3.1 step 8 (success/escalate/cluster-exhaust); Task 4.2 (gated termination).

**Placeholder scan.** No "TBD", "TODO", or "fill in details" tokens. The four-row outcome table in Task 3.1 step 6 is intentionally template — outcome is data, not placeholder. Fix-spec brainstorming (Task 4.3 step 3) is properly scoped out.

**Type/name consistency.** YAML schema in spec Section 3 matches Task 1.1+ usage. Hypothesis IDs (H1-H11, P1-P8, B1-B8) consistent with synthesis-v2.md. Log entry frontmatter consistent across tasks. File paths consistent (`docs/debug/investigations/F010-investigation/...` everywhere).

**Open question resolution.** Three deferred questions from spec resolved:
1. Cross-falsification → Task 1.10 audit using distinct-artifacts rule.
2. M1 operationalization → Task 1.11 four-question avoidance test + concrete forbidden shapes.
3. Harness construction order → Phase 2 split per-cluster, ordered cheapest-first per Task 1.10 cluster ordering decide.
