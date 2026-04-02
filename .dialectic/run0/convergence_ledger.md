# Convergence Ledger -- Dialectic Run 0

## 1. Process Overview

Three agents debated the question: **Is the current hypothesis-cascade approach (CLAUDE-APPROACH-CURRENT) or the stage-decomposition pivot (USER-APPROACH-PIVOT) better for converging on 241/241 golden tests at zero tolerance?**

- **Total propositions**: 74 (Agent 1: 25, Agent 2: 25, Agent 3: 24)
- **Tensions identified**: 28 (across 14 clusters)
- **High-severity tensions (>=0.70)**: 18
- **All tensions resolved**: Yes (all adjudicated in Round 3)

### Category Distribution

| Category | Count |
|----------|-------|
| Survivor | 8 |
| Wounded  | 49 |
| Contested| 0 |
| Fallen   | 17 |

## 2. Final Scoreboard

| Rank | ID | Proposition (truncated) | Composite | Category |
|------|----|-------------------------|-----------|----------|
| 1 | a3-05 | The C++ ScanlineTool::Init computes stride reduction (modify... | 0.8875 | survivor |
| 2 | a3-04 | The C++ rendering pipeline uses #include-based macro templat... | 0.8250 | survivor |
| 3 | a2-20 | The Phase 5 golden test setup (fill color, canvas color, sca... | 0.7750 | survivor |
| 4 | a3-14 | The C++ code computes ImgMap as a raw pointer with byte-leve... | 0.7750 | survivor |
| 5 | a3-01 | Previous investigations of Group A+B eliminated hypotheses a... | 0.7625 | survivor |
| 6 | a2-03 | Group A+B (20 tests) failures are caused by 9-slice border i... | 0.7550 | survivor |
| 7 | a1-06 | The Rust paint_9slice_section reimplements C++ ScanlineTool:... | 0.7500 | survivor |
| 8 | a3-11 | The failing tests show divergences concentrated at specific ... | 0.7500 | survivor |
| 9 | a2-05 | G3 (Hermite factor table, 2 tests, max_diff=1) can be fixed ... | 0.7450 | wounded |
| 10 | a1-22 | The previous assessment that paint_9slice_section was 'struc... | 0.7375 | wounded |
| 11 | a2-23 | The Group A root cause was corrected from 'PaintTextBoxed te... | 0.7375 | wounded |
| 12 | a3-17 | Approach B's pipeline decomposition targets only image rende... | 0.7375 | wounded |
| 13 | a2-02 | The 37 remaining golden test failures have been classified i... | 0.7300 | wounded |
| 14 | a2-06 | G5 (blend_hash_lookup, 1 test, max_diff=1) can be fixed by r... | 0.7300 | wounded |
| 15 | a2-16 | The next Group A investigation step is a ~330-line side-by-s... | 0.7050 | wounded |
| 16 | a2-07 | G3 + G5 combined can be fixed in approximately 30 minutes, a... | 0.7000 | wounded |
| 17 | a3-06 | The C++ area sampling loop carries state between pixels with... | 0.7000 | wounded |
| 18 | a3-08 | Building intermediate comparison harnesses (Approach B) risk... | 0.7000 | wounded |
| 19 | a3-18 | C++ unsigned-times-signed integer arithmetic involves implic... | 0.7000 | wounded |
| 20 | a1-04 | A Stage 1 harness that compares the 9 section rectangles com... | 0.6875 | wounded |
| 21 | a3-03 | Three investigation rounds targeting Group A+B produced real... | 0.6875 | wounded |
| 22 | a3-23 | The Rust and C++ pipelines have fundamentally different deco... | 0.6875 | wounded |
| 23 | a2-22 | The remaining 37 failures decompose into three effort tiers:... | 0.6800 | wounded |
| 24 | a2-04 | The C++ ScanlineTool::Init uses clamped source width (ImgW a... | 0.6750 | wounded |
| 25 | a3-24 | After Group A+B is fixed by either approach, 16 tests remain... | 0.6750 | wounded |
| 26 | a1-14 | The Rust pipeline should add stage-level tracing via a compi... | 0.6625 | wounded |
| 27 | a3-22 | The sunk cost of three investigation rounds on Group A+B cre... | 0.6625 | wounded |
| 28 | a1-19 | When a stage-level divergence is found, the correct fix depe... | 0.6500 | wounded |
| 29 | a1-03 | The root cause of Group A+B (21 tests) is 9-slice border ima... | 0.6425 | wounded |
| 30 | a2-24 | The C++ rendering pipeline for 9-slice sections goes through... | 0.6375 | wounded |
| 31 | a3-07 | Groups G2 (polygon, 2 tests), G7 (gradient, 2 tests), G9 (po... | 0.6250 | wounded |
| 32 | a1-05 | A Stage 2 harness comparing fixed-point transform parameters... | 0.6125 | wounded |
| 33 | a1-13 | For each failing golden test, the C++ generator should be mo... | 0.6125 | wounded |
| 34 | a2-19 | G2 (polygon rasterizer floating-point divergence, 6 tests) i... | 0.6000 | wounded |
| 35 | a3-13 | Neither Approach A (continue current hypothesis elimination)... | 0.6000 | wounded |
| 36 | a3-16 | The 204 passing golden tests at zero tolerance are the proje... | 0.6000 | wounded |
| 37 | a2-01 | The classify-hypothesize-investigate-fix methodology has pro... | 0.5875 | wounded |
| 38 | a1-24 | The gradient pipeline divergence in Group G7 can be diagnose... | 0.5750 | wounded |
| 39 | a3-15 | The reward signal from fixing adjacent bugs during Group A+B... | 0.5750 | wounded |
| 40 | a3-19 | The CLAUDE.md boundary between 'pixel arithmetic' (literal C... | 0.5750 | wounded |
| 41 | a1-01 | The inability of end-to-end golden tests to localize diverge... | 0.5625 | wounded |
| 42 | a1-15 | The tdx_init computation fix discovered during the area samp... | 0.5625 | wounded |
| 43 | a1-18 | The polygon rasterization divergence in Group G2 (6 tests) c... | 0.5625 | wounded |
| 44 | a2-10 | The 12 classified failure groups can be attacked independent... | 0.5625 | wounded |
| 45 | a3-10 | Zero-tolerance golden tests are fragile: a refactoring that ... | 0.5625 | wounded |
| 46 | a1-11 | The PaintImageColored investigation (Hypothesis 3) was exami... | 0.5500 | wounded |
| 47 | a3-02 | Each round of investigation that declares a subsystem 'struc... | 0.5500 | wounded |
| 48 | a3-09 | Some of the 37 failing golden tests may have incorrect refer... | 0.5500 | wounded |
| 49 | a3-12 | The 37 failing tests are grouped into 12 groups based on sym... | 0.5500 | wounded |
| 50 | a1-02 | Three rounds of hypothesis-driven investigation (area sampli... | 0.5375 | wounded |
| 51 | a1-08 | Stage-level verification eliminates the classification probl... | 0.5375 | wounded |
| 52 | a1-23 | A differential testing harness in the sense of McKeeman (199... | 0.5375 | wounded |
| 53 | a2-11 | The investigation has verified 7 subsystems as correct (inte... | 0.5375 | wounded |
| 54 | a2-13 | Golden pixel comparison at zero tolerance is the correct and... | 0.5375 | wounded |
| 55 | a1-09 | The Group A+B failure classification was revised three times... | 0.5250 | wounded |
| 56 | a2-18 | The monotonic ratchet property (tolerances only decrease, te... | 0.5000 | wounded |
| 57 | a3-20 | Infrastructure investment in intermediate comparison harness... | 0.5000 | wounded |
| 58 | a1-17 | Golden tests should serve only as composition verification (... | 0.4875 | fallen |
| 59 | a1-10 | 36 of 37 failing golden tests pass through one or more share... | 0.4625 | fallen |
| 60 | a1-21 | Vouk (1990) found that in interlanguage conversion projects,... | 0.4625 | fallen |
| 61 | a1-16 | Investigator hypothesis selection is biased toward stages th... | 0.4500 | fallen |
| 62 | a2-12 | The projected fix trajectory is: G3+G5 (207/241), then Group... | 0.4500 | fallen |
| 63 | a2-14 | The CLAUDE.md port fidelity rules (reproduce C++ integer for... | 0.4500 | fallen |
| 64 | a2-08 | Each of the three disproven Group A hypotheses produced coll... | 0.4450 | fallen |
| 65 | a1-12 | The hypothesis cascade approach has a hidden cost: false con... | 0.4375 | fallen |
| 66 | a3-21 | A backward trace from a pixel divergence through the renderi... | 0.4250 | fallen |
| 67 | a1-20 | The project is at an inflection point where continuing the h... | 0.4000 | fallen |
| 68 | a2-17 | Every prior correctness improvement (area sampling, HowTo te... | 0.4000 | fallen |
| 69 | a1-07 | The total engineering cost of building stage-level compariso... | 0.3625 | fallen |
| 70 | a1-25 | The existing golden test format (.painter.golden, .layout.go... | 0.3500 | fallen |
| 71 | a2-15 | Switching to a different methodology would not transfer the ... | 0.3500 | fallen |
| 72 | a2-09 | Building intermediate comparison harnesses (C++ instrumentat... | 0.3250 | fallen |
| 73 | a2-21 | Inserting printf/logging into C++ instrumentation code can c... | 0.3250 | fallen |
| 74 | a2-25 | The rate of progress constraint is investigator availability... | 0.1500 | fallen |

## 3. Survivors (composite >= 0.75, no axis below 0.50)

### From Agent 1 (Stage-Decomposition Advocate)

### a1-06

**Proposition**: The Rust paint_9slice_section reimplements C++ ScanlineTool::Init logic inline rather than routing through the shared PaintImage -> PaintRect -> Init path, creating an architectural divergence that manifests at sub-pixel boundary conditions.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.80 | 0.00 | 0.80 |
| actionability | 0.85 | 0.00 | 0.85 |
| failure_resistance | 0.60 | -0.05 | 0.55 |
| parity_convergence | 0.80 | 0.00 | 0.80 |

**Composite**: 0.7500

### From Agent 2 (Current-Methodology Defender)

### a2-20

**Proposition**: The Phase 5 golden test setup (fill color, canvas color, scale) may be wrong for some tests, and test setup must be verified against the C++ generator before investigating rendering code.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.90 | 0.00 | 0.90 |
| actionability | 0.85 | 0.00 | 0.85 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.50 | +0.05 | 0.55 |

**Composite**: 0.7750

### a2-03

**Proposition**: Group A+B (20 tests) failures are caused by 9-slice border image section boundary rounding divergence between Rust paint_9slice_section and C++ PaintBorderImage/ScanlineTool::Init.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.75 | -0.03 | 0.72 |
| actionability | 0.85 | 0.00 | 0.85 |
| failure_resistance | 0.55 | 0.00 | 0.55 |
| parity_convergence | 0.90 | 0.00 | 0.90 |

**Composite**: 0.7550

### From Agent 3 (Failure-Mode Analyst)

### a3-05

**Proposition**: The C++ ScanlineTool::Init computes stride reduction (modifying ImgW, ImgDX, ImgSX, ImgMap in-place) before computing transform parameters (tdx, tdy), creating an ordering dependency where the Rust code may compute transforms with pre-reduction values instead of post-reduction values.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | 0.00 | 0.85 |
| actionability | 0.90 | 0.00 | 0.90 |
| failure_resistance | 0.85 | 0.00 | 0.85 |
| parity_convergence | 0.90 | +0.05 | 0.95 |

**Composite**: 0.8875

### a3-04

**Proposition**: The C++ rendering pipeline uses #include-based macro template expansion (CHANNELS, EXTENSION parameters) that produces 12 variants of each interpolation function, and agents reading the generic template systematically misinterpret the specific variant (CHANNELS=4, CLAMP) that executes for failing tests.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | 0.00 | 0.85 |
| actionability | 0.70 | +0.10 | 0.80 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.75 | +0.10 | 0.85 |

**Composite**: 0.8250

### a3-14

**Proposition**: The C++ code computes ImgMap as a raw pointer with byte-level arithmetic (ImgMap=texture.GetImage().GetMap()+(sy*(size_t)iw+sx)*channels) while Rust passes section bounds as SectionBounds { ox, oy, w, h }, and verifying these two representations produce identical sampling behavior requires tracing every downstream consumer.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | 0.00 | 0.85 |
| actionability | 0.70 | +0.05 | 0.75 |
| failure_resistance | 0.75 | 0.00 | 0.75 |
| parity_convergence | 0.75 | 0.00 | 0.75 |

**Composite**: 0.7750

### a3-01

**Proposition**: Previous investigations of Group A+B eliminated hypotheses at the wrong level of abstraction -- comparing inputs to divergent functions rather than their derived internal state -- creating permanent blind spots in the search space.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.80 | 0.00 | 0.80 |
| actionability | 0.60 | +0.10 | 0.70 |
| failure_resistance | 0.75 | 0.00 | 0.75 |
| parity_convergence | 0.70 | +0.10 | 0.80 |

**Composite**: 0.7625

### a3-11

**Proposition**: The failing tests show divergences concentrated at specific pixel coordinates (edge pixels with small channel differences, max_diff=14), which is consistent with a sub-pixel positioning error in 9-slice subdivision rather than an interpolation or blending bug.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | 0.00 | 0.70 |
| actionability | 0.85 | 0.00 | 0.85 |
| failure_resistance | 0.60 | 0.00 | 0.60 |
| parity_convergence | 0.85 | 0.00 | 0.85 |

**Composite**: 0.7500


## 4. Wounded (composite 0.50-0.74, or any axis below 0.50)

### From Agent 1 (Stage-Decomposition Advocate)

### a1-22

**Proposition**: The previous assessment that paint_9slice_section was 'structurally equivalent with minor differences that don't cause divergence in normal cases' was made without per-case numerical comparison and should be re-verified with stage-level harness data.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.80 | 0.00 | 0.80 |
| actionability | 0.75 | 0.00 | 0.75 |
| failure_resistance | 0.70 | 0.00 | 0.70 |
| parity_convergence | 0.70 | 0.00 | 0.70 |

**Composite**: 0.7375

### a1-04

**Proposition**: A Stage 1 harness that compares the 9 section rectangles computed by Rust vs C++ PaintBorderImage for the same inputs would immediately reveal whether section boundary rounding is the root cause of Group A+B, without hypothesis formation or pixel forensics.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.75 | 0.00 | 0.75 |
| actionability | 0.80 | 0.00 | 0.80 |
| failure_resistance | 0.70 | -0.10 | 0.60 |
| parity_convergence | 0.65 | -0.05 | 0.60 |

**Composite**: 0.6875

### a1-14

**Proposition**: The Rust pipeline should add stage-level tracing via a compile-time feature flag (#[cfg(feature = "stage-trace")]) to emit intermediate values at section entry, transform computation, and interpolation output.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | 0.00 | 0.70 |
| actionability | 0.80 | 0.00 | 0.80 |
| failure_resistance | 0.65 | -0.05 | 0.60 |
| parity_convergence | 0.55 | 0.00 | 0.55 |

**Composite**: 0.6625

### a1-19

**Proposition**: When a stage-level divergence is found, the correct fix depends on its category: literal port error (change formula), architectural divergence (route through shared logic), intentional idiom difference (port C++ approach for golden path), C++ bug reproduction (port exact formula), or test setup error (align test parameters).

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.80 | 0.00 | 0.80 |
| actionability | 0.60 | 0.00 | 0.60 |
| failure_resistance | 0.70 | 0.00 | 0.70 |
| parity_convergence | 0.50 | 0.00 | 0.50 |

**Composite**: 0.6500

### a1-03

**Proposition**: The root cause of Group A+B (21 tests) is 9-slice border image section boundary rounding divergence from C++, with divergent pixels clustering at section edges.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.65 | -0.03 | 0.62 |
| actionability | 0.70 | 0.00 | 0.70 |
| failure_resistance | 0.50 | 0.00 | 0.50 |
| parity_convergence | 0.75 | 0.00 | 0.75 |

**Composite**: 0.6425

### a1-05

**Proposition**: A Stage 2 harness comparing fixed-point transform parameters (TX, TY, TDX, TDY, stride_x, stride_y, off_x, off_y) between Rust paint_9slice_section and C++ ScanlineTool::Init would localize transform-level divergences to the exact parameter and section.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | -0.10 | 0.60 |
| actionability | 0.75 | 0.00 | 0.75 |
| failure_resistance | 0.65 | -0.05 | 0.60 |
| parity_convergence | 0.60 | -0.10 | 0.50 |

**Composite**: 0.6125

### a1-13

**Proposition**: For each failing golden test, the C++ generator should be modified to emit intermediate values (section rectangles, transform parameters, interpolated scanline values) alongside pixel data, creating a per-stage oracle.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.65 | 0.00 | 0.65 |
| actionability | 0.70 | 0.00 | 0.70 |
| failure_resistance | 0.55 | -0.05 | 0.50 |
| parity_convergence | 0.60 | 0.00 | 0.60 |

**Composite**: 0.6125

### a1-24

**Proposition**: The gradient pipeline divergence in Group G7 can be diagnosed by a stage harness comparing the 1-byte g values before the hash lookup, which would determine whether the divergence is in the gradient walk or in the hash mediation.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | 0.00 | 0.70 |
| actionability | 0.65 | 0.00 | 0.65 |
| failure_resistance | 0.55 | 0.00 | 0.55 |
| parity_convergence | 0.40 | 0.00 | 0.40 |

**Composite**: 0.5750

### a1-01

**Proposition**: The inability of end-to-end golden tests to localize divergences to a specific pipeline stage is a mathematical property of composed functions, not a tooling limitation that can be resolved by better pixel-diff analysis or smarter hypothesis selection.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.90 | 0.00 | 0.90 |
| actionability | 0.25 | 0.00 | 0.25 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.30 | 0.00 | 0.30 |

**Composite**: 0.5625

### a1-15

**Proposition**: The tdx_init computation fix discovered during the area sampling investigation (Hypothesis 1) improved 5 unrelated tests, demonstrating that even disproven hypotheses can produce valuable side-effect fixes.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.95 | 0.00 | 0.95 |
| actionability | 0.15 | 0.00 | 0.15 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.40 | -0.05 | 0.35 |

**Composite**: 0.5625

### a1-18

**Proposition**: The polygon rasterization divergence in Group G2 (6 tests) can be localized by a harness comparing edge-crossing x-coordinates per scanline between Rust and C++, specifically where x_cur += dx_per_row diverges from C++'s in-place x1 += dx mutation.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.55 | 0.00 | 0.55 |
| actionability | 0.65 | 0.00 | 0.65 |
| failure_resistance | 0.50 | 0.00 | 0.50 |
| parity_convergence | 0.55 | 0.00 | 0.55 |

**Composite**: 0.5625

### a1-11

**Proposition**: The PaintImageColored investigation (Hypothesis 3) was examining the wrong rendering function entirely -- the divergent pixels at coordinates (x=24-35, y=288-305) correspond to the border indicator area, not the label text area where PaintImageColored operates (x=170-776).

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | 0.00 | 0.85 |
| actionability | 0.30 | 0.00 | 0.30 |
| failure_resistance | 0.75 | 0.00 | 0.75 |
| parity_convergence | 0.30 | 0.00 | 0.30 |

**Composite**: 0.5500

### a1-02

**Proposition**: Three rounds of hypothesis-driven investigation (area sampling inner loop, transform parameters TX/TY/TDX/TDY, PaintImageColored color mapping) consumed 5-10 hours of agentic compute and produced zero direct fixes for the 21-test Group A+B failure cluster.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.95 | 0.00 | 0.95 |
| actionability | 0.15 | 0.00 | 0.15 |
| failure_resistance | 0.85 | 0.00 | 0.85 |
| parity_convergence | 0.20 | 0.00 | 0.20 |

**Composite**: 0.5375

### a1-08

**Proposition**: Stage-level verification eliminates the classification problem: when all upstream stage harnesses pass, any remaining pixel divergence is definitionally in the compositing stage, with no spatial analysis or rendering-function attribution needed.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | 0.00 | 0.85 |
| actionability | 0.30 | -0.05 | 0.25 |
| failure_resistance | 0.75 | 0.00 | 0.75 |
| parity_convergence | 0.35 | -0.05 | 0.30 |

**Composite**: 0.5375

### a1-23

**Proposition**: A differential testing harness in the sense of McKeeman (1998), applied at each pipeline stage rather than only at the output, would automate the comparison of Rust and C++ intermediate values for all golden test inputs.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.65 | 0.00 | 0.65 |
| actionability | 0.55 | 0.00 | 0.55 |
| failure_resistance | 0.50 | 0.00 | 0.50 |
| parity_convergence | 0.45 | 0.00 | 0.45 |

**Composite**: 0.5375

### a1-09

**Proposition**: The Group A+B failure classification was revised three times (missing HowTo text -> PaintTextBoxed divergence -> 9-slice section boundary rounding), demonstrating that end-to-end golden tests provide insufficient signal for reliable root-cause classification.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.90 | 0.00 | 0.90 |
| actionability | 0.20 | -0.05 | 0.15 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.25 | 0.00 | 0.25 |

**Composite**: 0.5250

### From Agent 2 (Current-Methodology Defender)

### a2-05

**Proposition**: G3 (Hermite factor table, 2 tests, max_diff=1) can be fixed by replacing f64 runtime computation with a literal port of the C++ compile-time table constants.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.88 | 0.00 | 0.88 |
| actionability | 0.95 | 0.00 | 0.95 |
| failure_resistance | 0.85 | 0.00 | 0.85 |
| parity_convergence | 0.30 | 0.00 | 0.30 |

**Composite**: 0.7450

### a2-23

**Proposition**: The Group A root cause was corrected from 'PaintTextBoxed text rendering divergence' to '9-slice border image section boundary rounding' based on pixel tracing evidence showing divergent pixels at widget borders, not text regions.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | 0.00 | 0.85 |
| actionability | 0.70 | 0.00 | 0.70 |
| failure_resistance | 0.65 | 0.00 | 0.65 |
| parity_convergence | 0.75 | 0.00 | 0.75 |

**Composite**: 0.7375

### a2-02

**Proposition**: The 37 remaining golden test failures have been classified into 12 distinct groups with verified root causes, spatial evidence, and confidence labels.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.92 | 0.00 | 0.92 |
| actionability | 0.65 | 0.00 | 0.65 |
| failure_resistance | 0.75 | 0.00 | 0.75 |
| parity_convergence | 0.60 | 0.00 | 0.60 |

**Composite**: 0.7300

### a2-06

**Proposition**: G5 (blend_hash_lookup, 1 test, max_diff=1) can be fixed by replacing (c*a+127)/255 with the C++ (x*257+0x8073)>>16 Blinn formula.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.90 | 0.00 | 0.90 |
| actionability | 0.97 | 0.00 | 0.97 |
| failure_resistance | 0.90 | 0.00 | 0.90 |
| parity_convergence | 0.15 | 0.00 | 0.15 |

**Composite**: 0.7300

### a2-16

**Proposition**: The next Group A investigation step is a ~330-line side-by-side comparison of Rust paint_9slice_section (lines 2838-3003) with C++ PaintBorderImage (emPainter.cpp 1892-1982) and ScanlineTool::Init (emPainter_ScTl.cpp 228-293).

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.75 | 0.00 | 0.75 |
| actionability | 0.92 | 0.00 | 0.92 |
| failure_resistance | 0.50 | -0.10 | 0.40 |
| parity_convergence | 0.80 | -0.05 | 0.75 |

**Composite**: 0.7050

### a2-07

**Proposition**: G3 + G5 combined can be fixed in approximately 30 minutes, advancing the count from 204/241 to 207/241.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.82 | 0.00 | 0.82 |
| actionability | 0.93 | 0.00 | 0.93 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.25 | 0.00 | 0.25 |

**Composite**: 0.7000

### a2-22

**Proposition**: The remaining 37 failures decompose into three effort tiers: 3 tests with known trivial fixes (G3+G5), 20 tests with a localized root cause needing side-by-side comparison (A+B), and 14 tests requiring standard per-group investigation (G2, G4, G7, G8, G9, C, D).

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.72 | 0.00 | 0.72 |
| actionability | 0.80 | 0.00 | 0.80 |
| failure_resistance | 0.50 | 0.00 | 0.50 |
| parity_convergence | 0.70 | 0.00 | 0.70 |

**Composite**: 0.6800

### a2-04

**Proposition**: The C++ ScanlineTool::Init uses clamped source width (ImgW after bounds checking) whereas Rust paint_9slice_section passes unclamped sw, which is the likely cause of Group A divergence.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.60 | -0.05 | 0.55 |
| actionability | 0.90 | 0.00 | 0.90 |
| failure_resistance | 0.45 | -0.05 | 0.40 |
| parity_convergence | 0.85 | 0.00 | 0.85 |

**Composite**: 0.6750

### a2-24

**Proposition**: The C++ rendering pipeline for 9-slice sections goes through PaintBorderImage -> PaintImage -> PaintRect -> ScanlineTool::Init, whereas Rust uses an inline transform computation in paint_9slice_section, and this structural difference is the source of divergence.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.60 | -0.05 | 0.55 |
| actionability | 0.80 | 0.00 | 0.80 |
| failure_resistance | 0.45 | -0.05 | 0.40 |
| parity_convergence | 0.80 | 0.00 | 0.80 |

**Composite**: 0.6375

### a2-19

**Proposition**: G2 (polygon rasterizer floating-point divergence, 6 tests) is a fully independent group that can be fixed by porting C++ edge-crossing accumulation to match in-place arithmetic.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.65 | 0.00 | 0.65 |
| actionability | 0.75 | 0.00 | 0.75 |
| failure_resistance | 0.55 | 0.00 | 0.55 |
| parity_convergence | 0.45 | 0.00 | 0.45 |

**Composite**: 0.6000

### a2-01

**Proposition**: The classify-hypothesize-investigate-fix methodology has produced monotonic progress from 199/241 to 204/241 passing golden tests with zero regressions.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.95 | 0.00 | 0.95 |
| actionability | 0.20 | 0.00 | 0.20 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.40 | 0.00 | 0.40 |

**Composite**: 0.5875

### a2-10

**Proposition**: The 12 classified failure groups can be attacked independently and in parallel, unlike a unified comparison harness which would force all groups through a single infrastructure bottleneck.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | -0.05 | 0.65 |
| actionability | 0.60 | 0.00 | 0.60 |
| failure_resistance | 0.50 | -0.05 | 0.45 |
| parity_convergence | 0.55 | 0.00 | 0.55 |

**Composite**: 0.5625

### a2-11

**Proposition**: The investigation has verified 7 subsystems as correct (interpolation, transform parameters, PaintImageColored, area sampling inner loop, compositing, RoundX/RoundY rounding, border image TGA data), permanently removing them from suspicion.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.80 | -0.10 | 0.70 |
| actionability | 0.40 | 0.00 | 0.40 |
| failure_resistance | 0.65 | -0.10 | 0.55 |
| parity_convergence | 0.50 | 0.00 | 0.50 |

**Composite**: 0.5375

### a2-13

**Proposition**: Golden pixel comparison at zero tolerance is the correct and sufficient verification level; intermediate value comparison (transform parameters, interpolation weights) adds debugging aid but not stronger evidence.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | 0.00 | 0.85 |
| actionability | 0.25 | 0.00 | 0.25 |
| failure_resistance | 0.70 | -0.10 | 0.60 |
| parity_convergence | 0.45 | 0.00 | 0.45 |

**Composite**: 0.5375

### a2-18

**Proposition**: The monotonic ratchet property (tolerances only decrease, test counts only increase, no regressions) is a structural feature of the literal-port methodology, not accidental.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.75 | 0.00 | 0.75 |
| actionability | 0.20 | 0.00 | 0.20 |
| failure_resistance | 0.70 | -0.05 | 0.65 |
| parity_convergence | 0.40 | 0.00 | 0.40 |

**Composite**: 0.5000

### From Agent 3 (Failure-Mode Analyst)

### a3-17

**Proposition**: Approach B's pipeline decomposition targets only image rendering (Groups A+B+D, 21 tests), leaving 16 tests across 8 groups (G2 polygon, G3 composition, G5 starfield, G7 gradient, G8 layout, G9 polyline) that require entirely different investigation approaches and get zero benefit from the infrastructure.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.90 | 0.00 | 0.90 |
| actionability | 0.65 | 0.00 | 0.65 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.60 | 0.00 | 0.60 |

**Composite**: 0.7375

### a3-06

**Proposition**: The C++ area sampling loop carries state between pixels within a scanline (AreaSampleCarryState), and a 1-LSB error in pixel N propagates to pixel N+1, making this class of bug invisible to static code comparison and only visible through runtime tracing.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.80 | 0.00 | 0.80 |
| actionability | 0.65 | 0.00 | 0.65 |
| failure_resistance | 0.70 | 0.00 | 0.70 |
| parity_convergence | 0.65 | 0.00 | 0.65 |

**Composite**: 0.7000

### a3-08

**Proposition**: Building intermediate comparison harnesses (Approach B) risks the state-coupling problem: C++ ScanlineTool::Init computes all derived state in a single monolithic function, and testing stages separately may use pre-modification values that differ from the actual pipeline's post-modification values.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.80 | 0.00 | 0.80 |
| actionability | 0.65 | +0.05 | 0.70 |
| failure_resistance | 0.75 | 0.00 | 0.75 |
| parity_convergence | 0.55 | 0.00 | 0.55 |

**Composite**: 0.7000

### a3-18

**Proposition**: C++ unsigned-times-signed integer arithmetic involves implicit conversion to unsigned, while Rust requires explicit casts, and agents rarely track signedness through multi-step arithmetic when comparing the two codebases.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.75 | 0.00 | 0.75 |
| actionability | 0.70 | +0.05 | 0.75 |
| failure_resistance | 0.70 | 0.00 | 0.70 |
| parity_convergence | 0.60 | 0.00 | 0.60 |

**Composite**: 0.7000

### a3-03

**Proposition**: Three investigation rounds targeting Group A+B produced real fixes (blend_colored_scanline, HowTo text wiring) but zero Group A+B fixes, demonstrating a systematic diversion pattern where agents fix adjacent bugs instead of the target root cause.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.95 | 0.00 | 0.95 |
| actionability | 0.55 | -0.05 | 0.50 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.50 | 0.00 | 0.50 |

**Composite**: 0.6875

### a3-23

**Proposition**: The Rust and C++ pipelines have fundamentally different decompositions: C++ ScanlineTool::Init is a 280-line monolithic function whose responsibilities are distributed across four separate Rust structs (Transform24, SectionBounds, InterpolationBuffer, BlendMode), making stage-boundary comparison inherently ambiguous.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | +0.05 | 0.90 |
| actionability | 0.55 | +0.05 | 0.60 |
| failure_resistance | 0.75 | 0.00 | 0.75 |
| parity_convergence | 0.50 | 0.00 | 0.50 |

**Composite**: 0.6875

### a3-24

**Proposition**: After Group A+B is fixed by either approach, 16 tests remain across 8 diverse groups (polygon, composition, starfield, gradient, layout, polyline), each potentially requiring its own investigation methodology, and the project returns to incremental investigation for the tail regardless of approach choice.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | 0.00 | 0.85 |
| actionability | 0.50 | 0.00 | 0.50 |
| failure_resistance | 0.80 | 0.00 | 0.80 |
| parity_convergence | 0.55 | 0.00 | 0.55 |

**Composite**: 0.6750

### a3-22

**Proposition**: The sunk cost of three investigation rounds on Group A+B creates trajectory lock: the context window contains investigation history that primes agents to continue on A+B rather than pivot to higher-expected-value targets like the smaller groups.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | -0.05 | 0.65 |
| actionability | 0.75 | 0.00 | 0.75 |
| failure_resistance | 0.60 | 0.00 | 0.60 |
| parity_convergence | 0.65 | 0.00 | 0.65 |

**Composite**: 0.6625

### a3-07

**Proposition**: Groups G2 (polygon, 2 tests), G7 (gradient, 2 tests), G9 (polyline, 1 test), and other small groups may each require only a single targeted investigation round, making them higher expected-value targets than the deeply-investigated Group A+B.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.55 | -0.05 | 0.50 |
| actionability | 0.80 | 0.00 | 0.80 |
| failure_resistance | 0.50 | 0.00 | 0.50 |
| parity_convergence | 0.70 | 0.00 | 0.70 |

**Composite**: 0.6250

### a3-13

**Proposition**: Neither Approach A (continue current hypothesis elimination) nor Approach B (incremental rebuild with intermediate harnesses) eliminates the fundamental 'last mile' problem: an agent must ultimately compare C++ and Rust integer arithmetic line-by-line, where both approaches are equally vulnerable to agent limitations.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.85 | +0.05 | 0.90 |
| actionability | 0.35 | 0.00 | 0.35 |
| failure_resistance | 0.80 | +0.05 | 0.85 |
| parity_convergence | 0.30 | 0.00 | 0.30 |

**Composite**: 0.6000

### a3-16

**Proposition**: The 204 passing golden tests at zero tolerance are the project's most valuable asset, and any structural rewrite of paint_9slice_section risks the 'fix one, break three' pattern where correcting incorrect sections changes the behavior of already-correct sections.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | -0.05 | 0.65 |
| actionability | 0.60 | 0.00 | 0.60 |
| failure_resistance | 0.65 | 0.00 | 0.65 |
| parity_convergence | 0.50 | 0.00 | 0.50 |

**Composite**: 0.6000

### a3-15

**Proposition**: The reward signal from fixing adjacent bugs during Group A+B investigation (blend_colored_scanline, HowTo text) biases agents toward broad investigation over deep investigation, which is the wrong direction for a root cause in a specific 10-line code section.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.75 | -0.05 | 0.70 |
| actionability | 0.50 | 0.00 | 0.50 |
| failure_resistance | 0.65 | 0.00 | 0.65 |
| parity_convergence | 0.45 | 0.00 | 0.45 |

**Composite**: 0.5750

### a3-19

**Proposition**: The CLAUDE.md boundary between 'pixel arithmetic' (literal C++ port required) and 'geometry' (same algorithm, idiomatic Rust OK) is ambiguous for functions like paint_9slice_section's coordinate computation, and investigators classify the same function inconsistently across rounds.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.65 | 0.00 | 0.65 |
| actionability | 0.55 | 0.00 | 0.55 |
| failure_resistance | 0.60 | 0.00 | 0.60 |
| parity_convergence | 0.50 | 0.00 | 0.50 |

**Composite**: 0.5750

### a3-10

**Proposition**: Zero-tolerance golden tests are fragile: a refactoring that changes floating-point operation order (e.g., replacing a*b+c with c+a*b) can change geometry by 1 ULP, shift sub-pixel edges, and fail tests despite producing mathematically equivalent results.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.75 | -0.05 | 0.70 |
| actionability | 0.50 | 0.00 | 0.50 |
| failure_resistance | 0.70 | 0.00 | 0.70 |
| parity_convergence | 0.35 | 0.00 | 0.35 |

**Composite**: 0.5625

### a3-02

**Proposition**: Each round of investigation that declares a subsystem 'structurally equivalent with minor differences' adds a layer of fog that concentrates specifically around the areas where the divergence actually lives, because those are the most-investigated and most-declared-equivalent areas.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | 0.00 | 0.70 |
| actionability | 0.40 | 0.00 | 0.40 |
| failure_resistance | 0.65 | 0.00 | 0.65 |
| parity_convergence | 0.45 | 0.00 | 0.45 |

**Composite**: 0.5500

### a3-09

**Proposition**: Some of the 37 failing golden tests may have incorrect reference images generated by the C++ generator with wrong setup (fill, canvas, scale), making pixel parity with those references the wrong goal.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.60 | -0.05 | 0.55 |
| actionability | 0.75 | 0.00 | 0.75 |
| failure_resistance | 0.55 | 0.00 | 0.55 |
| parity_convergence | 0.40 | -0.05 | 0.35 |

**Composite**: 0.5500

### a3-12

**Proposition**: The 37 failing tests are grouped into 12 groups based on symptom similarity, but this grouping may be incorrect because groups that appear unrelated (e.g., Group A+B border images and G7 gradient) may share a root cause in upstream code like sub-pixel edge calculation.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.50 | -0.05 | 0.45 |
| actionability | 0.60 | 0.00 | 0.60 |
| failure_resistance | 0.55 | 0.00 | 0.55 |
| parity_convergence | 0.60 | 0.00 | 0.60 |

**Composite**: 0.5500

### a3-20

**Proposition**: Infrastructure investment in intermediate comparison harnesses creates commitment to using that infrastructure, and if the infrastructure's comparison is at the wrong granularity or wrong pipeline point, it becomes a trusted oracle that is systematically wrong.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.60 | -0.05 | 0.55 |
| actionability | 0.45 | 0.00 | 0.45 |
| failure_resistance | 0.65 | 0.00 | 0.65 |
| parity_convergence | 0.35 | 0.00 | 0.35 |

**Composite**: 0.5000


## 5. Contested (unresolved tension with severity >= 0.60)

*No propositions remain contested. All 28 tensions were adjudicated in Round 3.*

## 6. Fallen (composite < 0.50 or evidential_grounding < 0.30)

### From Agent 1 (Stage-Decomposition Advocate)

### a1-17

**Proposition**: Golden tests should serve only as composition verification (do correctly-implemented stages compose correctly), not double as component verification (does each stage produce correct intermediates), because conflating these two roles is the root cause of diagnostic fog.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | -0.05 | 0.65 |
| actionability | 0.40 | 0.00 | 0.40 |
| failure_resistance | 0.60 | 0.00 | 0.60 |
| parity_convergence | 0.35 | -0.05 | 0.30 |

**Composite**: 0.4875

### a1-10

**Proposition**: 36 of 37 failing golden tests pass through one or more shared pipeline stages (section rectangle computation, transform initialization, interpolation, compositing), so stage-level harnesses built once provide diagnostic capability for nearly all failures.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.60 | -0.15 | 0.45 |
| actionability | 0.45 | 0.00 | 0.45 |
| failure_resistance | 0.50 | -0.10 | 0.40 |
| parity_convergence | 0.55 | 0.00 | 0.55 |

**Composite**: 0.4625

### a1-21

**Proposition**: Vouk (1990) found that in interlanguage conversion projects, testing only at the system boundary causes defects to cluster in module interactions, while incremental module-boundary testing catches defects at their point of introduction.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.75 | 0.00 | 0.75 |
| actionability | 0.25 | 0.00 | 0.25 |
| failure_resistance | 0.65 | 0.00 | 0.65 |
| parity_convergence | 0.20 | 0.00 | 0.20 |

**Composite**: 0.4625

### a1-16

**Proposition**: Investigator hypothesis selection is biased toward stages they understand best, code they read most recently, and divergence patterns they have seen before, leading to systematically skipping structurally simple code assumed to be correct.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.75 | 0.00 | 0.75 |
| actionability | 0.20 | 0.00 | 0.20 |
| failure_resistance | 0.65 | 0.00 | 0.65 |
| parity_convergence | 0.20 | 0.00 | 0.20 |

**Composite**: 0.4500

### a1-12

**Proposition**: The hypothesis cascade approach has a hidden cost: false confidence from correct eliminations consumes the investigation budget on diagnosis rather than leaving it available for fixing the actual root cause.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | 0.00 | 0.70 |
| actionability | 0.20 | 0.00 | 0.20 |
| failure_resistance | 0.60 | 0.00 | 0.60 |
| parity_convergence | 0.25 | 0.00 | 0.25 |

**Composite**: 0.4375

### a1-20

**Proposition**: The project is at an inflection point where continuing the hypothesis cascade approach yields diminishing returns, because the remaining uninvestigated areas (section rectangle computation, Init logic divergence) are precisely those that the cascade systematically skipped.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.60 | -0.05 | 0.55 |
| actionability | 0.30 | 0.00 | 0.30 |
| failure_resistance | 0.45 | -0.05 | 0.40 |
| parity_convergence | 0.35 | 0.00 | 0.35 |

**Composite**: 0.4000

### a1-07

**Proposition**: The total engineering cost of building stage-level comparison harnesses (C++ instrumentation, Rust instrumentation, comparison tooling) is estimated at 4-8 hours, which is already at break-even against the 5-10 hours spent on three disproven hypotheses.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.40 | -0.10 | 0.30 |
| actionability | 0.50 | -0.10 | 0.40 |
| failure_resistance | 0.35 | 0.00 | 0.35 |
| parity_convergence | 0.40 | 0.00 | 0.40 |

**Composite**: 0.3625

### a1-25

**Proposition**: The existing golden test format (.painter.golden, .layout.golden) should be extended with a stage-trace variant that stores intermediate values alongside pixel data, making stage-level comparison a permanent part of the test infrastructure rather than a one-time diagnostic.

**Source**: Agent 1 (Stage-Decomposition Advocate)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.45 | -0.05 | 0.40 |
| actionability | 0.40 | -0.10 | 0.30 |
| failure_resistance | 0.40 | 0.00 | 0.40 |
| parity_convergence | 0.30 | 0.00 | 0.30 |

**Composite**: 0.3500

### From Agent 2 (Current-Methodology Defender)

### a2-12

**Proposition**: The projected fix trajectory is: G3+G5 (207/241), then Group A+B (227/241), then G2 (233/241), then long tail (241/241), achievable in approximately 6 sessions.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.40 | -0.10 | 0.30 |
| actionability | 0.70 | 0.00 | 0.70 |
| failure_resistance | 0.30 | -0.15 | 0.15 |
| parity_convergence | 0.65 | 0.00 | 0.65 |

**Composite**: 0.4500

### a2-14

**Proposition**: The CLAUDE.md port fidelity rules (reproduce C++ integer formulas exactly for pixel arithmetic, same algorithm for geometry) already encode the methodology and no additional infrastructure is needed to convey the requirements.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.80 | -0.10 | 0.70 |
| actionability | 0.30 | 0.00 | 0.30 |
| failure_resistance | 0.55 | -0.10 | 0.45 |
| parity_convergence | 0.35 | 0.00 | 0.35 |

**Composite**: 0.4500

### a2-08

**Proposition**: Each of the three disproven Group A hypotheses produced collateral value: 5 tests fixed, one subsystem permanently verified correct, and root cause attribution corrected.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.88 | 0.00 | 0.88 |
| actionability | 0.15 | 0.00 | 0.15 |
| failure_resistance | 0.60 | -0.10 | 0.50 |
| parity_convergence | 0.35 | -0.10 | 0.25 |

**Composite**: 0.4450

### a2-17

**Proposition**: Every prior correctness improvement (area sampling, HowTo text, blend_colored_scanline, pixel_scale passthrough) was discovered as a side effect of investigating golden test failure hypotheses.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.90 | 0.00 | 0.90 |
| actionability | 0.10 | 0.00 | 0.10 |
| failure_resistance | 0.55 | -0.15 | 0.40 |
| parity_convergence | 0.30 | -0.10 | 0.20 |

**Composite**: 0.4000

### a2-15

**Proposition**: Switching to a different methodology would not transfer the accumulated investigation knowledge (failure catalog, verified-correct subsystems, spatial evidence) and would require re-derivation through the new methodology's lens.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.65 | -0.10 | 0.55 |
| actionability | 0.15 | 0.00 | 0.15 |
| failure_resistance | 0.50 | -0.10 | 0.40 |
| parity_convergence | 0.30 | 0.00 | 0.30 |

**Composite**: 0.3500

### a2-09

**Proposition**: Building intermediate comparison harnesses (C++ instrumentation logging + Rust log alignment + diff tooling) would cost more than reading the two functions side-by-side and produce no additional test fixes.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.50 | -0.15 | 0.35 |
| actionability | 0.30 | 0.00 | 0.30 |
| failure_resistance | 0.35 | -0.10 | 0.25 |
| parity_convergence | 0.40 | 0.00 | 0.40 |

**Composite**: 0.3250

### a2-21

**Proposition**: Inserting printf/logging into C++ instrumentation code can change register allocation and floating-point rounding, making intermediate comparison harnesses potentially unreliable.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.70 | -0.15 | 0.55 |
| actionability | 0.15 | -0.05 | 0.10 |
| failure_resistance | 0.45 | 0.00 | 0.45 |
| parity_convergence | 0.20 | 0.00 | 0.20 |

**Composite**: 0.3250

### a2-25

**Proposition**: The rate of progress constraint is investigator availability (how often someone sits down to do the work), not methodological limitation.

**Source**: Agent 2 (Current-Methodology Defender)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.35 | -0.10 | 0.25 |
| actionability | 0.10 | 0.00 | 0.10 |
| failure_resistance | 0.25 | -0.20 | 0.05 |
| parity_convergence | 0.20 | 0.00 | 0.20 |

**Composite**: 0.1500

### From Agent 3 (Failure-Mode Analyst)

### a3-21

**Proposition**: A backward trace from a pixel divergence through the rendering pipeline has approximately 0.35 probability of being error-free over 10 steps (at P=0.1 per step), meaning most deep traces by agents will contain at least one error.

**Source**: Agent 3 (Failure-Mode Analyst)

| Axis | Initial | Delta | Final |
|------|---------|-------|-------|
| evidential_grounding | 0.45 | 0.00 | 0.45 |
| actionability | 0.40 | 0.00 | 0.40 |
| failure_resistance | 0.55 | 0.00 | 0.55 |
| parity_convergence | 0.30 | 0.00 | 0.30 |

**Composite**: 0.4250


## 7. Tension Resolution Map (high-severity tensions >= 0.70)

**t-01** (severity 0.92) -- Harness cost: a1-07 vs a2-09
  - *Resolution*: Both cost estimates lack empirical grounding; a2-09 fell harder because its counterfactual claim ("would cost MORE and produce no fixes") was even less evidenced than a1-07's speculative 4-8 hour estimate.

**t-02** (severity 0.88) -- Epistemic value of intermediate comparison: a1-08 vs a2-13
  - *Resolution*: a2-13 is right that pixel comparison is the final arbiter, but wrong to dismiss intermediate comparison as mere debugging aid -- three failed Group A+B rounds prove that diagnostic acceleration IS the bottleneck, so a2-13 lost failure_resistance.

**t-24** (severity 0.88) -- Last-mile agent comparison bottleneck: a3-13 vs a1-04 vs a2-16
  - *Resolution*: The last-mile problem (a3-13) is real and was upgraded; a2-16 (side-by-side comparison) took the hardest hit because it is most directly dependent on the agent code-reading capability that four identified failure modes undermine; a1-04 (harness) took a smaller hit because runtime value comparison partially sidesteps code-reading limitations.

**t-03** (severity 0.85) -- Collateral value vs missed target: a1-09 vs a2-08
  - *Resolution*: The collateral fixes are real but the framing as methodology success is fragile -- three rounds targeting 21 tests with zero direct fixes is a signal that cannot be dismissed as normal scientific progress, so a2-08 lost on both failure_resistance and parity_convergence.

**t-07** (severity 0.85) -- Side-by-side comparison vs macro misinterpretation: a2-16 vs a3-04
  - *Resolution*: a3-04 (macro expansion warning) was upgraded because pre-expanding C++ macros for CHANNELS=4/CLAMP is the single most actionable preparatory step for any comparison approach; a2-16 was cut because as-stated it does not include mitigations for the four identified agent failure modes.

**t-08** (severity 0.83) -- Verified-correct subsystems vs abstraction-level blindspot: a2-11 vs a3-01
  - *Resolution*: a3-01 won decisively -- "verified correct" was too strong for input-level-only verification, and the insight that derived state (not just inputs) must be compared was the most important diagnostic finding for Group A+B; a3-01 was upgraded on both actionability and parity_convergence.

**t-04** (severity 0.82) -- Methodology stall vs availability constraint: a1-20 vs a2-25
  - *Resolution*: a2-25 took the hardest hit of any proposition in the entire dialectic (-0.10 evidential_grounding, -0.20 failure_resistance) because its unfalsifiable "just need more time" claim provides no useful decision-making guidance after three focused rounds failed.

**t-05** (severity 0.80) -- Stage 1 harness vs state-coupling risk: a1-04 vs a3-08
  - *Resolution*: The defense rescued a1-04 by showing section rectangles are post-computation outputs not mid-Init intermediates, so the state-coupling warning applies to harness design but does not invalidate the concept; a3-08 was upgraded on actionability for its useful design constraint.

**t-13** (severity 0.80) -- Serendipitous discovery vs systematic diversion: a2-17 vs a3-03
  - *Resolution*: a2-17 (every fix came from investigation) took heavy cuts because the claim is trivially true when all work IS investigation, and because it elevates serendipity to methodology; a3-03's diversion pattern observation survived as descriptive but took a small cut for not prescribing a solution.

**t-06** (severity 0.78) -- Harness coverage: a1-10 vs a3-17
  - *Resolution*: a3-17's counter was factually decisive: the 36/37 coverage claim is verifiably wrong; realistic coverage is 21/37, and a1-10 took steep cuts on both evidential_grounding and failure_resistance for the inflated metric.

**t-09** (severity 0.77) -- Architectural divergence implications: a1-06 vs a2-24 vs a3-23
  - *Resolution*: All three agents agree the structural mismatch exists; a1-06 and a2-24 took small cuts for presenting correlation as causation, while a3-23 was upgraded for naming the decomposition problem neutrally without prescribing a specific solution.

**t-10** (severity 0.75) -- Investigation sequencing: a2-12 vs a3-07/a3-22
  - *Resolution*: a2-12's 6-session trajectory was identified as planning fallacy and took heavy cuts; the breadth-first approach (quick wins then untried groups) was judged better-supported on Bayesian grounds, though the "small groups are easy" assumption is also speculative.

**t-18** (severity 0.73) -- Stage 2 harness vs abstraction-level gap: a1-05 vs a3-01
  - *Resolution*: a3-01 won this tension too -- the abstraction-level gap (inputs vs derived state) actually justifies a harness at the right level, making a1-05's existing framing ("already verified") counterproductive to its own case; a3-01 was upgraded on both actionability and parity_convergence.

**t-17** (severity 0.72) -- Competing sub-hypotheses: a2-04 vs a3-05
  - *Resolution*: a2-04 (clamped/unclamped sw) and a3-05 (stride-reduction ordering) may describe the same bug from different angles, but a3-05 was upgraded because it cites specific C++ lines and ordering dependencies, making it the more precise and testable formulation.

**t-25** (severity 0.72) -- Restructuring benefit vs regression risk: a1-06 vs a3-16
  - *Resolution*: Both took small cuts; the regression risk is real but manageable through golden test regression guards already in place, and the project's zero-regression track record partially mitigates a3-16's concern.

**t-11** (severity 0.70) -- Testing philosophy: a1-17 vs a2-14
  - *Resolution*: a2-14 lost more heavily because the G5 divergence (Blinn formula rule violated despite CLAUDE.md mandate) is a direct counterexample to "existing rules suffice"; a1-17 took smaller cuts for overstating conflated testing as "the root cause" of fog when it is one factor among several.

**t-22** (severity 0.70) -- Investigator bias vs side-by-side comparison: a1-16 vs a2-16
  - *Resolution*: a2-16 was cut because the identified bias pattern (skipping "obviously correct" code) specifically threatens the side-by-side comparison approach; the bias is mitigable through systematic line-by-line discipline but the proposition as-stated does not include such safeguards.

**t-27** (severity 0.70) -- Transform comparison vs representation mismatch: a1-05 vs a3-14
  - *Resolution*: a3-14 was upgraded on actionability because its observation that C++ pointer arithmetic and Rust SectionBounds are incommensurable without semantic mapping is directly useful for designing any comparison methodology; a1-05 lost ground because naive numerical comparison is insufficient.

## 8. Key Takeaway

Of 74 propositions, 8 survived, 49 were wounded, and 17 fell. Agent 1 (stage-decomposition advocate) had 1 survivors and 8 fallen; Agent 2 (current-methodology defender) had 2 survivors and 8 fallen; Agent 3 (failure-mode analyst) had 5 survivors and 1 fallen.

**Neither approach dominates.** The strongest survivors include both a practical current-methodology action (a2-05, a2-06: fix G3 Hermite table and G5 Blinn formula as quick wins), a stage-decomposition diagnostic insight (a3-05: stride-reduction ordering dependency as the most precise Group A+B sub-hypothesis), and a failure-mode mitigation (a3-04: pre-expand C++ macros to fix the agent code-reading bottleneck). The heaviest casualties were the extreme positions on both sides: Agent 2's claims that investigator availability is the only constraint (a2-25) and that the projected 6-session trajectory is reliable (a2-12) both fell or were severely wounded, while Agent 1's infrastructure cost estimates (a1-07) and coverage claims (a1-10, a1-25) were also cut down. The evidence favors a **hybrid approach**: execute G3+G5 quick wins immediately, pre-expand C++ macros for accurate code comparison, then investigate Group A+B with specific focus on the stride-reduction ordering dependency (a3-05) and derived-state verification (a3-01), while pivoting to small groups (G2, G7, G9) if the next Group A+B round fails. Full-infrastructure stage harnesses are not justified given cost uncertainty, but targeted runtime value tracing at specific points (post-Init derived state) is warranted.
